//! The GitHub CLI (`gh`), run attached to the terminal so it can prompt and
//! report its own errors.
use std::ffi::OsStr;
use std::path::Path;
use std::process::Stdio;

use crate::config::ChildEnvOverlay;
use crate::error::Result;
use crate::platform::process;

/// Runs `gh` with `args`, exiting with its status when it fails.
pub fn run(
    args: impl IntoIterator<Item = impl AsRef<OsStr>>,
    cwd: &Path,
    env: &ChildEnvOverlay,
) -> Result<()> {
    let mut command = process::command("gh", cwd, env);
    command.args(args).stdin(Stdio::inherit());
    process::exit_like(process::status(&mut command)?)
}
