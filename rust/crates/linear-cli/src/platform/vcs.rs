//! Reading an issue identifier out of a git branch name or jj trailers.
use crate::error::Error;
use crate::refs::find_issue_identifier;

pub const JJ_TEMPLATE: &str = "trailers.map(|t| if(t.key() == \"Linear-issue\", t.value(), \"\"))";

pub fn parse_jj_trailers(output: &str) -> Option<String> {
    let mut last = None;
    for line in output.split('\n') {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if last.is_some() {
                return last;
            }
        } else if let Some(id) = find_issue_identifier(trimmed) {
            last = Some(id);
        }
    }
    last
}

/// The issue in the branch name printed by `git symbolic-ref --quiet --short
/// HEAD`, which exits with `exit_code` 1 when HEAD is detached (no branch, so
/// no issue) and with another nonzero status on a real failure.
pub fn parse_git_branch(
    exit_code: Option<i32>,
    stdout: &str,
    stderr: &str,
) -> Result<Option<String>, Error> {
    match exit_code {
        Some(0) => Ok(find_issue_identifier(stdout.trim())),
        Some(1) => Ok(None),
        Some(_) | None => Err(Error::new(format!(
            "Failed to get current branch: {}",
            stderr.trim()
        ))),
    }
}
