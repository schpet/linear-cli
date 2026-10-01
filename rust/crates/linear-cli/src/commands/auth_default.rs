//! Default selection uses loaded membership; only changed targets write the file.
use crate::auth::CredentialStore;
use crate::auth::write::{CredentialFileWriter, prepare_default_write};
use crate::error::{AppError, AppErrorKind};
use crate::platform::prompt::PlainOption;
use std::path::Path;

pub const CONTEXT: &str = "Failed to set default workspace";

#[derive(Debug)]
pub enum DefaultAction {
    Output(Vec<u8>),
    Select(Vec<PlainOption>),
    Save(String),
}

pub fn prepare(store: &CredentialStore, target: Option<&str>) -> Result<DefaultAction, AppError> {
    match store.workspaces() {
        [] => {
            return Err(
                AppError::new(AppErrorKind::Auth, "No workspaces configured")
                    .with_suggestion("Run `linear auth login` to add a workspace"),
            );
        }
        [only] => {
            return Ok(DefaultAction::Output(
                format!("Only one workspace configured: {only}\n").into_bytes(),
            ));
        }
        _ => {}
    }
    let Some(target) = target.filter(|target| !target.is_empty()) else {
        let options = store.workspaces().iter().map(|name| {
            // Refuse user data before constructing the session/entering raw mode.
            if name.trim().is_empty() || name.chars().any(char::is_control) {
                return Err(AppError::new(AppErrorKind::Validation,
                    "Workspace names containing control characters or only whitespace cannot be selected interactively")
                    .with_suggestion("Specify a workspace explicitly with `linear auth default <workspace>`."));
            }
            Ok(PlainOption {
                label: if store.default() == Some(name.as_str()) { format!("{name} (current)") } else { name.clone() },
                value: name.clone(),
                script_token: name.clone(),
            })
        }).collect::<Result<Vec<_>, AppError>>()?;
        return Ok(DefaultAction::Select(options));
    };
    if !store.workspaces().iter().any(|name| name == target) {
        return Err(
            AppError::not_found("Workspace", target).with_suggestion(format!(
                "Available workspaces: {}",
                store.workspaces().join(", ")
            )),
        );
    }
    if store.default() == Some(target) {
        return Ok(DefaultAction::Output(
            format!("\"{target}\" is already the default workspace\n").into_bytes(),
        ));
    }
    Ok(DefaultAction::Save(target.to_owned()))
}

pub fn save(
    store: &CredentialStore,
    target: &str,
    path: Option<&Path>,
    writer: &impl CredentialFileWriter,
) -> Result<Vec<u8>, AppError> {
    let plan = prepare_default_write(store, target, path)?;
    plan.save(writer)?;
    Ok(format!("Default workspace set to: {target}\n").into_bytes())
}

pub fn non_tty_error() -> AppError {
    AppError::new(
        AppErrorKind::Validation,
        "A workspace is required when stdin is not a terminal",
    )
    .with_suggestion("Specify a workspace with `linear auth default <workspace>`.")
}
