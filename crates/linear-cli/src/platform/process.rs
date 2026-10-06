//! Running external programs (git, jj, gh, editors) and reading their results.
use std::io;
use std::num::NonZeroU8;
use std::path::Path;
use std::process::{Command, ExitStatus, Output, Stdio};

use crate::config::ChildEnvOverlay;
use crate::error::{Error, Failure, Result};

/// `program` run in `cwd` with the configured child environment on top of
/// this process's own, and with stdin closed.
pub fn command(program: &str, cwd: &Path, env: &ChildEnvOverlay) -> Command {
    let mut command = Command::new(program);
    command
        .current_dir(cwd)
        .envs(env.iter())
        .stdin(Stdio::null());
    command
}

/// Runs `command` and captures its stdout and stderr.
pub fn output(command: &mut Command) -> Result<Output> {
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| spawn_error(command, error))
}

/// Runs `command` with stdout and stderr on this process's own.
pub fn status(command: &mut Command) -> Result<ExitStatus> {
    command
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|error| spawn_error(command, error))
}

/// Captures `command`'s output, failing with its stderr unless it succeeds.
pub fn checked_output(command: &mut Command) -> Result<Output> {
    let output = output(command)?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(failed(command, output.status, &output.stderr))
    }
}

/// The error for `command` exiting unsuccessfully, quoting what it wrote to stderr.
pub fn failed(command: &Command, status: ExitStatus, stderr: &[u8]) -> Error {
    let program = name(command);
    let stderr = text(stderr);
    let message = if stderr.is_empty() {
        format!("`{program}` failed ({status})")
    } else {
        format!("`{program}` failed ({status}): {stderr}")
    };
    Error::new(message)
}

/// Fails quietly when a child attached to the terminal failed, since it
/// reported its own failure there: with status 1, or 128 plus the signal
/// number for a child killed by a signal (130 for Ctrl-C), as a child that
/// exits 130 itself was interrupted too.
pub fn check_attached(status: ExitStatus) -> Result<()> {
    if status.success() {
        return Ok(());
    }
    let interrupted = match status.code() {
        Some(130) => NonZeroU8::new(130),
        Some(_) => None,
        None => signal(status)
            .and_then(|signal| signal.checked_add(128))
            .and_then(|code| u8::try_from(code).ok())
            .and_then(NonZeroU8::new),
    };
    Err(interrupted.map_or_else(|| Error::reported(Failure::General), Error::exit))
}

#[cfg(unix)]
fn signal(status: ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status.signal()
}

#[cfg(not(unix))]
fn signal(_status: ExitStatus) -> Option<i32> {
    None
}

/// Program output as text: invalid UTF-8 replaced, a leading byte-order mark
/// dropped, and surrounding whitespace trimmed.
pub fn text(bytes: &[u8]) -> String {
    let decoded = String::from_utf8_lossy(bytes);
    decoded
        .strip_prefix('\u{feff}')
        .unwrap_or(&decoded)
        .trim()
        .to_owned()
}

fn name(command: &Command) -> String {
    command.get_program().to_string_lossy().into_owned()
}

fn spawn_error(command: &Command, error: io::Error) -> Error {
    let program = name(command);
    if error.kind() == io::ErrorKind::NotFound {
        let hint = match program.as_str() {
            "gh" => "Install the GitHub CLI from https://cli.github.com.".to_owned(),
            "jj" => "Install jj from https://jj-vcs.github.io/jj.".to_owned(),
            _ => format!("Install {program} or add it to PATH."),
        };
        Error::new(format!("Could not find `{program}`"))
            .with_hint(hint)
            .with_source(error)
    } else {
        Error::new(format!("Failed to run `{program}`: {error}")).with_source(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_drops_a_bom_replaces_invalid_bytes_and_trims() {
        assert_eq!(text(b"\xef\xbb\xbf  ENG-7 \n"), "ENG-7");
        assert_eq!(text(b"\xef\xbb\xbf \xff \xc2\x85"), "\u{fffd}");
    }

    #[cfg(unix)]
    #[test]
    fn a_failed_attached_child_fails_with_status_1_unless_interrupted() {
        use std::os::unix::process::ExitStatusExt;
        for (raw, expected) in [
            (0, 0),
            (7 << 8, 1),
            (255 << 8, 1),
            (130 << 8, 130),
            (2, 130),
            (15, 143),
        ] {
            let status = ExitStatus::from_raw(raw);
            let code = check_attached(status).map_or_else(|error| error.exit_code(), |()| 0);
            assert_eq!(code, expected, "{status:?}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn failures_quote_stderr() {
        use std::os::unix::process::ExitStatusExt;
        let command = Command::new("git");
        let status = ExitStatus::from_raw(1 << 8);
        assert_eq!(
            failed(&command, status, b" fatal: not a branch\n").message(),
            "`git` failed (exit status: 1): fatal: not a branch"
        );
        assert_eq!(
            failed(&command, status, b"").message(),
            "`git` failed (exit status: 1)"
        );
    }

    #[test]
    fn a_missing_program_is_named() {
        let error = output(&mut Command::new("linear-cli-test-missing-program"))
            .expect_err("missing program");
        assert_eq!(
            error.message(),
            "Could not find `linear-cli-test-missing-program`"
        );
    }
}
