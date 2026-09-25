//! Explicit platform opener shared by URL-opening commands.
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio};

use crate::error::{AppError, AppErrorKind};

/// Wait for the platform opener and discard its output. The caller prints its
/// own opening line before invoking this function.
pub fn open(url: &str, app: bool) -> Result<(), AppError> {
    #[cfg(target_os = "linux")]
    let mut command = {
        let _ = app;
        let mut command = Command::new("xdg-open");
        command.arg(url);
        command
    };
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        if app {
            command.args(["-a", "Linear"]);
        }
        command.arg(url);
        command
    };
    #[cfg(target_os = "windows")]
    let mut command = {
        let _ = app;
        let mut command = Command::new("explorer.exe");
        command.arg(url);
        command
    };
    let program = command.get_program().to_string_lossy().into_owned();
    let status = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| {
            let message = if error.kind() == std::io::ErrorKind::NotFound {
                format!("Failed to spawn '{program}': entity not found")
            } else {
                format!("Failed to spawn '{program}': {error}")
            };
            AppError::new(AppErrorKind::IoProcess, message).with_source(error)
        })?;
    // Explorer commonly reports exit code 1 after handing a URL to the shell.
    // Its process exit does not tell us whether the handoff succeeded.
    #[cfg(target_os = "windows")]
    {
        let _ = status;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    if status.success() {
        Ok(())
    } else {
        let code = status
            .code()
            .or_else(|| status.signal().and_then(|signal| signal.checked_add(128)))
            .map_or_else(|| "signal".to_owned(), |code| code.to_string());
        Err(AppError::new(
            AppErrorKind::IoProcess,
            format!("Failed to open {url} (exit code: {code})"),
        ))
    }
}
