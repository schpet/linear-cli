//! Issue reference precedence before credential selection or VCS inference.
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
        match expect_url_kind(
            input,
            LinearUrlKind::Issue,
            "an issue URL or an identifier like ENG-123",
            scope,
        )? {
            Some(LinearUrlRef::Issue { identifier, .. }) => {
                return Ok(Some(identifier));
            }
            Some(other) => unreachable!("expect_url_kind returned a {:?} URL", other.kind()),
            None => {}
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
