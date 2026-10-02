use std::cell::{OnceCell, RefCell};
use std::collections::BTreeMap;
use std::error::Error as StdError;
use std::fmt;
use std::path::PathBuf;

use serde::Deserialize;

use crate::auth::keyring::KeyringReader;
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
impl StdError for CredentialFormatError {}

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

/// Stored credentials. Keyring-backed keys are looked up lazily, at most once
/// per workspace, so a command reads only the entry it uses.
pub struct CredentialStore {
    format: CredentialFormat,
    workspaces: Vec<String>,
    default: Option<String>,
    inline_keys: BTreeMap<String, ConfigSecret>,
    keyring: Box<dyn KeyringReader>,
    lookups: BTreeMap<String, OnceCell<LookupResult>>,
    warnings: RefCell<Vec<CredentialWarning>>,
}
impl fmt::Debug for CredentialStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CredentialStore")
            .field("format", &self.format)
            .field("workspaces", &self.workspaces)
            .field("default", &self.default)
            .finish_non_exhaustive()
    }
}
impl CredentialStore {
    pub fn new(manifest: CredentialManifest, keyring: Box<dyn KeyringReader>) -> Self {
        let lookups = match manifest.format {
            CredentialFormat::Metadata => manifest
                .workspaces
                .iter()
                .map(|workspace| (workspace.clone(), OnceCell::new()))
                .collect(),
            CredentialFormat::Inline => BTreeMap::new(),
        };
        Self {
            format: manifest.format,
            workspaces: manifest.workspaces,
            default: manifest.default,
            inline_keys: manifest.inline_keys.into_iter().collect(),
            keyring,
            lookups,
            warnings: RefCell::new(manifest.warnings),
        }
    }

    pub fn format(&self) -> CredentialFormat {
        self.format
    }
    pub fn workspaces(&self) -> &[String] {
        &self.workspaces
    }
    /// The default workspace; a store with a single workspace needs none set.
    pub fn default(&self) -> Option<&str> {
        self.default
            .as_deref()
            .or(match self.workspaces.as_slice() {
                [only] => Some(only.as_str()),
                _ => None,
            })
    }

    /// The key stored for `workspace`, reading the keyring on first use.
    pub fn key(&self, workspace: &str) -> Option<&ConfigSecret> {
        if let Some(key) = self.inline_keys.get(workspace) {
            return Some(key);
        }
        let cell = self.lookups.get(workspace)?;
        let result = cell.get_or_init(|| {
            let result = self.keyring.lookup(workspace);
            let warning = match &result {
                LookupResult::Hit(_) => None,
                LookupResult::Miss => Some(CredentialWarning::LookupMiss {
                    workspace: workspace.to_owned(),
                }),
                LookupResult::Failed(category) => Some(CredentialWarning::LookupFailed {
                    workspace: workspace.to_owned(),
                    category: *category,
                }),
            };
            self.warnings.borrow_mut().extend(warning);
            result
        });
        match result {
            LookupResult::Hit(secret) => Some(secret),
            LookupResult::Miss | LookupResult::Failed(_) => None,
        }
    }

    /// Warnings gathered so far (an invalid default, keyring misses), each returned once.
    pub fn take_warnings(&self) -> Vec<CredentialWarning> {
        self.warnings.take()
    }

    /// Every workspace's key, reading the keyring as needed, for rewriting the file.
    pub(crate) fn mutation_parts(
        &self,
    ) -> (
        CredentialFormat,
        Vec<String>,
        Option<String>,
        BTreeMap<String, ConfigSecret>,
    ) {
        let keys = self
            .workspaces
            .iter()
            .filter_map(|workspace| Some((workspace.clone(), self.key(workspace)?.clone())))
            .collect();
        (
            self.format,
            self.workspaces.clone(),
            self.default.clone(),
            keys,
        )
    }
}
