use std::fmt;

use crate::config::{ConfigOptions, ConfigSecret, OptionSource};

use super::format::CredentialStore;

#[derive(Clone)]
pub enum ApiKeyInput<'a> {
    Raw {
        value: &'a ConfigSecret,
        source: OptionSource,
    },
    Sourced {
        value: &'a ConfigSecret,
    },
    Absent,
}

impl<'a> ApiKeyInput<'a> {
    /// Classifies the configured API key by where it came from, without
    /// copying the secret.
    pub fn from_options(options: &'a ConfigOptions) -> Self {
        let Some(selected) = options.api_key() else {
            return Self::Absent;
        };
        match selected.source() {
            source @ (OptionSource::Env | OptionSource::ProjectEnv { .. }) => Self::Raw {
                value: selected.value(),
                source: source.clone(),
            },
            OptionSource::ProjectConfig { .. } | OptionSource::GlobalConfig { .. } => {
                Self::Sourced {
                    value: selected.value(),
                }
            }
            OptionSource::Cli => unreachable!("no command-line flag sets the API key"),
        }
    }
}

pub struct CredentialSelectionInputs<'a> {
    pub api_key: ApiKeyInput<'a>,
    pub cli_workspace: Option<&'a str>,
    pub sourced_workspace: Option<(&'a str, OptionSource)>,
}

/// How the workspace whose stored key a command uses was chosen.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkspaceChoice {
    /// The global `--workspace` flag.
    Flag,
    /// The `workspace` option, from the environment or a config file.
    Configured(OptionSource),
    /// The credentials file's default workspace.
    Default,
}

pub enum CredentialSelection<'a> {
    Selected {
        secret: &'a ConfigSecret,
        /// The stored credential's workspace; `None` for an API key from the
        /// environment or a config file, whose workspace is not known locally.
        workspace: Option<&'a str>,
    },
    NoKey,
    /// `--workspace` was passed while `LINEAR_API_KEY` is set (from `source`).
    EnvWorkspaceConflict {
        source: OptionSource,
    },
    /// The chosen workspace has no usable key: it is not stored, or its
    /// keyring entry is missing or unreadable.
    Unavailable {
        workspace: &'a str,
        choice: WorkspaceChoice,
        stored: bool,
    },
}
impl fmt::Debug for CredentialSelection<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Selected { workspace, .. } => f
                .debug_struct("Selected")
                .field("secret", &"<redacted>")
                .field("workspace", workspace)
                .finish(),
            Self::NoKey => f.write_str("NoKey"),
            Self::EnvWorkspaceConflict { source } => f
                .debug_struct("EnvWorkspaceConflict")
                .field("source", source)
                .finish(),
            Self::Unavailable {
                workspace,
                choice,
                stored,
            } => f
                .debug_struct("Unavailable")
                .field("workspace", workspace)
                .field("choice", choice)
                .field("stored", stored)
                .finish(),
        }
    }
}

/// Chooses the key a command uses: an API key from the environment or a
/// config file, else the stored key of the `--workspace` workspace, the
/// configured workspace, or the default workspace, in that order. Once a
/// workspace is chosen its key must be usable; there is no fallback to
/// another workspace's key.
pub fn resolve<'a>(
    inputs: &CredentialSelectionInputs<'a>,
    store: &'a CredentialStore,
) -> CredentialSelection<'a> {
    let present = |value: &&str| !value.is_empty();
    let cli = inputs.cli_workspace.filter(present);
    match &inputs.api_key {
        ApiKeyInput::Raw { value, source } if !value.expose().is_empty() => {
            if cli.is_some() {
                return CredentialSelection::EnvWorkspaceConflict {
                    source: source.clone(),
                };
            }
            return CredentialSelection::Selected {
                secret: value,
                workspace: None,
            };
        }
        ApiKeyInput::Sourced { value, .. } if !value.expose().is_empty() => {
            return CredentialSelection::Selected {
                secret: value,
                workspace: None,
            };
        }
        ApiKeyInput::Raw { .. } | ApiKeyInput::Sourced { .. } | ApiKeyInput::Absent => {}
    }
    let chosen = cli
        .map(|workspace| (workspace, WorkspaceChoice::Flag))
        .or_else(|| {
            inputs
                .sourced_workspace
                .as_ref()
                .filter(|(workspace, _)| present(workspace))
                .map(|(workspace, source)| {
                    (*workspace, WorkspaceChoice::Configured(source.clone()))
                })
        })
        .or_else(|| {
            store
                .default()
                .map(|workspace| (workspace, WorkspaceChoice::Default))
        });
    let Some((workspace, choice)) = chosen else {
        return CredentialSelection::NoKey;
    };
    match store
        .key(workspace)
        .filter(|secret| !secret.expose().is_empty())
    {
        Some(secret) => CredentialSelection::Selected {
            secret,
            workspace: Some(workspace),
        },
        None => CredentialSelection::Unavailable {
            workspace,
            choice,
            stored: store.workspaces().iter().any(|name| name == workspace),
        },
    }
}

#[cfg(test)]
mod tests;
