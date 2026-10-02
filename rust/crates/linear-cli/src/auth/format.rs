use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use serde::Deserialize;

use crate::config::{ConfigSecret, ConfigTier};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialFormat {
    Inline,
    Metadata,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialFormatErrorKind {
    MixedFormat,
    WrongType,
    EmptyWorkspace,
    TooManyWorkspaces,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialFormatError {
    pub path: PathBuf,
    pub kind: CredentialFormatErrorKind,
}
impl fmt::Display for CredentialFormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid credentials file at {}: {:?}",
            self.path.display(),
            self.kind
        )
    }
}
impl Error for CredentialFormatError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LookupFailureCategory {
    Unavailable,
    Permission,
    Other,
    UnsupportedPlatform,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CredentialWarning {
    InvalidDefault {
        workspace: String,
    },
    LookupMiss {
        workspace: String,
    },
    LookupFailed {
        workspace: String,
        category: LookupFailureCategory,
    },
}

pub struct CredentialManifest {
    format: CredentialFormat,
    workspaces: Vec<String>,
    default: Option<String>,
    inline_keys: Vec<(String, ConfigSecret)>,
    warnings: Vec<CredentialWarning>,
}
impl fmt::Debug for CredentialManifest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CredentialManifest")
            .field("format", &self.format)
            .field("workspaces", &self.workspaces)
            .field("default", &self.default)
            .field("inline_keys", &"<redacted>")
            .field("warnings", &self.warnings)
            .finish()
    }
}
impl CredentialManifest {
    pub fn empty() -> Self {
        Self {
            format: CredentialFormat::Metadata,
            workspaces: Vec::new(),
            default: None,
            inline_keys: Vec::new(),
            warnings: Vec::new(),
        }
    }
    pub fn format(&self) -> CredentialFormat {
        self.format
    }
    pub fn workspaces(&self) -> &[String] {
        &self.workspaces
    }
    pub fn default(&self) -> Option<&str> {
        self.default.as_deref()
    }
    pub fn warnings(&self) -> &[CredentialWarning] {
        &self.warnings
    }
    pub fn lookup_requests(&self) -> Vec<&str> {
        if self.format == CredentialFormat::Metadata {
            self.workspaces.iter().map(String::as_str).collect()
        } else {
            Vec::new()
        }
    }
}

/// The most workspaces a metadata credentials file may list.
const MAX_WORKSPACES: usize = 256;

/// Reads a parsed credentials file. Two layouts exist:
///
/// - inline: `<workspace> = "<api key>"` entries, plus an optional
///   `default = "<workspace>"`;
/// - metadata: `default = "<workspace>"` and `workspaces = [...]`, with the
///   keys stored in the system keyring.
///
/// No keyring lookup happens here.
pub fn parse_credentials(tier: ConfigTier) -> Result<CredentialManifest, CredentialFormatError> {
    let ConfigTier { path, mut table } = tier;
    let fail = |kind| CredentialFormatError {
        path: path.clone(),
        kind,
    };
    let mut default = match table.remove("default") {
        None => None,
        Some(toml::Value::String(workspace)) => Some(workspace),
        Some(_) => return Err(fail(CredentialFormatErrorKind::WrongType)),
    };
    let listed = table
        .remove("workspaces")
        .map(|value| {
            Vec::<String>::deserialize(value)
                .map_err(|_| fail(CredentialFormatErrorKind::WrongType))
        })
        .transpose()?;
    let inline_keys = table
        .into_iter()
        .map(|(workspace, value)| match value {
            toml::Value::String(key) => Ok((workspace, ConfigSecret::new(key))),
            _ => Err(fail(CredentialFormatErrorKind::WrongType)),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (format, workspaces) = match listed {
        Some(_) if !inline_keys.is_empty() => {
            return Err(fail(CredentialFormatErrorKind::MixedFormat));
        }
        Some(listed) => {
            if listed.iter().any(String::is_empty) {
                return Err(fail(CredentialFormatErrorKind::EmptyWorkspace));
            }
            if listed.len() > MAX_WORKSPACES {
                return Err(fail(CredentialFormatErrorKind::TooManyWorkspaces));
            }
            let mut workspaces = Vec::new();
            for workspace in listed {
                if !workspaces.contains(&workspace) {
                    workspaces.push(workspace);
                }
            }
            (CredentialFormat::Metadata, workspaces)
        }
        None if inline_keys.is_empty() => (CredentialFormat::Metadata, Vec::new()),
        None => (
            CredentialFormat::Inline,
            inline_keys.iter().map(|(name, _)| name.clone()).collect(),
        ),
    };
    if workspaces.iter().any(String::is_empty) {
        return Err(fail(CredentialFormatErrorKind::EmptyWorkspace));
    }
    let mut warnings = Vec::new();
    if format == CredentialFormat::Metadata
        && let Some(workspace) = &default
        && !workspaces.contains(workspace)
    {
        warnings.push(CredentialWarning::InvalidDefault {
            workspace: workspace.clone(),
        });
        default = None;
    }
    Ok(CredentialManifest {
        format,
        workspaces,
        default,
        inline_keys,
        warnings,
    })
}

#[derive(Clone, Debug)]
pub enum LookupResult {
    Hit(ConfigSecret),
    Miss,
    Failed(LookupFailureCategory),
}
#[derive(Clone, Debug)]
pub struct LookupReply {
    pub workspace: String,
    pub result: LookupResult,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CredentialInvariantError {
    ExtraReply { workspace: String },
    DuplicateReply { workspace: String },
    MissingReply { workspace: String },
    UnexpectedApiKeySource,
}
impl fmt::Display for CredentialInvariantError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "credential lookup invariant failed: {self:?}")
    }
}
impl Error for CredentialInvariantError {}

pub struct CredentialStore {
    format: CredentialFormat,
    workspaces: Vec<String>,
    default: Option<String>,
    keys: BTreeMap<String, ConfigSecret>,
    warnings: Vec<CredentialWarning>,
}
impl fmt::Debug for CredentialStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CredentialStore")
            .field("format", &self.format)
            .field("workspaces", &self.workspaces)
            .field("default", &self.default)
            .field("keys", &"<redacted>")
            .field("warnings", &self.warnings)
            .finish()
    }
}
impl CredentialStore {
    pub(crate) fn mutation_parts(
        &self,
    ) -> (
        CredentialFormat,
        Vec<String>,
        Option<String>,
        BTreeMap<String, ConfigSecret>,
    ) {
        (
            self.format,
            self.workspaces.clone(),
            self.default.clone(),
            self.keys.clone(),
        )
    }

    pub fn format(&self) -> CredentialFormat {
        self.format
    }
    pub fn workspaces(&self) -> &[String] {
        &self.workspaces
    }
    pub fn default(&self) -> Option<&str> {
        self.default.as_deref()
    }
    pub fn key(&self, workspace: &str) -> Option<&ConfigSecret> {
        self.keys.get(workspace)
    }
    pub fn warnings(&self) -> &[CredentialWarning] {
        &self.warnings
    }
}

/// Combines a parsed credentials file with keyring lookup results. Every
/// metadata workspace must have exactly one reply.
pub fn hydrate(
    manifest: CredentialManifest,
    replies: Vec<LookupReply>,
) -> Result<CredentialStore, CredentialInvariantError> {
    let expected = manifest
        .lookup_requests()
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let mut reply_map = BTreeMap::new();
    for reply in replies {
        if !expected.contains(&reply.workspace) {
            return Err(CredentialInvariantError::ExtraReply {
                workspace: reply.workspace,
            });
        }
        if reply_map
            .insert(reply.workspace.clone(), reply.result)
            .is_some()
        {
            return Err(CredentialInvariantError::DuplicateReply {
                workspace: reply.workspace,
            });
        }
    }
    for workspace in &manifest.workspaces {
        if manifest.format == CredentialFormat::Metadata && !reply_map.contains_key(workspace) {
            return Err(CredentialInvariantError::MissingReply {
                workspace: workspace.clone(),
            });
        }
    }
    let mut keys = manifest.inline_keys.into_iter().collect::<BTreeMap<_, _>>();
    let mut warnings = manifest.warnings;
    for workspace in &manifest.workspaces {
        match reply_map.remove(workspace) {
            Some(LookupResult::Hit(secret)) => {
                keys.insert(workspace.clone(), secret);
            }
            Some(LookupResult::Miss) => warnings.push(CredentialWarning::LookupMiss {
                workspace: workspace.clone(),
            }),
            Some(LookupResult::Failed(category)) => {
                warnings.push(CredentialWarning::LookupFailed {
                    workspace: workspace.clone(),
                    category,
                })
            }
            None => {}
        }
    }
    Ok(CredentialStore {
        format: manifest.format,
        workspaces: manifest.workspaces,
        default: manifest.default,
        keys,
        warnings,
    })
}
