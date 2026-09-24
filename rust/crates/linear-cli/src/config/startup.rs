//! One owned, injectable config startup result for all routes, including help.
use std::collections::BTreeMap;
use std::fmt;
use std::sync::OnceLock;

use crate::error::{AppError, AppErrorKind};

use super::discover::ConfigCandidate;
use super::discover::discover_config_paths;
use super::dotenv::{ConfigDiagnostic, ConfigFailure, DiagnosticReason, SelectedEnv, load_env};
use super::options::{
    ConfigOptionError, ConfigOptions, OptionErrorReason, OptionInputs, OptionKey, OptionSource,
};
use super::parse::{ConfigParseError, ConfigParseErrorKind, ConfigTier, parse_config_tier};
use super::runtime::ProcessEnvSnapshot;
use super::source::{
    FileSource, GitProbeError, GitProbeResult, GitRootProbe, OsFamily, ReadCandidate,
    read_config_candidate,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NoColor {
    Absent,
    Empty,
    Nonempty,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DisplaySettings {
    pub debug: bool,
    pub no_color: NoColor,
}

impl DisplaySettings {
    pub fn help_color(self) -> bool {
        self.no_color != NoColor::Nonempty
    }

    pub fn no_color(self) -> bool {
        self.no_color == NoColor::Nonempty
    }
}

/// Applied dotenv values for later child processes. These can contain secrets.
#[derive(Clone, Eq, PartialEq)]
pub struct ChildEnvOverlay {
    os: OsFamily,
    values: BTreeMap<String, String>,
}

impl ChildEnvOverlay {
    pub fn get(&self, name: &str) -> Option<&str> {
        if self.os == OsFamily::Windows {
            self.values
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(name))
                .map(|(_, value)| value.as_str())
        } else {
            self.values.get(name).map(String::as_str)
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.values
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
    }
}

impl fmt::Debug for ChildEnvOverlay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ChildEnvOverlay(<redacted>)")
    }
}

pub struct StartupConfig {
    pub options: ConfigOptions,
    pub child_env: ChildEnvOverlay,
}

impl fmt::Debug for StartupConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StartupConfig(<redacted>)")
    }
}

#[derive(Clone, Debug)]
pub struct StartupError {
    kind: AppErrorKind,
    message: String,
    suggestion: Option<String>,
}

impl StartupError {
    fn new(kind: AppErrorKind, message: impl Into<String>, suggestion: Option<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            suggestion,
        }
    }

    pub fn app_error(&self) -> AppError {
        let mut error = AppError::new(self.kind, self.message.clone());
        if let Some(suggestion) = &self.suggestion {
            error = error.with_suggestion(suggestion.clone());
        }
        error
    }
}

pub struct StartupReport {
    pub settings: DisplaySettings,
    pub diagnostics: Vec<ConfigDiagnostic>,
    pub result: Result<StartupConfig, StartupError>,
}

impl fmt::Debug for StartupReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StartupReport")
            .field("settings", &self.settings)
            .field("diagnostics", &self.diagnostics)
            .field("result", &self.result.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

struct MemoGit<'a, G> {
    git: &'a G,
    cache: OnceLock<GitProbeResult>,
}

impl<G: GitRootProbe> GitRootProbe for MemoGit<'_, G> {
    fn probe(&self) -> GitProbeResult {
        self.cache.get_or_init(|| self.git.probe()).clone()
    }
}

fn effective<'a>(
    process: &'a ProcessEnvSnapshot,
    dotenv: &'a SelectedEnv,
    name: &str,
) -> Option<&'a str> {
    if let Some(value) = process.inputs.env(name) {
        return Some(value);
    }
    let value = if process.inputs.os == OsFamily::Windows {
        dotenv
            .applied
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value)
    } else {
        dotenv.applied.get(name)
    };
    value.map(String::as_str)
}

fn settings(process: &ProcessEnvSnapshot, dotenv: Option<&SelectedEnv>) -> DisplaySettings {
    let no_color = match process.inputs.env("NO_COLOR") {
        None => NoColor::Absent,
        Some("") => NoColor::Empty,
        Some(_) => NoColor::Nonempty,
    };
    let debug = match dotenv {
        Some(dotenv) => matches!(
            effective(process, dotenv, "LINEAR_DEBUG"),
            Some("1" | "true")
        ),
        None => matches!(process.inputs.env("LINEAR_DEBUG"), Some("1" | "true")),
    };
    DisplaySettings { debug, no_color }
}

fn source_label(source: &OptionSource) -> String {
    match source {
        OptionSource::Cli => "command line".to_owned(),
        OptionSource::Env => "process environment".to_owned(),
        OptionSource::ProjectEnv { path } => format!(".env file {}", path.display()),
        OptionSource::ProjectConfig { path } => format!("project config {}", path.display()),
        OptionSource::GlobalConfig { path } => format!("global config {}", path.display()),
    }
}

fn option_name(key: OptionKey, source: &OptionSource) -> String {
    match source {
        OptionSource::Env | OptionSource::ProjectEnv { .. } => key.env_name(),
        OptionSource::Cli
        | OptionSource::ProjectConfig { .. }
        | OptionSource::GlobalConfig { .. } => key.name().to_owned(),
    }
}

fn option_reason(key: OptionKey, reason: &OptionErrorReason) -> &'static str {
    match reason {
        OptionErrorReason::WrongType => match key {
            OptionKey::IssueCreateAskProject
            | OptionKey::DownloadImages
            | OptionKey::AutoDownloadAttachments => "expected a boolean",
            _ => "expected a string",
        },
        OptionErrorReason::InvalidBoolean => "expected a boolean",
        OptionErrorReason::InvalidChoice => match key {
            OptionKey::IssueSort => "expected manual or priority",
            OptionKey::IssueCreateAssignSelf => "expected always, auto or never",
            OptionKey::Vcs => "expected git or jj",
            _ => "invalid choice",
        },
        OptionErrorReason::EmptyTemplate => "expected a nonempty template path",
        OptionErrorReason::InvalidEndpoint
        | OptionErrorReason::MissingDotenvPath
        | OptionErrorReason::InvalidCwd => "invalid value",
    }
}

fn option_error(error: ConfigOptionError) -> StartupError {
    if error.reason == OptionErrorReason::InvalidCwd {
        return StartupError::new(
            AppErrorKind::Invariant,
            "config working directory must be absolute",
            None,
        );
    }
    if error.reason == OptionErrorReason::MissingDotenvPath {
        return StartupError::new(
            AppErrorKind::Invariant,
            "selected dotenv value has no source path",
            None,
        );
    }
    let source = source_label(&error.source);
    if error.reason == OptionErrorReason::InvalidEndpoint {
        return StartupError::new(
            AppErrorKind::Validation,
            format!(
                "invalid LINEAR_GRAPHQL_ENDPOINT from {source}: expected an http(s) URL without credentials or fragment"
            ),
            Some("Set a valid LINEAR_GRAPHQL_ENDPOINT or remove it.".to_owned()),
        );
    }
    let Some(key) = error.key else {
        return StartupError::new(
            AppErrorKind::Invariant,
            "config option key is missing",
            None,
        );
    };
    let name = option_name(key, &error.source);
    StartupError::new(
        AppErrorKind::Validation,
        format!(
            "invalid config option {name} from {source}: {}",
            option_reason(key, &error.reason)
        ),
        Some(format!("Fix {name} in {source}.")),
    )
}

fn parse_error(error: ConfigParseError) -> StartupError {
    let reason = match error.kind {
        ConfigParseErrorKind::TooLarge => "too large",
        ConfigParseErrorKind::InvalidUtf8 => "invalid UTF-8",
        ConfigParseErrorKind::ByteOrderMark => "byte-order mark",
        ConfigParseErrorKind::InvalidToml => "invalid TOML",
        ConfigParseErrorKind::TooDeep => "nesting too deep",
    };
    StartupError::new(
        AppErrorKind::Validation,
        format!("invalid config file {}: {reason}", error.path.display()),
        Some("Fix or remove the config file.".to_owned()),
    )
}

fn config_failure(error: ConfigFailure) -> StartupError {
    match error {
        ConfigFailure::Oversize { path } => StartupError::new(
            AppErrorKind::Validation,
            format!("invalid environment file {}: too large", path.display()),
            Some("Fix the .env file or set LINEAR_IGNORE_ENV_FILE=1.".to_owned()),
        ),
        ConfigFailure::InvalidUtf8 { path } => StartupError::new(
            AppErrorKind::Validation,
            format!("invalid environment file {}: invalid UTF-8", path.display()),
            Some("Fix the .env file or set LINEAR_IGNORE_ENV_FILE=1.".to_owned()),
        ),
        ConfigFailure::InvalidInput(_) => StartupError::new(
            AppErrorKind::Invariant,
            "config working directory must be absolute",
            None,
        ),
        ConfigFailure::GitProbe(error) => {
            let reason = match error {
                GitProbeError::Timeout => "timed out",
                GitProbeError::Oversize => "output too large",
                GitProbeError::InvalidUtf8 | GitProbeError::MalformedStdout => "invalid output",
                GitProbeError::Io { .. } => "I/O failure",
            };
            StartupError::new(
                AppErrorKind::IoProcess,
                format!("failed to determine Git root: {reason}"),
                None,
            )
        }
    }
}

fn read_tier(
    candidates: &[ConfigCandidate],
    files: &impl FileSource,
) -> Result<Option<ConfigTier>, StartupError> {
    for candidate in candidates {
        match read_config_candidate(files, &candidate.path) {
            ReadCandidate::Absent => {}
            ReadCandidate::Contents(raw) => {
                return parse_config_tier(raw).map(Some).map_err(parse_error);
            }
            ReadCandidate::TooLarge { path } => {
                return Err(StartupError::new(
                    AppErrorKind::Validation,
                    format!("invalid config file {}: too large", path.display()),
                    Some("Fix or remove the config file.".to_owned()),
                ));
            }
            ReadCandidate::Poisoned { path, reason } => {
                return Err(StartupError::new(
                    AppErrorKind::Validation,
                    format!("cannot read config file {}: {reason}", path.display()),
                    Some("Fix or remove the config file.".to_owned()),
                ));
            }
        }
    }
    Ok(None)
}

fn fail(
    settings: DisplaySettings,
    diagnostics: Vec<ConfigDiagnostic>,
    error: StartupError,
) -> StartupReport {
    StartupReport {
        settings,
        diagnostics,
        result: Err(error),
    }
}

/// Read and parse all config tiers once, retaining warnings even on failure.
pub fn load_startup(
    process: &ProcessEnvSnapshot,
    files: &impl FileSource,
    git: &impl GitRootProbe,
) -> StartupReport {
    let initial_settings = settings(process, None);
    let git = MemoGit {
        git,
        cache: OnceLock::new(),
    };
    let dotenv = match load_env(&process.inputs, files, &git) {
        Ok(dotenv) => dotenv,
        Err(error) => {
            return fail(
                initial_settings,
                error.diagnostics,
                config_failure(error.failure),
            );
        }
    };
    let display = settings(process, Some(&dotenv));
    let diagnostics = dotenv.diagnostics.clone();
    let paths = match discover_config_paths(&process.inputs, &git) {
        Ok(paths) => paths,
        Err(error) => return fail(display, diagnostics, config_failure(error)),
    };
    let global = match read_tier(&paths.global, files) {
        Ok(global) => global,
        Err(error) => return fail(display, diagnostics, error),
    };
    let project = match read_tier(&paths.project, files) {
        Ok(project) => project,
        Err(error) => return fail(display, diagnostics, error),
    };
    let options = match ConfigOptions::from_inputs(OptionInputs {
        env: &process.inputs,
        dotenv: &dotenv,
        project: project.as_ref(),
        global: global.as_ref(),
    }) {
        Ok(options) => options,
        Err(error) => return fail(display, diagnostics, option_error(error)),
    };
    let overlay = dotenv
        .applied
        .into_iter()
        .filter(|(name, _)| {
            let normalized = if process.inputs.os == OsFamily::Windows {
                name.to_ascii_uppercase()
            } else {
                name.clone()
            };
            !process.inputs.process_env.contains_key(&normalized)
        })
        .collect();
    StartupReport {
        settings: display,
        diagnostics,
        result: Ok(StartupConfig {
            options,
            child_env: ChildEnvOverlay {
                os: process.inputs.os,
                values: overlay,
            },
        }),
    }
}

pub fn render_diagnostic(diagnostic: &ConfigDiagnostic, color: bool) -> String {
    let (message, suggestion) = match &diagnostic.reason {
        DiagnosticReason::Unusable(reason) => (
            format!(
                "Warning: Ignoring {}: {reason}. No variables were loaded from it.",
                diagnostic.path.display()
            ),
            "  Set LINEAR_IGNORE_ENV_FILE=1 to skip .env loading entirely.",
        ),
        DiagnosticReason::SkippedExpansion(keys) => (
            format!(
                "Warning: Ignoring {} in {}: the value references a shell variable, which linear does not expand.",
                keys.join(", "),
                diagnostic.path.display()
            ),
            "  Write the literal value, or set the variable in your environment instead.",
        ),
        DiagnosticReason::UnterminatedQuote(keys) => (
            format!(
                "Warning: Ignoring {} in {}: the value opens a quote it never closes on the same line.",
                keys.join(", "),
                diagnostic.path.display()
            ),
            "  linear does not support values that span multiple lines.",
        ),
    };
    if color {
        format!("\x1b[33m{message}\x1b[39m\n\x1b[90m{suggestion}\x1b[39m\n")
    } else {
        format!("{message}\n{suggestion}\n")
    }
}
