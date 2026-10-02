//! Paging long terminal output.
//!
//! Output longer than the terminal goes through `PAGER`, run by the shell as
//! git does, or `less -FRX` when `PAGER` is unset. Quitting the pager early is
//! a normal way to stop reading, not a failure.
use std::ffi::OsStr;
use std::io::{self, Write};
use std::process::{Command, Stdio};

use crate::config::ChildEnvOverlay;
use crate::error::{Error, Result};

/// Line limit used when a terminal reports no size.
const UNKNOWN_SIZE_LINE_LIMIT: usize = 50;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalSize {
    pub columns: u16,
    pub rows: u16,
}

/// The stdout terminal size, or `None` when stdout is not a terminal or
/// reports a zero dimension.
pub fn stdout_size() -> Option<TerminalSize> {
    terminal_size::terminal_size_of(io::stdout())
        .map(
            |(terminal_size::Width(columns), terminal_size::Height(rows))| TerminalSize {
                columns,
                rows,
            },
        )
        .filter(|size| size.columns > 0 && size.rows > 0)
}

/// Whether `rendered` is too long to show on one terminal screen.
pub fn too_long(rendered: &str, size: Option<TerminalSize>) -> bool {
    let lines = rendered.lines().count();
    match size {
        Some(size) => lines > usize::from(size.rows).saturating_sub(2),
        None => lines > UNKNOWN_SIZE_LINE_LIMIT,
    }
}

pub enum Paged {
    Shown,
    /// No pager could be started; the caller prints the text itself.
    NoPager,
}

/// Feeds `text` to the pager on stdin and waits for it to exit.
pub fn page(text: &str, pager: Option<&OsStr>, env: &ChildEnvOverlay) -> Result<Paged> {
    let mut command = pager_command(pager.filter(|value| !value.is_empty()));
    command
        .envs(env.iter())
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Paged::NoPager),
        Err(error) => {
            return Err(Error::new(format!("Failed to start pager: {error}")).with_source(error));
        }
    };
    let written = child
        .stdin
        .take()
        .map_or(Ok(()), |mut stdin| stdin.write_all(text.as_bytes()));
    let status = child.wait().map_err(|error| {
        Error::new(format!("Failed to wait for pager: {error}")).with_source(error)
    })?;
    match written {
        Err(error) if error.kind() != io::ErrorKind::BrokenPipe => {
            Err(Error::new(format!("Failed to write to pager: {error}")).with_source(error))
        }
        // The shell's "command not found" status: the pager never ran.
        Ok(()) | Err(_) if status.code() == Some(127) => Ok(Paged::NoPager),
        Ok(()) | Err(_) => Ok(Paged::Shown),
    }
}

#[cfg(unix)]
fn pager_command(pager: Option<&OsStr>) -> Command {
    let mut command = Command::new("sh");
    command.arg("-c");
    match pager {
        Some(pager) => command.arg(pager),
        None => command.arg("less -FRX"),
    };
    command
}

#[cfg(not(unix))]
fn pager_command(pager: Option<&OsStr>) -> Command {
    match pager {
        Some(pager) => {
            let mut command = Command::new("cmd");
            command.arg("/C").arg(pager);
            command
        }
        None => Command::new("more"),
    }
}
