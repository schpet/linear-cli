//! Literal child executable, inherited stdio, and owned temporary Markdown file.
use crate::{
    commands::text_input::{edited_body, read_file},
    config::ChildEnvOverlay,
    error::{AppError, AppErrorKind},
};
use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

pub const NO_EDITOR: &str = "Set EDITOR environment variable or configure git editor with: git config --global core.editor <editor>";
pub fn discover(env: &ChildEnvOverlay) -> Option<OsString> {
    if let Ok(output) = Command::new("git")
        .args(["config", "--global", "core.editor"])
        .envs(env.iter())
        .stdin(Stdio::null())
        .output()
        && output.status.success()
    {
        let decoded = String::from_utf8_lossy(&output.stdout);
        let value = decoded.trim();
        if !value.is_empty() {
            return Some(OsString::from(value));
        }
    }
    std::env::var_os("EDITOR").filter(|value| !value.is_empty())
}

struct TempFile(PathBuf);
impl TempFile {
    fn create(root: &Path) -> Result<Self, AppError> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        for _ in 0..100 {
            let path = root.join(format!(
                "linear-document-{}-{}.md",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match options.open(&path) {
                Ok(_) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(AppError::new(
                        AppErrorKind::IoProcess,
                        format!("Failed to create editor temporary file: {error}"),
                    )
                    .with_source(error));
                }
            }
        }
        Err(AppError::new(
            AppErrorKind::IoProcess,
            "Could not allocate a unique editor temporary file",
        ))
    }
}
impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0); /* Source explicitly best-effort cleanup. */
    }
}
#[derive(Debug)]
pub enum EditorOutcome {
    Content(Option<String>),
    Missing,
    Failed(AppError),
}
/// Temp creation remains outside the optional/create error conversion. Update
/// seed writing remains inside the editor error conversion.
pub fn open(
    env: &ChildEnvOverlay,
    seed: Option<&str>,
    root: &Path,
) -> Result<EditorOutcome, AppError> {
    let Some(editor) = discover(env) else {
        return Ok(EditorOutcome::Missing);
    };
    let temp = TempFile::create(root)?;
    let result: Result<EditorOutcome, std::io::Error> = (|| {
        if let Some(seed) = seed {
            let mut file = OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(&temp.0)?;
            file.write_all(seed.as_bytes())?;
        }
        let status = Command::new(editor)
            .arg(&temp.0)
            .envs(env.iter())
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()?;
        if !status.success() {
            return Ok(EditorOutcome::Failed(AppError::new(
                AppErrorKind::IoProcess,
                "Editor exited with an error",
            )));
        }
        let text = read_file(&temp.0)?;
        Ok(EditorOutcome::Content(edited_body(&text)))
    })();
    match result {
        Ok(outcome) => Ok(outcome),
        Err(error) => Ok(EditorOutcome::Failed(
            AppError::new(
                AppErrorKind::IoProcess,
                format!("Failed to open editor: {error}"),
            )
            .with_source(error),
        )),
    }
}

/// Update-create policy retains the actual failed child status. DOC open is unchanged.
#[derive(Debug)]
pub enum UpdateEditorOutcome {
    Content(Option<String>),
    Missing,
    Failed(AppError),
    ChildFailed(std::process::ExitStatus),
}
impl UpdateEditorOutcome {
    pub fn interrupted(&self) -> bool {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            matches!(self, Self::ChildFailed(status) if status.signal()==Some(signal_hook::consts::SIGINT))
        }
        #[cfg(not(unix))]
        {
            false
        }
    }
}
#[cfg(unix)]
mod update_signal {
    use super::*;
    use std::sync::{Arc, Mutex, MutexGuard, OnceLock, atomic::AtomicBool};
    struct State {
        outside: Arc<AtomicBool>,
        scope: Mutex<()>,
        // Deliberately retained: unregister does not restore a signal disposition.
        _registration: signal_hook::SigId,
    }
    static STATE: OnceLock<Result<State, String>> = OnceLock::new();
    pub struct Guard {
        state: &'static State,
        _ownership: MutexGuard<'static, ()>,
    }
    pub fn enter() -> Result<Guard, AppError> {
        let state = STATE
            .get_or_init(|| {
                let outside = Arc::new(AtomicBool::new(true));
                signal_hook::flag::register_conditional_default(
                    signal_hook::consts::SIGINT,
                    outside.clone(),
                )
                .map(|registration| State {
                    outside,
                    scope: Mutex::new(()),
                    _registration: registration,
                })
                .map_err(|error| error.to_string())
            })
            .as_ref()
            .map_err(|message| {
                AppError::new(
                    AppErrorKind::IoProcess,
                    format!("Failed to register update editor SIGINT policy: {message}"),
                )
            })?;
        let ownership = state.scope.lock().map_err(|error| {
            AppError::new(
                AppErrorKind::Invariant,
                format!("Update editor signal ownership failed: {error}"),
            )
        })?;
        state.outside.store(false, Ordering::SeqCst);
        Ok(Guard {
            state,
            _ownership: ownership,
        })
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            self.state.outside.store(true, Ordering::SeqCst);
        }
    }
}
/// Configured update editor only: register lazily, reap, clean temp, then re-arm.
/// The short post-child read/cleanup window still swallows parent SIGINT by design.
pub fn open_update(env: &ChildEnvOverlay, root: &Path) -> Result<UpdateEditorOutcome, AppError> {
    let Some(editor) = discover(env) else {
        return Ok(UpdateEditorOutcome::Missing);
    };
    let temp = TempFile::create(root)?;
    #[cfg(unix)]
    let guard = match update_signal::enter() {
        Ok(guard) => guard,
        Err(error) => {
            drop(temp);
            return Err(error);
        }
    };
    let result = (|| {
        let mut child = match Command::new(editor)
            .arg(&temp.0)
            .envs(env.iter())
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
        {
            Ok(child) => child,
            Err(error) => {
                return UpdateEditorOutcome::Failed(
                    AppError::new(
                        AppErrorKind::IoProcess,
                        format!("Failed to open editor: {error}"),
                    )
                    .with_source(error),
                );
            }
        };
        let wait = |child: &mut std::process::Child| loop {
            match child.wait() {
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                result => break result,
            }
        };
        let status = match wait(&mut child) {
            Ok(status) => status,
            Err(error) => {
                let kill = child.kill();
                let reap = wait(&mut child);
                match reap {
                    Ok(_) => {}
                    Err(reap) => {
                        // Ownership cannot be released while the child may still be live.
                        // Abort rather than clean its file or re-arm with an unconfirmed reap.
                        eprintln!(
                            "Update editor ownership invariant failed: wait={error}; kill={kill:?}; reap={reap}"
                        );
                        std::process::abort();
                    }
                }
                return UpdateEditorOutcome::Failed(
                    AppError::new(
                        AppErrorKind::IoProcess,
                        format!("Failed to wait for editor: {error}"),
                    )
                    .with_source(error),
                );
            }
        };
        if !status.success() {
            return UpdateEditorOutcome::ChildFailed(status);
        }
        match read_file(&temp.0) {
            Ok(text) => UpdateEditorOutcome::Content(edited_body(&text)),
            Err(error) => UpdateEditorOutcome::Failed(
                AppError::new(
                    AppErrorKind::IoProcess,
                    format!("Failed to open editor: {error}"),
                )
                .with_source(error),
            ),
        }
    })();
    drop(temp);
    #[cfg(unix)]
    drop(guard);
    Ok(result)
}
