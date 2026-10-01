use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use crate::config::{ConfigSecret, ConfigTier, ConfigValue};

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

pub(crate) fn js_index(name: &str) -> Option<u32> {
    if name.is_empty()
        || (name.len() > 1 && name.starts_with('0'))
        || !name.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let index = name.parse::<u32>().ok()?;
    (index < u32::MAX).then_some(index)
}

fn js_entry_order(entries: Vec<(String, ConfigValue)>) -> Vec<(String, ConfigValue)> {
    let (mut numeric, ordinary): (Vec<_>, Vec<_>) = entries
        .into_iter()
        .partition(|(name, _)| js_index(name).is_some());
    numeric.sort_by_key(|(name, _)| js_index(name));
    numeric.into_iter().chain(ordinary).collect()
}

/// Parse the already bounded, owned TOML tree. No keyring lookup occurs.
pub fn parse_credentials(tier: ConfigTier) -> Result<CredentialManifest, CredentialFormatError> {
    let ConfigTier { path, entries } = tier;
    // Original TOML block parsing deepMerge omits this own property before
    // credential format detection. Metadata array values remain untouched.
    let entries = js_entry_order(
        entries
            .into_iter()
            .filter(|(name, _)| name != "__proto__")
            .collect(),
    );
    let fail = |kind| CredentialFormatError {
        path: path.clone(),
        kind,
    };
    let has_workspaces = entries.iter().any(|(name, _)| name == "workspaces");
    let has_inline_string = entries.iter().any(|(name, value)| {
        name != "default" && name != "workspaces" && matches!(value, ConfigValue::String(_))
    });
    if has_workspaces && has_inline_string {
        return Err(fail(CredentialFormatErrorKind::MixedFormat));
    }
    let format = if has_inline_string {
        CredentialFormat::Inline
    } else {
        CredentialFormat::Metadata
    };
    let mut default = None;
    let mut workspaces = Vec::new();
    let mut inline_keys = Vec::new();
    let mut metadata_values = None;
    for (name, value) in entries {
        match (format, name.as_str(), value) {
            (_, "default", ConfigValue::String(value)) => default = Some(value),
            (_, "default", _) => return Err(fail(CredentialFormatErrorKind::WrongType)),
            (CredentialFormat::Metadata, "workspaces", ConfigValue::Array(values)) => {
                metadata_values = Some(values)
            }
            (CredentialFormat::Metadata, "workspaces", _) => {
                return Err(fail(CredentialFormatErrorKind::WrongType));
            }
            (CredentialFormat::Inline, _, ConfigValue::String(value)) => {
                workspaces.push(name.clone());
                inline_keys.push((name, ConfigSecret::new(value)));
            }
            _ => return Err(fail(CredentialFormatErrorKind::WrongType)),
        }
    }
    let mut raw_count = 0;
    if let Some(values) = metadata_values {
        raw_count = values.len();
        let mut seen = BTreeSet::new();
        for value in values {
            let ConfigValue::String(workspace) = value else {
                return Err(fail(CredentialFormatErrorKind::WrongType));
            };
            if seen.insert(workspace.clone()) {
                workspaces.push(workspace);
            }
        }
    }
    if workspaces.iter().any(String::is_empty) {
        return Err(fail(CredentialFormatErrorKind::EmptyWorkspace));
    }
    if format == CredentialFormat::Metadata && raw_count > 256 {
        return Err(fail(CredentialFormatErrorKind::TooManyWorkspaces));
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
    // Insert inside impl CredentialStore. Owned handoff; no new parse/reload.
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

/// Convert a complete fake lookup table into cache state. No backend is called.
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
