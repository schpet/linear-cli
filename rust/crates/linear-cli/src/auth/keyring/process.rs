//! Running a keyring command-line tool (`secret-tool`, `security`) with a
//! deadline.
use std::ffi::OsString;
use std::io;
use std::process::{Output, Stdio};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

use super::{LookupFailureCategory, LookupResult};
use crate::config::{ChildEnvOverlay, ConfigSecret};
use crate::error::Error;

/// How long one keyring command may run before it is killed.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// How long output may keep arriving after the tool exits. A pipe stays open
/// past that only when another process inherited it, and that process never
/// writes the tool's output.
const PIPE_GRACE: Duration = Duration::from_millis(500);

#[derive(Debug)]
pub enum RunError {
    Runtime(io::Error),
    Spawn(io::Error),
    Io(io::Error),
    Timeout,
}

impl RunError {
    pub fn category(&self) -> LookupFailureCategory {
        match self {
            Self::Spawn(error) if error.kind() == io::ErrorKind::NotFound => {
                LookupFailureCategory::Unavailable
            }
            Self::Spawn(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                LookupFailureCategory::Permission
            }
            Self::Runtime(_) | Self::Spawn(_) | Self::Io(_) | Self::Timeout => {
                LookupFailureCategory::Other
            }
        }
    }
}

/// A keyring tool: the executable and the environment it runs with.
pub struct Tool {
    pub name: &'static str,
    pub executable: OsString,
    pub overlay: ChildEnvOverlay,
    pub timeout: Duration,
}

impl Tool {
    pub fn new(name: &'static str, executable: &str, overlay: ChildEnvOverlay) -> Self {
        Self {
            name,
            executable: OsString::from(executable),
            overlay,
            timeout: DEFAULT_TIMEOUT,
        }
    }

    /// Runs the tool to completion with `args`, feeding `input` on stdin.
    ///
    /// Callers may or may not be inside the command's async runtime, so the
    /// tool runs on a private runtime on its own thread. The child is killed
    /// if it outlives the timeout.
    pub fn run(&self, args: &[String], input: Option<&[u8]>) -> Result<Output, RunError> {
        std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(RunError::Runtime)?;
                    let mut command = Command::new(&self.executable);
                    command.args(args).envs(self.overlay.iter());
                    runtime.block_on(run(&mut command, input, self.timeout))
                })
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
        })
    }

    /// [`Tool::run`] for storing or deleting, with failures as user-facing
    /// errors.
    pub fn run_change(
        &self,
        args: &[String],
        input: Option<&[u8]>,
        install_hint: &str,
    ) -> Result<Output, Error> {
        if args.iter().any(|arg| arg.contains('\0')) {
            return Err(Error::new(
                "Keyring arguments cannot contain a NUL character",
            ));
        }
        self.run(args, input).map_err(|error| match error {
            RunError::Spawn(error) => Error::new(format!(
                "Could not run {}: {error}",
                self.executable.to_string_lossy()
            ))
            .with_hint(install_hint)
            .with_source(error),
            RunError::Runtime(error) | RunError::Io(error) => {
                Error::new(format!("{} failed: {error}", self.name)).with_source(error)
            }
            RunError::Timeout => Error::new(format!(
                "{} did not finish within {} seconds",
                self.name,
                self.timeout.as_secs()
            )),
        })
    }

    /// Fails unless the tool exited with one of `accepted`.
    pub fn check(&self, output: &Output, action: &str, accepted: &[i32]) -> Result<(), Error> {
        let code = output.status.code();
        if code.is_some_and(|code| accepted.contains(&code)) {
            return Ok(());
        }
        let status = code.map_or_else(|| output.status.to_string(), |code| format!("exit {code}"));
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(Error::new(format!(
            "{} {action} failed ({status}): {}",
            self.name,
            stderr.trim()
        )))
    }
}

/// Runs `command` to completion, feeding `input` on stdin.
///
/// The child is killed if it outlives `timeout` or the caller is dropped.
async fn run(
    command: &mut Command,
    input: Option<&[u8]>,
    timeout: Duration,
) -> Result<Output, RunError> {
    command
        .kill_on_drop(true)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(RunError::Spawn)?;
    let stdin = child.stdin.take();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let exchange = async {
        if let (Some(input), Some(mut stdin)) = (input, stdin) {
            stdin.write_all(input).await.map_err(RunError::Io)?;
        }
        let mut stdout_bytes = Vec::new();
        let mut stderr_bytes = Vec::new();
        let status = {
            let reads = async {
                tokio::try_join!(
                    read_into(stdout, &mut stdout_bytes),
                    read_into(stderr, &mut stderr_bytes)
                )
            };
            tokio::pin!(reads);
            let mut drained = false;
            let status = tokio::select! {
                status = child.wait() => status,
                output = &mut reads => {
                    output.map_err(RunError::Io)?;
                    drained = true;
                    child.wait().await
                }
            }
            .map_err(RunError::Io)?;
            if !drained && let Ok(output) = tokio::time::timeout(PIPE_GRACE, reads).await {
                output.map_err(RunError::Io)?;
            }
            status
        };
        Ok(Output {
            status,
            stdout: stdout_bytes,
            stderr: stderr_bytes,
        })
    };
    match tokio::time::timeout(timeout, exchange).await {
        Ok(result) => result,
        Err(_elapsed) => {
            // Kill and reap so no zombie outlives the lookup.
            let _ = child.start_kill();
            let _ = child.wait().await;
            Err(RunError::Timeout)
        }
    }
}

/// Appends everything read from `pipe` to `bytes`, which keeps whatever
/// arrived if this future is dropped early.
async fn read_into(pipe: Option<impl AsyncRead + Unpin>, bytes: &mut Vec<u8>) -> io::Result<()> {
    let Some(mut pipe) = pipe else {
        return Ok(());
    };
    let mut chunk = [0_u8; 4096];
    loop {
        let count = pipe.read(&mut chunk).await?;
        if count == 0 {
            return Ok(());
        }
        bytes.extend_from_slice(chunk.get(..count).unwrap_or_default());
    }
}

/// The key a successful lookup printed. Surrounding whitespace (such as
/// `security -w`'s trailing newline) is dropped, and nothing left is a miss.
pub fn printed_key(stdout: Vec<u8>) -> LookupResult {
    let Ok(stdout) = String::from_utf8(stdout) else {
        return LookupResult::Failed(LookupFailureCategory::Other);
    };
    match stdout.trim() {
        "" => LookupResult::Miss,
        key => LookupResult::Hit(ConfigSecret::new(key.to_owned())),
    }
}

#[cfg(test)]
mod tests;
