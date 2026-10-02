//! Default selection uses loaded membership; only changed targets write the file.
use crate::auth::CredentialStore;
use crate::auth::write::{CredentialFileWriter, prepare_default_write};
use crate::error::Error;
use crate::platform::prompt::PlainOption;
use std::path::Path;

pub const CONTEXT: &str = "Failed to set default workspace";

#[derive(Debug)]
pub enum DefaultAction {
    Output(Vec<u8>),
    Select(Vec<PlainOption>),
    Save(String),
}

pub fn prepare(store: &CredentialStore, target: Option<&str>) -> Result<DefaultAction, Error> {
    match store.workspaces() {
        [] => {
            return Err(Error::auth("No workspaces configured")
                .with_hint("Run `linear auth login` to add a workspace"));
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
                return Err(Error::new("Workspace names containing control characters or only whitespace cannot be selected interactively")
                    .with_hint("Specify a workspace explicitly with `linear auth default <workspace>`."));
            }
            Ok(PlainOption {
                label: if store.default() == Some(name.as_str()) { format!("{name} (current)") } else { name.clone() },
                value: name.clone(),
                script_token: name.clone(),
            })
        }).collect::<Result<Vec<_>, Error>>()?;
        return Ok(DefaultAction::Select(options));
    };
    if !store.workspaces().iter().any(|name| name == target) {
        return Err(Error::not_found("Workspace", target).with_hint(format!(
            "Available workspaces: {}",
            store.workspaces().join(", ")
        )));
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
) -> Result<Vec<u8>, Error> {
    let plan = prepare_default_write(store, target, path)?;
    plan.save(writer)?;
    Ok(format!("Default workspace set to: {target}\n").into_bytes())
}

pub fn non_tty_error() -> Error {
    Error::new("A workspace is required when stdin is not a terminal")
        .with_hint("Specify a workspace with `linear auth default <workspace>`.")
}
