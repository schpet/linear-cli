//! Issue reference precedence before credential selection.
use super::url::{LinearUrlParse, issue_identifier, parse_linear_url};
use super::{LinearUrlKind, LinearUrlRef, WorkspaceScope, expect_url_kind};
use crate::error::Error;

/// Data for displaying a fetched issue. Its URL identifies the workspace even
/// when the same team key and issue number exist in several workspaces.
#[derive(Debug, serde::Serialize)]
pub struct IssueReference<'a> {
    pub identifier: &'a str,
    pub url: Option<&'a str>,
    pub workspace_url_key: Option<String>,
}

impl<'a> IssueReference<'a> {
    pub fn new(identifier: &'a str, url: Option<&'a str>) -> Self {
        let url = url.filter(|value| !value.is_empty());
        let workspace_url_key = match url.map(parse_linear_url) {
            Some(LinearUrlParse::Known(LinearUrlRef::Issue { workspace, .. })) => Some(workspace),
            _ => None,
        };
        Self {
            identifier,
            url,
            workspace_url_key,
        }
    }
}

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
