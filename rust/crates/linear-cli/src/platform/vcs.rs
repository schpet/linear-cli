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

pub fn parse_git_branch(
    success: bool,
    stdout: &str,
    stderr: &str,
) -> Result<Option<String>, Error> {
    if !success {
        let error = stderr.trim();
        if error.contains("not a symbolic ref") {
            return Ok(None);
        }
        return Err(Error::new(format!("Failed to get current branch: {error}")));
    }
    Ok(find_issue_identifier(stdout.trim()))
}
