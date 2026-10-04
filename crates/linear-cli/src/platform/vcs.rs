//! Reading the issue the current git branch or jj change names.
use std::path::Path;

use crate::config::{ChildEnvOverlay, Vcs};
use crate::error::{Error, Result};
use crate::platform::process;
use crate::refs::find_issue_identifier;

/// Prints the value of each `Linear-issue` trailer on its own line, with a
/// blank line after each change.
const JJ_TRAILERS: &str =
    "trailers.map(|t| if(t.key() == \"Linear-issue\", t.value() ++ \"\\n\")).join(\"\") ++ \"\\n\"";

/// The issue the current git branch name or the nearest jj change with a
/// `Linear-issue` trailer names.
pub fn infer_issue(vcs: Vcs, cwd: &Path, env: &ChildEnvOverlay) -> Result<Option<String>> {
    read_issue(vcs, cwd, env).map_err(|error| {
        if in_repository(vcs, cwd) {
            error
        } else {
            not_in_repository(vcs)
                .with_hint("Pass an issue ID like ENG-123, or run from inside the repository.")
        }
    })
}

/// Whether `cwd` is inside a repository of the `vcs` kind: a `.git` (a
/// directory, or the file a worktree has) or a `.jj` directory there or in a
/// parent. Only used to explain a failed git or jj command, so setups such
/// as `GIT_DIR` that work without one are unaffected.
pub fn in_repository(vcs: Vcs, cwd: &Path) -> bool {
    cwd.ancestors().any(|dir| match vcs {
        Vcs::Git => dir.join(".git").exists(),
        Vcs::Jj => dir.join(".jj").is_dir(),
    })
}

/// The error for a command that needs a repository run outside one.
pub fn not_in_repository(vcs: Vcs) -> Error {
    let kind = match vcs {
        Vcs::Git => "git",
        Vcs::Jj => "jj",
    };
    Error::new(format!("Not in a {kind} repository"))
}

fn read_issue(vcs: Vcs, cwd: &Path, env: &ChildEnvOverlay) -> Result<Option<String>> {
    match vcs {
        Vcs::Git => {
            let mut command = process::command("git", cwd, env);
            command.args(["symbolic-ref", "--quiet", "--short", "HEAD"]);
            let output = process::output(&mut command)?;
            match output.status.code() {
                Some(0) => Ok(find_issue_identifier(&process::text(&output.stdout))),
                // A detached HEAD has no branch, so no issue.
                Some(1) => Ok(None),
                _ => Err(process::failed(&command, output.status, &output.stderr)
                    .context("Failed to get current branch")),
            }
        }
        Vcs::Jj => {
            let mut command = process::command("jj", cwd, env);
            command.args(["log", "-r", "::@", "--no-graph", "-T", JJ_TRAILERS]);
            let output = process::checked_output(&mut command)
                .map_err(|error| error.context("Failed to read jj trailers"))?;
            Ok(parse_jj_trailers(&String::from_utf8_lossy(&output.stdout)))
        }
    }
}

/// The issue in the last `Linear-issue` trailer of the first change that has
/// one, given blank-line-separated blocks of trailer values.
fn parse_jj_trailers(output: &str) -> Option<String> {
    output.split("\n\n").find_map(|block| {
        block
            .lines()
            .filter_map(|line| find_issue_identifier(line.trim()))
            .next_back()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jj_trailers_use_the_last_identifier_of_the_nearest_change() {
        for (text, expected) in [
            (
                "Fixes ABC-123\nFixes DEF-456\n\nFixes XYZ-9\n\n",
                Some("DEF-456"),
            ),
            ("\n\n\nNo issue\nFixes XYZ-9\n\n", Some("XYZ-9")),
            ("References ABC-1\n\n", Some("ABC-1")),
            ("\n\n\n", None),
            ("", None),
        ] {
            assert_eq!(parse_jj_trailers(text).as_deref(), expected, "{text:?}");
        }
    }
}
