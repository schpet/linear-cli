//! The GitHub CLI (`gh`), run attached to the terminal so it can prompt and
//! report its own errors.
use std::ffi::OsStr;
use std::path::Path;
use std::process::Stdio;

use crate::config::ChildEnvOverlay;
use crate::error::{Error, Result};
use crate::platform::process;

/// Runs `gh` with `args`. When it fails it has said why on stderr, and the
/// command fails with status 1 (130 when it was cancelled or interrupted),
/// not with gh's own status codes.
pub fn run(
    args: impl IntoIterator<Item = impl AsRef<OsStr>>,
    cwd: &Path,
    env: &ChildEnvOverlay,
) -> Result<()> {
    let mut command = process::command("gh", cwd, env);
    command.args(args).stdin(Stdio::inherit());
    let status = process::status(&mut command)?;
    // gh exits 2 when one of its prompts is cancelled.
    if status.code() == Some(2) {
        return Err(Error::cancelled());
    }
    process::check_attached(status)
}
