//! Config options merged from the command line, process environment, `.env`,
//! project config and global config, in that order of precedence.
//!
//! Every tier that sets an option is validated, even when a higher tier
//! overrides it, so a typo never lies dormant. No process state is read here.
use std::error::Error as StdError;
use std::fmt;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use serde::de::value::{Error as ValueError, StrDeserializer};
use serde::de::{DeserializeOwned, IntoDeserializer};

use crate::client::EndpointUrl;

use super::dotenv::SelectedEnv;
use super::parse::ConfigTier;
use super::source::{ConfigInputs, OsFamily};

const DEFAULT_ENDPOINT: &str = "https://api.linear.app/graphql";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OptionKey {
    /// The default team's key, under its historical name `team_id`.
    TeamKey,
    ApiKey,
    Workspace,
    IssueSort,
    IssueCreateAskProject,
    IssueCreateAssignSelf,
    Vcs,
    DownloadImages,
    HyperlinkFormat,
    AttachmentDir,
    AutoDownloadAttachments,
    PrTemplate,
}

impl OptionKey {
    #[cfg(test)]
    pub const ALL: [Self; 12] = [
        Self::TeamKey,
        Self::ApiKey,
        Self::Workspace,
        Self::IssueSort,
        Self::IssueCreateAskProject,
        Self::IssueCreateAssignSelf,
        Self::Vcs,
        Self::DownloadImages,
        Self::HyperlinkFormat,
        Self::AttachmentDir,
        Self::AutoDownloadAttachments,
        Self::PrTemplate,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::TeamKey => "team_id",
            Self::ApiKey => "api_key",
            Self::Workspace => "workspace",
            Self::IssueSort => "issue_sort",
            Self::IssueCreateAskProject => "issue_create_ask_project",
            Self::IssueCreateAssignSelf => "issue_create_assign_self",
            Self::Vcs => "vcs",
            Self::DownloadImages => "download_images",
            Self::HyperlinkFormat => "hyperlink_format",
            Self::AttachmentDir => "attachment_dir",
            Self::AutoDownloadAttachments => "auto_download_attachments",
            Self::PrTemplate => "pr_template",
        }
    }

    pub fn env_name(self) -> String {
        format!("LINEAR_{}", self.name().to_ascii_uppercase())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OptionSource {
    Cli,
    Env,
    ProjectEnv { path: PathBuf },
    ProjectConfig { path: PathBuf },
    GlobalConfig { path: PathBuf },
}

impl OptionSource {
    /// The directory config-file paths are relative to. Paths from the
    /// command line and environment are used as given.
    pub fn config_dir(&self) -> Option<&Path> {
        match self {
            Self::ProjectConfig { path } | Self::GlobalConfig { path } => path.parent(),
            Self::Cli | Self::Env | Self::ProjectEnv { .. } => None,
        }
    }

    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::ProjectEnv { path }
            | Self::ProjectConfig { path }
            | Self::GlobalConfig { path } => Some(path),
            Self::Cli | Self::Env => None,
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::Cli => "command line".to_owned(),
            Self::Env => "process environment".to_owned(),
            Self::ProjectEnv { path } => format!(".env file {}", path.display()),
            Self::ProjectConfig { path } => format!("project config {}", path.display()),
            Self::GlobalConfig { path } => format!("global config {}", path.display()),
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct Resolved<T> {
    value: T,
    source: OptionSource,
}

impl<T> Resolved<T> {
    pub fn value(&self) -> &T {
        &self.value
    }
    pub fn source(&self) -> &OptionSource {
        &self.source
    }
}

impl<T> fmt::Debug for Resolved<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Resolved")
            .field("value", &"<redacted>")
            .field("source", &self.source)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq, Deserialize)]
#[serde(transparent)]
pub struct ConfigSecret(String);

impl ConfigSecret {
    pub fn new(value: String) -> Self {
        Self(value)
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ConfigSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase", expecting = "manual or priority")]
pub enum IssueSort {
    Manual,
    Priority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase", expecting = "always, auto or never")]
pub enum AssignSelf {
    Always,
    Auto,
    Never,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase", expecting = "git or jj")]
pub enum Vcs {
    Git,
    Jj,
}

/// A boolean option: a TOML boolean, or one of the usual words in a string
/// (`true`/`false`, `yes`/`no`, `on`/`off`, `1`/`0`, ...), ignoring case.
#[derive(Clone, Copy)]
struct Flag(bool);

impl<'de> Deserialize<'de> for Flag {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct FlagVisitor;
        impl serde::de::Visitor<'_> for FlagVisitor {
            type Value = Flag;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a boolean")
            }

            fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<Flag, E> {
                Ok(Flag(value))
            }

            fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<Flag, E> {
                let is = |words: &[&str]| words.iter().any(|word| text.eq_ignore_ascii_case(word));
                if is(&["true", "yes", "y", "on", "1", "t"]) {
                    Ok(Flag(true))
                } else if is(&["false", "no", "n", "off", "0", "f"]) {
                    Ok(Flag(false))
                } else {
                    Err(E::invalid_value(serde::de::Unexpected::Str(text), &self))
                }
            }
        }
        deserializer.deserialize_any(FlagVisitor)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OptionErrorReason {
    /// The value has the wrong type or is not one of the allowed values.
    Invalid(String),
    EmptyTemplate,
    InvalidEndpoint,
    MissingDotenvPath,
    InvalidCwd,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigOptionError {
    pub key: Option<OptionKey>,
    pub source: OptionSource,
    pub reason: OptionErrorReason,
}

impl fmt::Display for ConfigOptionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let source = self.source.label();
        match (&self.reason, self.key) {
            (OptionErrorReason::InvalidCwd, _) => {
                f.write_str("config working directory must be absolute")
            }
            (OptionErrorReason::MissingDotenvPath, _) => {
                f.write_str("selected dotenv value has no source path")
            }
            (OptionErrorReason::InvalidEndpoint, _) | (_, None) => write!(
                f,
                "invalid LINEAR_GRAPHQL_ENDPOINT from {source}: expected an http(s) URL without credentials or fragment"
            ),
            (OptionErrorReason::Invalid(detail), Some(key)) => write!(
                f,
                "invalid config option {} from {source}: {detail}",
                self.option_name(key)
            ),
            (OptionErrorReason::EmptyTemplate, Some(key)) => write!(
                f,
                "invalid config option {} from {source}: expected a nonempty template path",
                self.option_name(key)
            ),
        }
    }
}

impl StdError for ConfigOptionError {}

impl ConfigOptionError {
    /// The spelling the user wrote: `LINEAR_FOO` in the environment,
    /// `foo` in a config file or flag.
    fn option_name(&self, key: OptionKey) -> String {
        match self.source {
            OptionSource::Env | OptionSource::ProjectEnv { .. } => key.env_name(),
            OptionSource::Cli
            | OptionSource::ProjectConfig { .. }
            | OptionSource::GlobalConfig { .. } => key.name().to_owned(),
        }
    }

    pub fn suggestion(&self) -> Option<String> {
        match (&self.reason, self.key) {
            (OptionErrorReason::InvalidCwd | OptionErrorReason::MissingDotenvPath, _) => None,
            (OptionErrorReason::InvalidEndpoint, _) | (_, None) => {
                Some("Set a valid LINEAR_GRAPHQL_ENDPOINT or remove it.".to_owned())
            }
            (OptionErrorReason::Invalid(_) | OptionErrorReason::EmptyTemplate, Some(key)) => Some(
                format!("Fix {} in {}.", self.option_name(key), self.source.label()),
            ),
        }
    }
}

impl From<ConfigOptionError> for crate::error::Error {
    fn from(error: ConfigOptionError) -> Self {
        use crate::error::Error;
        let app = Error::new(error.to_string());
        match error.suggestion() {
            Some(suggestion) => app.with_hint(suggestion),
            None => app,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EndpointSource {
    Default,
    Env,
    ProjectEnv { path: PathBuf },
}

#[derive(Clone, Eq, PartialEq)]
pub struct ResolvedEndpoint {
    value: EndpointUrl,
    source: EndpointSource,
}

impl ResolvedEndpoint {
    pub fn value(&self) -> &EndpointUrl {
        &self.value
    }
    #[cfg(test)]
    pub fn source(&self) -> &EndpointSource {
        &self.source
    }
}

impl fmt::Debug for ResolvedEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResolvedEndpoint")
            .field("value", &"<redacted>")
            .field("source", &self.source)
            .finish()
    }
}

pub struct OptionInputs<'a> {
    pub env: &'a ConfigInputs,
    pub dotenv: &'a SelectedEnv,
    pub project: Option<&'a ConfigTier>,
    pub global: Option<&'a ConfigTier>,
}

/// One tier's raw value for an option.
enum Raw<'a> {
    Text(&'a str),
    Toml(&'a toml::Value),
}

impl Raw<'_> {
    fn parse<T: DeserializeOwned>(&self) -> Result<T, OptionErrorReason> {
        fn run<'de, T: serde::Deserialize<'de>, D: serde::Deserializer<'de>>(
            deserializer: D,
        ) -> Result<T, OptionErrorReason>
        where
            D::Error: fmt::Display,
        {
            T::deserialize(deserializer).map_err(|error| {
                OptionErrorReason::Invalid(error.to_string().trim_end().to_owned())
            })
        }
        // Scalars go through serde's plain deserializers so a wrong type reads
        // "invalid type: integer `1`, expected ..." for every target type.
        match self {
            Self::Text(text) => run::<T, StrDeserializer<'_, ValueError>>(text.into_deserializer()),
            Self::Toml(toml::Value::String(text)) => {
                run::<T, StrDeserializer<'_, ValueError>>(text.as_str().into_deserializer())
            }
            Self::Toml(toml::Value::Integer(value)) => {
                run::<T, _>(IntoDeserializer::<ValueError>::into_deserializer(*value))
            }
            Self::Toml(toml::Value::Float(value)) => {
                run::<T, _>(IntoDeserializer::<ValueError>::into_deserializer(*value))
            }
            Self::Toml(toml::Value::Boolean(value)) => {
                run::<T, _>(IntoDeserializer::<ValueError>::into_deserializer(*value))
            }
            Self::Toml(value) => run::<T, _>((*value).clone()),
        }
    }
}

fn error(
    key: Option<OptionKey>,
    source: OptionSource,
    reason: OptionErrorReason,
) -> ConfigOptionError {
    ConfigOptionError {
        key,
        source,
        reason,
    }
}

/// A `.env` value that was applied (the process environment did not set it).
/// Windows variable names are case-insensitive.
fn dotenv_value<'a>(inputs: &'a OptionInputs<'_>, name: &str) -> Option<&'a str> {
    match inputs.env.os {
        OsFamily::Windows => {
            if inputs.env.process_env.contains_key(name) {
                return None;
            }
            inputs
                .dotenv
                .applied
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(name))
                .map(|(_, value)| value.as_str())
        }
        OsFamily::Unix => inputs.dotenv.applied.get(name).map(String::as_str),
    }
}

fn dotenv_source(
    inputs: &OptionInputs<'_>,
    key: Option<OptionKey>,
) -> Result<OptionSource, ConfigOptionError> {
    match &inputs.dotenv.source_path {
        Some(path) => Ok(OptionSource::ProjectEnv { path: path.clone() }),
        None => Err(error(
            key,
            OptionSource::Env,
            OptionErrorReason::MissingDotenvPath,
        )),
    }
}

/// Every tier's raw value for `key`, highest precedence first.
fn tiers<'a>(
    inputs: &'a OptionInputs<'_>,
    key: OptionKey,
) -> Result<Vec<(Raw<'a>, OptionSource)>, ConfigOptionError> {
    let name = key.env_name();
    let mut found = Vec::new();
    if let Some(value) = inputs.env.process_env.get(&name) {
        found.push((Raw::Text(value), OptionSource::Env));
    }
    if let Some(value) = dotenv_value(inputs, &name) {
        found.push((Raw::Text(value), dotenv_source(inputs, Some(key))?));
    }
    if let Some(tier) = inputs.project
        && let Some(value) = tier.table.get(key.name())
    {
        let path = tier.path.clone();
        found.push((Raw::Toml(value), OptionSource::ProjectConfig { path }));
    }
    if let Some(tier) = inputs.global
        && let Some(value) = tier.table.get(key.name())
    {
        let path = tier.path.clone();
        found.push((Raw::Toml(value), OptionSource::GlobalConfig { path }));
    }
    Ok(found)
}

/// Validates every tier's value and returns the highest-precedence one.
fn select<T>(
    inputs: &OptionInputs<'_>,
    key: OptionKey,
    parse: impl Fn(&Raw<'_>) -> Result<T, OptionErrorReason>,
) -> Result<Option<Resolved<T>>, ConfigOptionError> {
    let mut selected = None;
    for (raw, source) in tiers(inputs, key)? {
        let value = parse(&raw).map_err(|reason| error(Some(key), source.clone(), reason))?;
        selected.get_or_insert(Resolved { value, source });
    }
    Ok(selected)
}

fn parsed<T: DeserializeOwned>(raw: &Raw<'_>) -> Result<T, OptionErrorReason> {
    raw.parse()
}

/// A team key, uppercased. An empty value means "no default team", so it
/// also hides a team set in a lower tier.
fn team_key(raw: &Raw<'_>) -> Result<Option<String>, OptionErrorReason> {
    let value = raw.parse::<String>()?;
    Ok((!value.is_empty()).then(|| value.to_uppercase()))
}

fn flag(raw: &Raw<'_>) -> Result<bool, OptionErrorReason> {
    raw.parse::<Flag>().map(|Flag(value)| value)
}

fn template(raw: &Raw<'_>) -> Result<String, OptionErrorReason> {
    let value = raw.parse::<String>()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(OptionErrorReason::EmptyTemplate)
    } else {
        Ok(trimmed.to_owned())
    }
}

/// Lexically resolves `.` and `..` without touching the filesystem.
fn normalized_config_path(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                result.push(component.as_os_str());
            }
        }
    }
    result
}

#[derive(Clone)]
pub struct ConfigOptions {
    cwd: PathBuf,
    team_key: Option<Resolved<Option<String>>>,
    api_key: Option<Resolved<ConfigSecret>>,
    workspace: Option<Resolved<String>>,
    issue_sort: Option<Resolved<IssueSort>>,
    issue_create_ask_project: Option<Resolved<bool>>,
    issue_create_assign_self: Option<Resolved<AssignSelf>>,
    vcs: Option<Resolved<Vcs>>,
    download_images: Option<Resolved<bool>>,
    hyperlink_format: Option<Resolved<String>>,
    attachment_dir: Option<Resolved<String>>,
    auto_download_attachments: Option<Resolved<bool>>,
    pr_template: Option<Resolved<String>>,
    endpoint: ResolvedEndpoint,
}

impl fmt::Debug for ConfigOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConfigOptions")
            .field("values", &"<redacted>")
            .field("endpoint", &self.endpoint)
            .finish()
    }
}

pub enum PrTemplateCli<'a> {
    Unset,
    Path(&'a str),
    Disabled,
}

#[derive(Clone, Eq, PartialEq)]
pub struct PrTemplatePath {
    path: PathBuf,
    source: OptionSource,
}

impl PrTemplatePath {
    pub fn path(&self) -> &Path {
        &self.path
    }
    #[cfg(test)]
    pub fn source(&self) -> &OptionSource {
        &self.source
    }
}

impl fmt::Debug for PrTemplatePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PrTemplatePath")
            .field("path", &"<redacted>")
            .field("source", &self.source)
            .finish()
    }
}

impl ConfigOptions {
    pub fn from_inputs(inputs: OptionInputs<'_>) -> Result<Self, ConfigOptionError> {
        if !inputs.env.cwd.is_absolute() {
            return Err(error(
                None,
                OptionSource::Env,
                OptionErrorReason::InvalidCwd,
            ));
        }
        Ok(Self {
            cwd: inputs.env.cwd.clone(),
            team_key: select(&inputs, OptionKey::TeamKey, team_key)?,
            api_key: select(&inputs, OptionKey::ApiKey, parsed)?,
            workspace: select(&inputs, OptionKey::Workspace, parsed)?,
            issue_sort: select(&inputs, OptionKey::IssueSort, parsed)?,
            issue_create_ask_project: select(&inputs, OptionKey::IssueCreateAskProject, flag)?,
            issue_create_assign_self: select(&inputs, OptionKey::IssueCreateAssignSelf, parsed)?,
            vcs: select(&inputs, OptionKey::Vcs, parsed)?,
            download_images: select(&inputs, OptionKey::DownloadImages, flag)?,
            hyperlink_format: select(&inputs, OptionKey::HyperlinkFormat, parsed)?,
            attachment_dir: select(&inputs, OptionKey::AttachmentDir, parsed)?,
            auto_download_attachments: select(&inputs, OptionKey::AutoDownloadAttachments, flag)?,
            pr_template: select(&inputs, OptionKey::PrTemplate, template)?,
            endpoint: endpoint(&inputs)?,
        })
    }

    /// The default team's key.
    pub fn team_key(&self) -> Option<&str> {
        self.team_key
            .as_ref()
            .and_then(|resolved| resolved.value.as_deref())
    }
    /// Where the default team (or its explicit absence) was set.
    pub fn team_key_source(&self) -> Option<&OptionSource> {
        self.team_key.as_ref().map(Resolved::source)
    }
    pub fn api_key(&self) -> Option<&Resolved<ConfigSecret>> {
        self.api_key.as_ref()
    }
    pub fn workspace(&self) -> Option<&Resolved<String>> {
        self.workspace.as_ref()
    }
    #[cfg(test)]
    pub fn sourced_issue_sort(&self) -> Option<&Resolved<IssueSort>> {
        self.issue_sort.as_ref()
    }
    /// Whether `issue create` asks for a project; off by default.
    pub fn issue_create_ask_project(&self) -> bool {
        self.issue_create_ask_project
            .as_ref()
            .is_some_and(|resolved| resolved.value)
    }
    pub fn issue_create_assign_self(&self) -> AssignSelf {
        self.issue_create_assign_self
            .as_ref()
            .map_or(AssignSelf::Auto, |resolved| resolved.value)
    }
    pub fn vcs(&self) -> Vcs {
        self.vcs
            .as_ref()
            .map_or(Vcs::Git, |resolved| resolved.value)
    }
    #[cfg(test)]
    pub fn vcs_source(&self) -> Option<&OptionSource> {
        self.vcs.as_ref().map(Resolved::source)
    }
    /// Whether issue and document views download images; on by default.
    pub fn download_images(&self) -> bool {
        self.download_images
            .as_ref()
            .is_none_or(|resolved| resolved.value)
    }
    pub fn hyperlink_format(&self) -> Option<&str> {
        self.hyperlink_format
            .as_ref()
            .map(|resolved| resolved.value.as_str())
    }
    /// Where issue attachments are saved, when set to a non-empty path.
    pub fn attachment_dir(&self) -> Option<&str> {
        self.attachment_dir
            .as_ref()
            .map(|resolved| resolved.value.as_str())
            .filter(|dir| !dir.is_empty())
    }
    /// Whether `issue view` downloads attachments; on by default.
    pub fn auto_download_attachments(&self) -> bool {
        self.auto_download_attachments
            .as_ref()
            .is_none_or(|resolved| resolved.value)
    }
    #[cfg(test)]
    pub fn sourced_pr_template(&self) -> Option<&Resolved<String>> {
        self.pr_template.as_ref()
    }
    pub fn endpoint(&self) -> &ResolvedEndpoint {
        &self.endpoint
    }

    /// The issue sort order: the `--sort` flag, then config, then priority.
    pub fn issue_sort(&self, cli: Option<IssueSort>) -> (IssueSort, Option<OptionSource>) {
        match cli {
            Some(value) => (value, Some(OptionSource::Cli)),
            None => match &self.issue_sort {
                Some(value) => (*value.value(), Some(value.source().clone())),
                None => (IssueSort::Priority, None),
            },
        }
    }

    /// The pull request template path. Paths from config files are relative
    /// to the file's directory.
    pub fn pr_template(
        &self,
        cli: PrTemplateCli<'_>,
    ) -> Result<Option<PrTemplatePath>, ConfigOptionError> {
        let selected = match cli {
            PrTemplateCli::Disabled => return Ok(None),
            PrTemplateCli::Path(raw) => Some(Resolved {
                value: template(&Raw::Text(raw)).map_err(|reason| {
                    error(Some(OptionKey::PrTemplate), OptionSource::Cli, reason)
                })?,
                source: OptionSource::Cli,
            }),
            PrTemplateCli::Unset => self.pr_template.clone(),
        };
        Ok(selected.map(|selected| {
            let path = match selected.source.config_dir() {
                Some(base) => normalized_config_path(&self.cwd.join(base).join(&selected.value)),
                None => PathBuf::from(&selected.value),
            };
            PrTemplatePath {
                path,
                source: selected.source,
            }
        }))
    }
}

fn endpoint(inputs: &OptionInputs<'_>) -> Result<ResolvedEndpoint, ConfigOptionError> {
    let name = "LINEAR_GRAPHQL_ENDPOINT";
    let (value, source, origin) = if let Some(value) = inputs.env.process_env.get(name) {
        (value.as_str(), EndpointSource::Env, OptionSource::Env)
    } else if let Some(value) = dotenv_value(inputs, name) {
        let origin = dotenv_source(inputs, None)?;
        let Some(path) = origin.path() else {
            return Err(error(
                None,
                OptionSource::Env,
                OptionErrorReason::MissingDotenvPath,
            ));
        };
        let source = EndpointSource::ProjectEnv {
            path: path.to_owned(),
        };
        (value, source, origin)
    } else {
        (DEFAULT_ENDPOINT, EndpointSource::Default, OptionSource::Env)
    };
    let (value, source) = if value.is_empty() {
        (DEFAULT_ENDPOINT, EndpointSource::Default)
    } else {
        (value, source)
    };
    let parsed = EndpointUrl::parse(value)
        .map_err(|_| error(None, origin, OptionErrorReason::InvalidEndpoint))?;
    Ok(ResolvedEndpoint {
        value: parsed,
        source,
    })
}

#[cfg(test)]
mod tests;
