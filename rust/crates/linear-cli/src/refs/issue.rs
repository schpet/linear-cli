//! Issue reference precedence before credential selection or VCS inference.
use super::url::issue_identifier;
use super::{LinearUrlKind, LinearUrlRef, WorkspaceScope, expect_url_kind};
use crate::error::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IssueReference {
    Identifier(String),
    Inferred,
    Unresolved,
}

pub fn prepare_issue_reference(
    input: Option<&str>,
    team_key: Option<&str>,
    scope: &WorkspaceScope<'_>,
) -> Result<IssueReference, Error> {
    let Some(input) = input else {
        return Ok(IssueReference::Inferred);
    };
    if !input.is_empty() {
        match expect_url_kind(
            input,
            LinearUrlKind::Issue,
            "an issue URL or an identifier like ENG-123",
            scope,
        )? {
            Some(LinearUrlRef::Issue { identifier, .. }) => {
                return Ok(IssueReference::Identifier(identifier));
            }
            Some(other) => unreachable!("expect_url_kind returned a {:?} URL", other.kind()),
            None => {}
        }
        if let Some(id) = issue_identifier(input) {
            return Ok(IssueReference::Identifier(id));
        }
    }
    if input.starts_with(|c: char| ('1'..='9').contains(&c))
        && input.bytes().all(|b| b.is_ascii_digit())
    {
        let team = team_key.filter(|value| !value.is_empty()).ok_or_else(|| {
            Error::new("an integer id was provided, but no team is set")
                .with_hint("Run `linear config` to set a team.")
        })?;
        return Ok(
            issue_identifier(&format!("{}-{input}", team.to_uppercase()))
                .map(IssueReference::Identifier)
                .unwrap_or(IssueReference::Unresolved),
        );
    }
    Ok(IssueReference::Unresolved)
}

/// Finds the first `TEAM-123` identifier that starts and ends on a word
/// boundary, where word bytes are ASCII letters, digits and `_`.
pub fn find_issue_identifier(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    for (start, byte) in bytes.iter().enumerate() {
        if !byte.is_ascii_alphanumeric()
            || start
                .checked_sub(1)
                .and_then(|index| bytes.get(index))
                .is_some_and(|previous| word(*previous))
        {
            continue;
        }
        let mut dash = start;
        while bytes.get(dash).is_some_and(u8::is_ascii_alphanumeric) {
            dash += 1;
        }
        if bytes.get(dash) != Some(&b'-')
            || !bytes
                .get(dash + 1)
                .is_some_and(|b| (b'1'..=b'9').contains(b))
        {
            continue;
        }
        let mut end = dash + 2;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
        if bytes.get(end).is_some_and(|b| word(*b)) {
            continue;
        }
        // Both ends are ASCII bytes, hence UTF-8 boundaries.
        return issue_identifier(&text[start..end]);
    }
    None
}

#[cfg(test)]
mod tests;
