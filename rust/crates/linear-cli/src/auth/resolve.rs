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
        source: OptionSource,
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
            source @ (OptionSource::ProjectConfig { .. } | OptionSource::GlobalConfig { .. }) => {
                Self::Sourced {
                    value: selected.value(),
                    source: source.clone(),
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CredentialSource {
    Raw(OptionSource),
    Sourced(OptionSource),
    ExplicitWorkspace {
        workspace: String,
    },
    SourcedWorkspace {
        workspace: String,
        source: OptionSource,
    },
    DefaultWorkspace {
        workspace: String,
    },
}

pub enum CredentialSelection<'a> {
    Selected {
        secret: &'a ConfigSecret,
        source: CredentialSource,
    },
    NoKey,
    EnvWorkspaceConflict,
    MissingExplicitWorkspace {
        workspace: &'a str,
    },
}
impl fmt::Debug for CredentialSelection<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Selected { source, .. } => f
                .debug_struct("Selected")
                .field("secret", &"<redacted>")
                .field("source", source)
                .finish(),
            Self::NoKey => f.write_str("NoKey"),
            Self::EnvWorkspaceConflict => f.write_str("EnvWorkspaceConflict"),
            Self::MissingExplicitWorkspace { workspace } => f
                .debug_struct("MissingExplicitWorkspace")
                .field("workspace", workspace)
                .finish(),
        }
    }
}

fn truthy(value: &str) -> bool {
    !value.is_empty()
}

/// Resolve a key without HTTP-header transformation or any backend interaction.
pub fn resolve<'a>(
    inputs: &'a CredentialSelectionInputs<'a>,
    store: &'a CredentialStore,
) -> CredentialSelection<'a> {
    let cli = inputs.cli_workspace.filter(|value| truthy(value));
    match &inputs.api_key {
        ApiKeyInput::Raw { value, source } if truthy(value.expose()) => {
            if cli.is_some() {
                return CredentialSelection::EnvWorkspaceConflict;
            }
            return CredentialSelection::Selected {
                secret: value,
                source: CredentialSource::Raw(source.clone()),
            };
        }
        ApiKeyInput::Sourced { value, source } if truthy(value.expose()) => {
            return CredentialSelection::Selected {
                secret: value,
                source: CredentialSource::Sourced(source.clone()),
            };
        }
        ApiKeyInput::Raw { .. } | ApiKeyInput::Sourced { .. } | ApiKeyInput::Absent => {}
    }
    if let Some(workspace) = cli {
        if let Some(secret) = store
            .key(workspace)
            .filter(|secret| truthy(secret.expose()))
        {
            return CredentialSelection::Selected {
                secret,
                source: CredentialSource::ExplicitWorkspace {
                    workspace: workspace.to_owned(),
                },
            };
        }
        return CredentialSelection::MissingExplicitWorkspace { workspace };
    }
    if let Some((workspace, source)) = &inputs.sourced_workspace
        && truthy(workspace)
        && let Some(secret) = store
            .key(workspace)
            .filter(|secret| truthy(secret.expose()))
    {
        return CredentialSelection::Selected {
            secret,
            source: CredentialSource::SourcedWorkspace {
                workspace: (*workspace).to_owned(),
                source: source.clone(),
            },
        };
    }
    if let Some(workspace) = store.default()
        && let Some(secret) = store
            .key(workspace)
            .filter(|secret| truthy(secret.expose()))
    {
        return CredentialSelection::Selected {
            secret,
            source: CredentialSource::DefaultWorkspace {
                workspace: workspace.to_owned(),
            },
        };
    }
    CredentialSelection::NoKey
}
