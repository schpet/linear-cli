use crate::auth::{ApiKeyInput, CredentialSelectionInputs, CredentialStore};
use crate::error::{AppError, AppErrorKind};
use crate::text::js_space;

use super::url::{LinearUrlKind, LinearUrlParse, LinearUrlRef, parse_linear_url};

/// Borrowed local inputs. Neither URL parsing nor the workspace check reads process state.
pub struct WorkspaceScope<'a> {
    pub cli_workspace: Option<&'a str>,
    pub sourced_workspace: Option<&'a str>,
    pub default_workspace: Option<&'a str>,
    pub api_key: &'a ApiKeyInput<'a>,
}

impl<'a> WorkspaceScope<'a> {
    pub fn from_selection(
        inputs: &'a CredentialSelectionInputs<'a>,
        store: &'a CredentialStore,
    ) -> Self {
        Self {
            cli_workspace: inputs.cli_workspace,
            sourced_workspace: inputs.sourced_workspace.as_ref().map(|(value, _)| *value),
            default_workspace: store.default(),
            api_key: &inputs.api_key,
        }
    }

    fn effective_workspace(&self) -> Option<&str> {
        let selected = self
            .cli_workspace
            .or(self.sourced_workspace)
            .or(self.default_workspace)?;
        let trimmed = selected.trim_matches(js_space);
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

    fn check(&self, url_workspace: &str) -> Result<(), AppError> {
        let Some(current) = self.effective_workspace() else {
            return Ok(());
        };
        if current.to_lowercase() == url_workspace.to_lowercase() {
            return Ok(());
        }
        Err(AppError::new(
            AppErrorKind::Validation,
            format!(
                "That URL is for the \"{url_workspace}\" workspace, but this is the \"{current}\" workspace."
            ),
        )
        .with_suggestion(self.switch_suggestion(url_workspace, current)))
    }
}

/// Return `None` for ordinary input, or a typed URL of the requested kind.
/// URL refusal precedes workspace checking; workspace checking precedes kind checking.
pub fn expect_url_kind(
    input: &str,
    kind: LinearUrlKind,
    entity_label: &str,
    scope: &WorkspaceScope<'_>,
) -> Result<Option<LinearUrlRef>, AppError> {
    let suggestion = || format!("Pass {entity_label}.");
    let parsed = match parse_linear_url(input) {
        LinearUrlParse::NotLinear => return Ok(None),
        LinearUrlParse::Unsupported(reason) => {
            return Err(AppError::new(
                AppErrorKind::Validation,
                format!("\"{input}\" is a Linear URL, but {reason}."),
            )
            .with_suggestion(suggestion()));
        }
        LinearUrlParse::Known(reference) => reference,
    };
    scope.check(parsed.workspace())?;
    if parsed.kind() != kind {
        return Err(AppError::new(
            AppErrorKind::Validation,
            format!(
                "\"{input}\" is {} URL, not {} URL.",
                parsed.kind().label(),
                kind.label()
            ),
        )
        .with_suggestion(suggestion()));
    }
    Ok(Some(parsed))
}

/// Reject any recognized Linear URL for commands that accept only plain references.
pub fn reject_linear_url(input: &str, entity_label: &str) -> Result<(), AppError> {
    if matches!(parse_linear_url(input), LinearUrlParse::NotLinear) {
        return Ok(());
    }
    Err(AppError::new(
        AppErrorKind::Validation,
        format!("\"{input}\" is a Linear URL, and this command does not take one."),
    )
    .with_suggestion(format!("Pass {entity_label}.")))
}

/// Reject an issue comment link in any workspace: its anchor carries only the
/// first eight characters of the comment id. Other URLs, including issue URLs
/// with a non-comment anchor, are left to `reject_linear_url`.
pub fn reject_comment_url(input: &str) -> Result<(), AppError> {
    if !matches!(
        parse_linear_url(input),
        LinearUrlParse::Known(LinearUrlRef::Issue {
            comment_id_prefix: Some(_),
            ..
        })
    ) {
        return Ok(());
    }
    Err(AppError::new(
        AppErrorKind::Validation,
        format!(
            "\"{input}\" links to a comment, but a comment URL only carries the first eight characters of its ID."
        ),
    )
    .with_suggestion("Pass the comment's full UUID, from `linear issue comment list <issue> --json`."))
}

/// Prepare a team URL for the later GraphQL resolver without selecting credentials.
pub fn expect_team_url(
    input: &str,
    scope: &WorkspaceScope<'_>,
) -> Result<Option<String>, AppError> {
    match expect_url_kind(
        input,
        LinearUrlKind::Team,
        "a team URL, key, name, or ID",
        scope,
    )? {
        Some(LinearUrlRef::Team { team_key, .. }) => Ok(Some(team_key)),
        None => Ok(None),
        Some(_) => Err(AppError::new(
            AppErrorKind::Invariant,
            "team URL kind check returned a different kind",
        )),
    }
}
