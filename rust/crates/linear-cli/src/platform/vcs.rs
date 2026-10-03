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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jj_trailers_use_the_last_identifier_of_the_first_block() {
        for (text, expected) in [
            ("Fixes ABC-123Fixes DEF-456", Some("DEF-456")),
            ("Fixes ABC-123 References DEF-456", Some("ABC-123")),
            (
                "Fixes ABC-123\nFixes DEF-456\n\nFixes XYZ-9",
                Some("DEF-456"),
            ),
            ("\n\nNo issue\nFixes XYZ-9", Some("XYZ-9")),
            ("", None),
            (" ENG-7 ", Some("ENG-7")),
        ] {
            assert_eq!(parse_jj_trailers(text).as_deref(), expected, "{text:?}");
        }
    }

    #[test]
    fn exit_one_is_a_detached_head_and_other_failures_are_errors() {
        assert_eq!(
            parse_git_branch(Some(1), "ENG-7", "").expect("detached"),
            None
        );
        assert_eq!(
            parse_git_branch(Some(0), " feature/eng-7-x\n", "warning")
                .expect("branch")
                .as_deref(),
            Some("ENG-7")
        );
        assert_eq!(parse_git_branch(Some(0), "\n", "").expect("empty"), None);
        let error = parse_git_branch(Some(128), "ENG-7", " fatal: denied\n").expect_err("fatal");
        assert_eq!(
            error.message(),
            "Failed to get current branch: fatal: denied"
        );
    }
}
