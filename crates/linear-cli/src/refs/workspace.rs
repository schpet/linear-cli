use crate::auth::ApiKeyInput;
use crate::error::Error;

use super::url::{LinearUrlKind, LinearUrlParse, LinearUrlRef, parse_linear_url};

/// The active workspace, for checking that Linear URLs belong to it. No
/// process state is read.
pub struct WorkspaceScope<'a> {
    pub workspace: Option<&'a str>,
    pub api_key: ApiKeyInput<'a>,
}

impl<'a> WorkspaceScope<'a> {
    pub fn new(workspace: Option<&'a str>, api_key: ApiKeyInput<'a>) -> Self {
        Self { workspace, api_key }
    }

    fn effective_workspace(&self) -> Option<&str> {
        let trimmed = self.workspace?.trim();
        (!trimmed.is_empty()).then_some(trimmed)
    }

    fn switch_suggestion(&self, url_workspace: &str, current: &str) -> String {
        let from_url = format!("or use a URL from \"{current}\".");
        match self.api_key {
            ApiKeyInput::Raw { .. } => format!(
                "LINEAR_API_KEY is set, and the CLI won't combine it with --workspace. Unset it and pass --workspace {url_workspace}, {from_url}"
            ),
            ApiKeyInput::Sourced { .. } => format!(
                "The api_key in your config outranks --workspace. Remove it to pass --workspace {url_workspace}, {from_url}"
            ),
            ApiKeyInput::Absent => format!("Pass --workspace {url_workspace}, {from_url}"),
        }
    }

    fn check(&self, url_workspace: &str) -> Result<(), Error> {
        let Some(current) = self.effective_workspace() else {
            return Ok(());
        };
        if current.to_lowercase() == url_workspace.to_lowercase() {
            return Ok(());
        }
        Err(Error::new(format!(
                "That URL is for the \"{url_workspace}\" workspace, but this is the \"{current}\" workspace."
            ),
        )
        .with_hint(self.switch_suggestion(url_workspace, current)))
    }
}

/// Return `None` for ordinary input, or the payload of a URL of the requested kind.
/// URL refusal precedes workspace checking; workspace checking precedes kind checking.
pub fn expect_url_kind<T>(
    input: &str,
    kind: LinearUrlKind,
    entity_label: &str,
    scope: &WorkspaceScope<'_>,
    payload: impl FnOnce(LinearUrlRef) -> Option<T>,
) -> Result<Option<T>, Error> {
    let suggestion = || format!("Pass {entity_label}.");
    let parsed = match parse_linear_url(input) {
        LinearUrlParse::NotLinear => return Ok(None),
        LinearUrlParse::Unsupported(reason) => {
            return Err(
                Error::new(format!("\"{input}\" is a Linear URL, but {reason}."))
                    .with_hint(suggestion()),
            );
        }
        LinearUrlParse::Known(reference) => reference,
    };
    scope.check(parsed.workspace())?;
    if parsed.kind() != kind {
        return Err(Error::new(format!(
            "\"{input}\" is {} URL, not {} URL.",
            parsed.kind().label(),
            kind.label()
        ))
        .with_hint(suggestion()));
    }
    Ok(Some(
        payload(parsed).expect("the URL payload picker matches the checked kind"),
    ))
}

/// Reject any recognized Linear URL for commands that accept only plain references.
pub fn reject_linear_url(input: &str, entity_label: &str) -> Result<(), Error> {
    if matches!(parse_linear_url(input), LinearUrlParse::NotLinear) {
        return Ok(());
    }
    Err(Error::new(format!(
        "\"{input}\" is a Linear URL, and this command does not take one."
    ))
    .with_hint(format!("Pass {entity_label}.")))
}

/// Reject an issue comment link in any workspace: its anchor carries only the
/// first eight characters of the comment id. Other URLs, including issue URLs
/// with a non-comment anchor, are left to `reject_linear_url`.
pub fn reject_comment_url(input: &str) -> Result<(), Error> {
    if !matches!(
        parse_linear_url(input),
        LinearUrlParse::Known(LinearUrlRef::Issue {
            comment_id_prefix: Some(_),
            ..
        })
    ) {
        return Ok(());
    }
    Err(Error::new(format!(
            "\"{input}\" links to a comment, but a comment URL only carries the first eight characters of its ID."
        ),
    )
    .with_hint("Pass the comment's full UUID, from `linear issue comment list <issue> --json`."))
}

/// Prepare a team URL for the later GraphQL resolver without selecting credentials.
pub fn expect_team_url(input: &str, scope: &WorkspaceScope<'_>) -> Result<Option<String>, Error> {
    expect_url_kind(
        input,
        LinearUrlKind::Team,
        "a team URL, key, name, or ID",
        scope,
        |url| match url {
            LinearUrlRef::Team { team_key, .. } => Some(team_key),
            _ => None,
        },
    )
}

#[cfg(test)]
mod tests;
