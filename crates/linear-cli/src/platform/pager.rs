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

#[derive(Debug, Eq, PartialEq)]
pub enum Paged {
    Shown,
    /// No pager could be started; the caller prints the text itself.
    NoPager,
}

/// Feeds `text` to the pager on stdin and waits for it to exit.
///
/// The pager succeeded when it exits with status 0, including after the
/// reader quits early (which closes its input), or when it is stopped by an
/// interrupt, termination or hangup signal. Any other exit is reported: its
/// output, if any, cannot be trusted to have shown the text.
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
    if let Err(error) = written
        && error.kind() != io::ErrorKind::BrokenPipe
    {
        return Err(Error::new(format!("Failed to write to pager: {error}")).with_source(error));
    }
    let failed = |how: String| {
        Err(Error::new(format!("The pager {how}"))
            .with_hint("Check the PAGER environment variable, or pass --no-pager."))
    };
    match status.code() {
        Some(0) => Ok(Paged::Shown),
        // The shell's "command not found" status: the pager never ran.
        Some(127) => Ok(Paged::NoPager),
        Some(code) => failed(format!("exited with status {code}")),
        None if stopped_deliberately(status) => Ok(Paged::Shown),
        None => failed(format!("stopped abnormally ({status})")),
    }
}

/// Whether the pager was stopped by a signal a reader sends on purpose.
#[cfg(unix)]
fn stopped_deliberately(status: std::process::ExitStatus) -> bool {
    use std::os::unix::process::ExitStatusExt;
    status
        .signal()
        .is_some_and(|signal| [libc::SIGINT, libc::SIGTERM, libc::SIGHUP].contains(&signal))
}

#[cfg(not(unix))]
fn stopped_deliberately(_: std::process::ExitStatus) -> bool {
    false
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

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn run(pager: &str) -> Result<Paged> {
        let text = "line\n".repeat(20_000);
        page(&text, Some(OsStr::new(pager)), &ChildEnvOverlay::empty())
    }

    #[test]
    fn reading_everything_or_quitting_early_counts_as_shown() {
        assert_eq!(run("cat > /dev/null").expect("read all"), Paged::Shown);
        assert_eq!(
            run("head -n 1 > /dev/null").expect("quit early"),
            Paged::Shown
        );
        assert_eq!(run("kill -INT $$").expect("interrupted"), Paged::Shown);
    }

    #[test]
    fn a_missing_pager_falls_back_to_direct_output() {
        assert_eq!(
            run("definitely-not-a-pager-command").expect("not found"),
            Paged::NoPager
        );
    }

    #[test]
    fn failed_or_crashed_pagers_are_reported() {
        let error = run("exit 2").expect_err("bad exit");
        assert_eq!(error.message(), "The pager exited with status 2");
        assert!(error.hint().is_some_and(|hint| hint.contains("PAGER")));
        let error = run("kill -SEGV $$").expect_err("crash");
        assert!(
            error.message().starts_with("The pager stopped abnormally"),
            "{error}"
        );
    }
}
