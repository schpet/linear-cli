use crate::{
    auth::mutation::{
        CredentialMutationBackend, CredentialMutationFileWriter, CredentialMutationState,
        MutationFailure,
    },
    error::Error,
    platform::prompt::{PlainOption, escaped_display},
};
use std::path::Path;
pub const CONTEXT: &str = "Failed to logout";
pub const SELECT_MESSAGE: &str = "Select workspace to remove";
pub enum LogoutTarget {
    Selected(String),
    Select(Vec<PlainOption>),
}
pub fn prepare(
    state: &CredentialMutationState,
    explicit: Option<&str>,
) -> Result<LogoutTarget, Error> {
    if state.workspaces().is_empty() {
        return Err(Error::auth("No workspaces configured"));
    }
    if let Some(name) = explicit.filter(|value| !value.is_empty()) {
        return selected(state, name);
    }
    if let [only] = state.workspaces() {
        return Ok(LogoutTarget::Selected(only.clone()));
    }
    let options = state.workspaces().iter().map(|name| {
        if name.trim().is_empty() || name.chars().any(char::is_control) {
            return Err(Error::new("Workspace names containing control characters or only whitespace cannot be selected interactively")
                .with_hint("Specify the workspace explicitly with `linear auth logout <workspace>`."));
        }
        Ok(PlainOption { label: if state.default() == Some(name.as_str()) {
            format!("{} (default)", escaped_display(name))
        } else { escaped_display(name) }, value: name.clone(), script_token: name.clone() })
    }).collect::<Result<Vec<_>, Error>>()?;
    Ok(LogoutTarget::Select(options))
}
pub fn selected(state: &CredentialMutationState, name: &str) -> Result<LogoutTarget, Error> {
    if !state.has_workspace(name) {
        return Err(Error::not_found("Workspace", name));
    }
    Ok(LogoutTarget::Selected(name.to_owned()))
}
pub fn confirm_message(name: &str) -> String {
    format!(
        "Remove credentials for workspace \"{}\"?",
        escaped_display(name)
    )
}
pub async fn remove(
    state: &mut CredentialMutationState,
    name: &str,
    path: Option<&Path>,
    backend: &impl CredentialMutationBackend,
    writer: &impl CredentialMutationFileWriter,
) -> Result<Vec<u8>, MutationFailure> {
    state.remove(name, path, backend, writer).await?;
    let mut output = format!("Removed credentials for workspace: {name}\n");
    if !state.workspaces().is_empty()
        && let Some(default) = state.default().filter(|name| !name.is_empty())
    {
        output.push_str(&format!("  Default workspace is now: {default}\n"));
    }
    Ok(output.into_bytes())
}
