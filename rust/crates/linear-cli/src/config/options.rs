//! Eagerly validated, source-aware config options. No process state is read here.
use std::error::Error;
use std::fmt;
use std::path::{Component, Path, PathBuf};

use crate::graphql::transport::EndpointUrl;
use crate::text::js_space;

use super::dotenv::SelectedEnv;
use super::parse::{ConfigTier, ConfigValue};
use super::source::{ConfigInputs, OsFamily};

const DEFAULT_ENDPOINT: &str = "https://api.linear.app/graphql";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OptionKey {
    TeamId,
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
    pub const ALL: [Self; 12] = [
        Self::TeamId,
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
            Self::TeamId => "team_id",
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
    /// Base for config-sourced template paths only. Other path options retain raw text.
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

#[derive(Clone, Eq, PartialEq)]
pub struct ConfigSecret(String);

impl ConfigSecret {
    /// Construct an owned, redacted secret from an injected credential source.
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IssueSort {
    Manual,
    Priority,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssignSelf {
    Always,
    Auto,
    Never,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Vcs {
    Git,
    Jj,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OptionErrorReason {
    WrongType,
    InvalidBoolean,
    InvalidChoice,
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
        if self.reason == OptionErrorReason::InvalidCwd {
            return f.write_str("config working directory must be absolute");
        }
        let name = self.key.map_or("LINEAR_GRAPHQL_ENDPOINT", OptionKey::name);
        write!(
            f,
            "invalid config option {name} from {:?}: {:?}",
            self.source, self.reason
        )
    }
}
impl Error for ConfigOptionError {}

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

#[derive(Clone, Copy)]
enum Raw<'a> {
    Text(&'a str),
    Toml(&'a ConfigValue),
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

fn toml_key(tier: Option<&ConfigTier>, key: OptionKey) -> Option<(&ConfigValue, PathBuf)> {
    let tier = tier?;
    tier.entries
        .iter()
        .find(|(name, _)| name == key.name())
        .map(|(_, value)| (value, tier.path.clone()))
}

fn dotenv_key<'a>(inputs: &'a OptionInputs<'_>, name: &str) -> Option<&'a str> {
    // The B1 applied map is case-sensitive; Windows process variables are not.
    // A differently-cased dotenv duplicate did not apply and must not be parsed.
    if inputs.env.os == OsFamily::Windows && inputs.env.process_env.contains_key(name) {
        return None;
    }
    let pair = if inputs.env.os == OsFamily::Windows {
        inputs
            .dotenv
            .applied
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
    } else {
        inputs.dotenv.applied.get_key_value(name)
    };
    pair.map(|(_, value)| value.as_str())
}

fn select<T>(
    inputs: &OptionInputs<'_>,
    key: OptionKey,
    parse: impl Fn(Raw<'_>) -> Result<T, OptionErrorReason>,
) -> Result<Option<Resolved<T>>, ConfigOptionError> {
    let name = key.env_name();
    let mut selected = None;
    // Report the first bad value in a fixed order, while parsing all present tiers.
    if let Some(value) = inputs.env.process_env.get(&name) {
        selected = Some(Resolved {
            value: parse(Raw::Text(value))
                .map_err(|reason| error(Some(key), OptionSource::Env, reason))?,
            source: OptionSource::Env,
        });
    }
    if let Some(value) = dotenv_key(inputs, &name) {
        let source = match &inputs.dotenv.source_path {
            Some(path) => OptionSource::ProjectEnv { path: path.clone() },
            None => {
                return Err(error(
                    Some(key),
                    OptionSource::Env,
                    OptionErrorReason::MissingDotenvPath,
                ));
            }
        };
        let parsed =
            parse(Raw::Text(value)).map_err(|reason| error(Some(key), source.clone(), reason))?;
        if selected.is_none() {
            selected = Some(Resolved {
                value: parsed,
                source,
            });
        }
    }
    if let Some((value, path)) = toml_key(inputs.project, key) {
        let source = OptionSource::ProjectConfig { path };
        let parsed =
            parse(Raw::Toml(value)).map_err(|reason| error(Some(key), source.clone(), reason))?;
        if selected.is_none() {
            selected = Some(Resolved {
                value: parsed,
                source,
            });
        }
    }
    if let Some((value, path)) = toml_key(inputs.global, key) {
        let source = OptionSource::GlobalConfig { path };
        let parsed =
            parse(Raw::Toml(value)).map_err(|reason| error(Some(key), source.clone(), reason))?;
        if selected.is_none() {
            selected = Some(Resolved {
                value: parsed,
                source,
            });
        }
    }
    Ok(selected)
}

fn text(raw: Raw<'_>) -> Result<String, OptionErrorReason> {
    match raw {
        Raw::Text(value) => Ok(value.to_owned()),
        Raw::Toml(ConfigValue::String(value)) => Ok(value.clone()),
        Raw::Toml(_) => Err(OptionErrorReason::WrongType),
    }
}

fn boolean(raw: Raw<'_>) -> Result<bool, OptionErrorReason> {
    match raw {
        Raw::Toml(ConfigValue::Boolean(value)) => Ok(*value),
        Raw::Text(value) => boolean_word(value),
        Raw::Toml(ConfigValue::String(value)) => boolean_word(value),
        Raw::Toml(_) => Err(OptionErrorReason::WrongType),
    }
}

fn boolean_word(value: &str) -> Result<bool, OptionErrorReason> {
    if ["true", "yes", "y", "on", "1", "t"]
        .iter()
        .any(|word| value.eq_ignore_ascii_case(word))
    {
        Ok(true)
    } else if ["false", "no", "n", "off", "0", "f"]
        .iter()
        .any(|word| value.eq_ignore_ascii_case(word))
    {
        Ok(false)
    } else {
        Err(OptionErrorReason::InvalidBoolean)
    }
}

fn choice<T>(raw: Raw<'_>, choices: &[(&str, T)]) -> Result<T, OptionErrorReason>
where
    T: Copy,
{
    let value = text(raw)?;
    choices
        .iter()
        .find(|(name, _)| value == *name)
        .map(|(_, result)| *result)
        .ok_or(OptionErrorReason::InvalidChoice)
}

fn issue_sort(raw: Raw<'_>) -> Result<IssueSort, OptionErrorReason> {
    choice(
        raw,
        &[
            ("manual", IssueSort::Manual),
            ("priority", IssueSort::Priority),
        ],
    )
}
fn assign_self(raw: Raw<'_>) -> Result<AssignSelf, OptionErrorReason> {
    choice(
        raw,
        &[
            ("always", AssignSelf::Always),
            ("auto", AssignSelf::Auto),
            ("never", AssignSelf::Never),
        ],
    )
}
fn vcs(raw: Raw<'_>) -> Result<Vcs, OptionErrorReason> {
    choice(raw, &[("git", Vcs::Git), ("jj", Vcs::Jj)])
}

fn template(raw: Raw<'_>) -> Result<String, OptionErrorReason> {
    let value = text(raw)?;
    let trimmed = value.trim_matches(js_space);
    if trimmed.is_empty() {
        Err(OptionErrorReason::EmptyTemplate)
    } else {
        Ok(trimmed.to_owned())
    }
}

fn normalized_config_path(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if result.has_root() && result.parent().is_none() {
                    continue;
                }
                let _ = result.pop();
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
    team_id: Option<Resolved<String>>,
    api_key: Option<Resolved<ConfigSecret>>,
    workspace: Option<Resolved<String>>,
    issue_sort: Option<Resolved<IssueSort>>,
    deferred_issue_sort: Option<Resolved<DeferredIssueSort>>,
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

// Only mine/query select this policy; other commands retain startup validation.
#[derive(Clone, Debug)]
enum DeferredIssueSort {
    Parsed(IssueSort),
    Invalid(serde_json::Value),
}
fn sort_raw_value(raw: Raw<'_>) -> serde_json::Value {
    fn value(v: &ConfigValue) -> serde_json::Value {
        match v {
            ConfigValue::String(v) | ConfigValue::Datetime(v) => {
                serde_json::Value::String(v.clone())
            }
            ConfigValue::Integer(v) => serde_json::Value::Number((*v).into()),
            ConfigValue::Float(v) => serde_json::Number::from_f64(*v)
                .map_or(serde_json::Value::Null, serde_json::Value::Number),
            ConfigValue::Boolean(v) => serde_json::Value::Bool(*v),
            ConfigValue::Array(v) => serde_json::Value::Array(v.iter().map(value).collect()),
            ConfigValue::Table(v) => {
                serde_json::Value::Object(v.iter().map(|(k, v)| (k.clone(), value(v))).collect())
            }
        }
    }
    match raw {
        Raw::Text(v) => serde_json::Value::String(v.to_owned()),
        Raw::Toml(v) => value(v),
    }
}
impl ConfigOptions {
    pub fn from_inputs(inputs: OptionInputs<'_>) -> Result<Self, ConfigOptionError> {
        Self::from_inputs_with_issue_read_sort(inputs, false)
    }
    /// Explicit command policy: defer only the sort value consumed by mine/query.
    pub(crate) fn from_inputs_with_issue_read_sort(
        inputs: OptionInputs<'_>,
        defer: bool,
    ) -> Result<Self, ConfigOptionError> {
        if !inputs.env.cwd.is_absolute() {
            return Err(error(
                None,
                OptionSource::Env,
                OptionErrorReason::InvalidCwd,
            ));
        }
        // Keep OptionSchemas order as the error order. `select` validates every tier.
        let team_id = select(&inputs, OptionKey::TeamId, text)?;
        let api_key = select(&inputs, OptionKey::ApiKey, |raw| {
            text(raw).map(ConfigSecret)
        })?;
        let workspace = select(&inputs, OptionKey::Workspace, text)?;
        let (issue_sort, deferred_issue_sort) = if defer {
            let selected = select(&inputs, OptionKey::IssueSort, |raw| {
                Ok(match issue_sort(raw) {
                    Ok(value) => DeferredIssueSort::Parsed(value),
                    Err(_) => DeferredIssueSort::Invalid(sort_raw_value(raw)),
                })
            })?;
            (None, selected)
        } else {
            (select(&inputs, OptionKey::IssueSort, issue_sort)?, None)
        };
        let issue_create_ask_project = select(&inputs, OptionKey::IssueCreateAskProject, boolean)?;
        let issue_create_assign_self =
            select(&inputs, OptionKey::IssueCreateAssignSelf, assign_self)?;
        let vcs = select(&inputs, OptionKey::Vcs, vcs)?;
        let download_images = select(&inputs, OptionKey::DownloadImages, boolean)?;
        let hyperlink_format = select(&inputs, OptionKey::HyperlinkFormat, text)?;
        let attachment_dir = select(&inputs, OptionKey::AttachmentDir, text)?;
        let auto_download_attachments =
            select(&inputs, OptionKey::AutoDownloadAttachments, boolean)?;
        let pr_template = select(&inputs, OptionKey::PrTemplate, template)?;
        let endpoint = endpoint(&inputs)?;
        Ok(Self {
            cwd: inputs.env.cwd.clone(),
            team_id,
            api_key,
            workspace,
            issue_sort,
            deferred_issue_sort,
            issue_create_ask_project,
            issue_create_assign_self,
            vcs,
            download_images,
            hyperlink_format,
            attachment_dir,
            auto_download_attachments,
            pr_template,
            endpoint,
        })
    }

    pub fn team_id(&self) -> Option<&Resolved<String>> {
        self.team_id.as_ref()
    }
    pub fn api_key(&self) -> Option<&Resolved<ConfigSecret>> {
        self.api_key.as_ref()
    }
    pub fn workspace(&self) -> Option<&Resolved<String>> {
        self.workspace.as_ref()
    }
    pub fn sourced_issue_sort(&self) -> Option<&Resolved<IssueSort>> {
        self.issue_sort.as_ref()
    }
    pub fn issue_create_ask_project(&self) -> Option<&Resolved<bool>> {
        self.issue_create_ask_project.as_ref()
    }
    pub fn issue_create_assign_self(&self) -> Option<&Resolved<AssignSelf>> {
        self.issue_create_assign_self.as_ref()
    }
    pub fn vcs(&self) -> Option<&Resolved<Vcs>> {
        self.vcs.as_ref()
    }
    pub fn download_images(&self) -> Option<&Resolved<bool>> {
        self.download_images.as_ref()
    }
    pub fn hyperlink_format(&self) -> Option<&Resolved<String>> {
        self.hyperlink_format.as_ref()
    }
    pub fn attachment_dir(&self) -> Option<&Resolved<String>> {
        self.attachment_dir.as_ref()
    }
    pub fn auto_download_attachments(&self) -> Option<&Resolved<bool>> {
        self.auto_download_attachments.as_ref()
    }
    pub fn sourced_pr_template(&self) -> Option<&Resolved<String>> {
        self.pr_template.as_ref()
    }
    pub fn endpoint(&self) -> &ResolvedEndpoint {
        &self.endpoint
    }

    /// Only commands with a registered `--sort` can supply this typed override.
    /// Source failure stage is the command's filter pipeline, never startup.
    pub fn issue_read_sort(
        &self,
        cli: Option<IssueSort>,
    ) -> Result<IssueSort, crate::error::AppError> {
        if let Some(cli) = cli {
            return Ok(cli);
        }
        match self.deferred_issue_sort.as_ref().map(|v| v.value()) {
            Some(DeferredIssueSort::Parsed(value)) => Ok(*value),
            Some(DeferredIssueSort::Invalid(raw)) => {
                let text = serde_json::to_string(&crate::graphql::bulk_error::JsValue(raw))
                    .map_err(|e| {
                        crate::error::AppError::new(
                            crate::error::AppErrorKind::Invariant,
                            "could not format issue sort input",
                        )
                        .with_source(e)
                    })?;
                Err(crate::error::AppError::new(crate::error::AppErrorKind::Validation,format!("Invalid issue sort: {text}")).with_suggestion("Use one of: manual, priority (via --sort, the issue_sort config option, or the LINEAR_ISSUE_SORT environment variable)"))
            }
            None => Ok(self.issue_sort(None).0),
        }
    }
    pub fn issue_sort(&self, cli: Option<IssueSort>) -> (IssueSort, Option<OptionSource>) {
        match cli {
            Some(value) => (value, Some(OptionSource::Cli)),
            None => match &self.issue_sort {
                Some(value) => (*value.value(), Some(value.source().clone())),
                None => (IssueSort::Priority, None),
            },
        }
    }

    /// Only the PR creation command supplies `--template` or `--no-template`.
    pub fn pr_template(
        &self,
        cli: PrTemplateCli<'_>,
    ) -> Result<Option<PrTemplatePath>, ConfigOptionError> {
        let selected = match cli {
            PrTemplateCli::Disabled => return Ok(None),
            PrTemplateCli::Path(raw) => {
                let value = template(Raw::Text(raw)).map_err(|reason| {
                    error(Some(OptionKey::PrTemplate), OptionSource::Cli, reason)
                })?;
                Some(Resolved {
                    value,
                    source: OptionSource::Cli,
                })
            }
            PrTemplateCli::Unset => self.pr_template.clone(),
        };
        Ok(selected.map(|selected| {
            let path = match selected.source.config_dir() {
                Some(base) => {
                    let base = if base.is_absolute() {
                        base.to_owned()
                    } else {
                        self.cwd.join(base)
                    };
                    normalized_config_path(&base.join(&selected.value))
                }
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
    } else if let Some(value) = dotenv_key(inputs, name) {
        let Some(path) = &inputs.dotenv.source_path else {
            return Err(error(
                None,
                OptionSource::Env,
                OptionErrorReason::MissingDotenvPath,
            ));
        };
        (
            value,
            EndpointSource::ProjectEnv { path: path.clone() },
            OptionSource::ProjectEnv { path: path.clone() },
        )
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
