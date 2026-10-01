//! Literal child executable, inherited stdio, and owned temporary Markdown file.
use crate::{
    commands::document_content::{decode_file, edited_body},
    config::ChildEnvOverlay,
    error::{AppError, AppErrorKind},
    text::js_space,
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
        let value = decoded.trim_matches(js_space);
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
        let bytes = fs::read(&temp.0)?;
        Ok(EditorOutcome::Content(edited_body(&decode_file(&bytes))))
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
