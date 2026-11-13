//! Issue reference precedence before credential selection.
use super::url::issue_identifier;
use super::{LinearUrlKind, LinearUrlRef, WorkspaceScope, expect_url_kind};
use crate::error::Error;

/// The normalized identifier, or `None` when the input names no issue.
pub fn prepare_issue_reference(
    input: &str,
    team_key: Option<&str>,
    scope: &WorkspaceScope<'_>,
) -> Result<Option<String>, Error> {
    if !input.is_empty() {
        if let Some(identifier) = expect_url_kind(
            input,
            LinearUrlKind::Issue,
            "an issue URL or an identifier like ENG-123",
            scope,
            |url| match url {
                LinearUrlRef::Issue { identifier, .. } => Some(identifier),
                _ => None,
            },
        )? {
            return Ok(Some(identifier));
        }
        if let Some(id) = issue_identifier(input) {
            return Ok(Some(id));
        }
    }
    if input.starts_with(|c: char| ('1'..='9').contains(&c))
        && input.bytes().all(|b| b.is_ascii_digit())
    {
        let team = team_key.ok_or_else(|| {
            Error::new(format!("Issue number {input} needs a team"))
                .with_hint("Pass a full identifier like ENG-123, or run `linear config` to set a default team.")
        })?;
        return Ok(issue_identifier(&format!(
            "{}-{input}",
            team.to_uppercase()
        )));
    }
    Ok(None)
}

#[cfg(test)]
mod tests;
