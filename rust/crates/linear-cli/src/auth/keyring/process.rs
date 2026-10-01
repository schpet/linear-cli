//! Uncapped concurrent keyring feed/drain/wait with owned cleanup.
use crate::{
    auth::mutation::MutationFailure,
    config::{ChildEnvOverlay, ConfigSecret},
    error::{AppError, AppErrorKind},
    text::js_trim,
};
use std::{io, process::Stdio};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::{Child, Command},
};

struct OwnedChild {
    child: Option<Child>,
}
impl OwnedChild {
    async fn cleanup(&mut self) {
        if let Some(child) = self.child.as_mut() {
            // Exited child may report InvalidInput on kill; wait still reaps it.
            let _kill_result = child.start_kill();
            let _wait_result = child.wait().await;
        }
        self.child = None;
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _kill_result = child.start_kill();
            // Own a reaper on future cancellation; Tokio's kill_on_drop remains
            // enabled for the case the runtime itself is shutting down.
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.spawn(async move {
                    let _wait_result = child.wait().await;
                });
            }
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessCompletion {
    Exited(i32),
    Signaled(i32),
}
impl ProcessCompletion {
    pub fn source_code(self) -> Result<i32, MutationFailure> {
        match self {
            Self::Exited(code) => Ok(code),
            Self::Signaled(signal) => 128_i32.checked_add(signal).ok_or_else(|| {
                MutationFailure::Typed(AppError::new(
                    AppErrorKind::IoProcess,
                    format!("Keyring signal {signal} cannot be represented as an exit code; backend effects may have occurred"),
                ))
            }),
        }
    }
}
fn completion(status: std::process::ExitStatus) -> Result<ProcessCompletion, MutationFailure> {
    if let Some(code) = status.code() {
        return Ok(ProcessCompletion::Exited(code));
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return Ok(ProcessCompletion::Signaled(signal));
        }
    }
    Err(MutationFailure::Typed(AppError::new(
        AppErrorKind::IoProcess,
        "Keyring command terminated without an exit code or signal; backend effects may have occurred",
    )))
}
pub struct ProcessReply {
    pub completion: ProcessCompletion,
    pub stdout: Vec<u8>,
    pub stderr: String,
}
pub async fn run(
    executable: &str,
    args: &[String],
    input: Option<&ConfigSecret>,
    overlay: &ChildEnvOverlay,
) -> Result<ProcessReply, MutationFailure> {
    if args.iter().any(|arg| arg.contains('\0')) {
        return Err(MutationFailure::Typed(AppError::new(
            AppErrorKind::Validation,
            "Keyring arguments cannot contain a NUL character",
        )));
    }
    let mut command = Command::new(executable);
    command
        .args(args)
        .envs(overlay.iter())
        .kill_on_drop(true)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = command.spawn().map_err(|error| MutationFailure::Ordinary(match executable {
        "secret-tool" => format!("Could not run secret-tool. Install libsecret (e.g. apt install libsecret-tools, pacman -S libsecret).\nAlternatively, set the LINEAR_API_KEY environment variable.\n  ({error})"),
        "/usr/bin/security" => format!("Could not run /usr/bin/security. Is this a macOS system?\n  ({error})"),
        _ => unreachable!("keyring executable is explicit and platform-owned"),
    }))?;
    let mut owned = OwnedChild { child: Some(child) };
    let child = owned
        .child
        .as_mut()
        .unwrap_or_else(|| unreachable!("owned child just spawned"));
    let mut stdout = child
        .stdout
        .take()
        .unwrap_or_else(|| unreachable!("requested stdout pipe"));
    let mut stderr = child
        .stderr
        .take()
        .unwrap_or_else(|| unreachable!("requested stderr pipe"));
    let stdin = child.stdin.take();
    let feed = async {
        match (input, stdin) {
            (Some(secret), Some(mut pipe)) => {
                let failed = |error: io::Error| {
                    MutationFailure::Ordinary(format!(
                        "Failed to write to stdin of {executable}: {error}"
                    ))
                };
                pipe.write_all(secret.expose().as_bytes())
                    .await
                    .map_err(failed)?;
                pipe.shutdown().await.map_err(failed)
            }
            (None, None) => Ok(()),
            _ => Err(MutationFailure::Typed(AppError::new(
                AppErrorKind::Invariant,
                "keyring stdin pipe mismatch",
            ))),
        }
    };
    let out = async {
        let mut bytes = Vec::new();
        stdout
            .read_to_end(&mut bytes)
            .await
            .map_err(|error| MutationFailure::Ordinary(error.to_string()))?;
        Ok::<_, MutationFailure>(bytes)
    };
    let err = async {
        let mut bytes = Vec::new();
        stderr
            .read_to_end(&mut bytes)
            .await
            .map_err(|error| MutationFailure::Ordinary(error.to_string()))?;
        Ok::<_, MutationFailure>(bytes)
    };
    let wait = async {
        child
            .wait()
            .await
            .map_err(|error| MutationFailure::Ordinary(error.to_string()))
    };
    let result = tokio::try_join!(feed, out, err, wait);
    match result {
        Ok(((), stdout, stderr, status)) => {
            owned.child = None; // wait completed, fully drained and reaped.
            let completion = completion(status)?;
            let decoded = String::from_utf8_lossy(&stderr);
            Ok(ProcessReply {
                completion,
                stdout,
                stderr: js_trim(decoded.strip_prefix('\u{feff}').unwrap_or(&decoded)).to_owned(),
            })
        }
        Err(failure) => {
            owned.cleanup().await;
            Err(failure)
        }
    }
}

pub struct ProcessMutationBackend {
    pub overlay: ChildEnvOverlay,
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
impl crate::auth::mutation::CredentialMutationBackend for ProcessMutationBackend {
    async fn available(&self) -> bool {
        #[cfg(target_os = "linux")]
        {
            run("secret-tool", &[], None, &self.overlay).await.is_ok()
        }
        #[cfg(target_os = "macos")]
        {
            true
        }
    }
    async fn store(&self, workspace: &str, secret: &ConfigSecret) -> Result<(), MutationFailure> {
        #[cfg(target_os = "linux")]
        let (exe, action, args, input) = (
            "secret-tool",
            "store",
            vec![
                "store".to_owned(),
                "--label".to_owned(),
                format!("linear-cli: {workspace}"),
                "service".to_owned(),
                "linear-cli".to_owned(),
                "account".to_owned(),
                workspace.to_owned(),
            ],
            Some(secret),
        );
        #[cfg(target_os = "macos")]
        let (exe, action, args, input) = (
            "/usr/bin/security",
            "add-generic-password",
            vec![
                "add-generic-password".to_owned(),
                "-a".to_owned(),
                workspace.to_owned(),
                "-s".to_owned(),
                "linear-cli".to_owned(),
                "-w".to_owned(),
                secret.expose().to_owned(),
                "-U".to_owned(),
            ],
            None,
        );
        let reply = run(exe, &args, input, &self.overlay).await?;
        let code = reply.completion.source_code()?;
        if code != 0 {
            return Err(MutationFailure::Ordinary(format!(
                "{} {action} failed (exit {}): {}",
                if exe == "secret-tool" {
                    "secret-tool"
                } else {
                    "security"
                },
                code,
                reply.stderr
            )));
        }
        Ok(())
    }
    async fn delete(&self, workspace: &str) -> Result<(), MutationFailure> {
        #[cfg(target_os = "linux")]
        let (exe, action, args) = (
            "secret-tool",
            "clear",
            vec![
                "clear".to_owned(),
                "service".to_owned(),
                "linear-cli".to_owned(),
                "account".to_owned(),
                workspace.to_owned(),
            ],
        );
        #[cfg(target_os = "macos")]
        let (exe, action, args) = (
            "/usr/bin/security",
            "delete-generic-password",
            vec![
                "delete-generic-password".to_owned(),
                "-a".to_owned(),
                workspace.to_owned(),
                "-s".to_owned(),
                "linear-cli".to_owned(),
            ],
        );
        let reply = run(exe, &args, None, &self.overlay).await?;
        let code = reply.completion.source_code()?;
        let accepted = code == 0 || (cfg!(target_os = "macos") && code == 44);
        if !accepted {
            return Err(MutationFailure::Ordinary(format!(
                "{} {action} failed (exit {}): {}",
                if exe == "secret-tool" {
                    "secret-tool"
                } else {
                    "security"
                },
                code,
                reply.stderr
            )));
        }
        Ok(())
    }
}
