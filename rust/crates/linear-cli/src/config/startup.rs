//! Loads `.env` and config files into one startup result.
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt;

use crate::error::{AppError, AppErrorKind};

use super::discover::{ConfigCandidate, discover_config_paths};
use super::dotenv::{ConfigDiagnostic, ConfigFailure, DiagnosticReason, SelectedEnv, load_env};
use super::options::{ConfigOptionError, ConfigOptions, OptionInputs};
use super::parse::{ConfigParseError, ConfigTier, parse_config_tier};
use super::runtime::ProcessEnvSnapshot;
use super::source::{FileSource, OsFamily, ReadCandidate, read_config_candidate, repo_root};
use super::transport::TransportEnvInputs;

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
    pub transport_env: TransportEnvInputs,
    /// The process `CI` value; `.env` files cannot set it.
    pub ci: Option<String>,
    /// Process PAGER value; dotenv files cannot set it either.
    pub pager: Option<OsString>,
    /// Where downloaded Markdown images are cached.
    pub image_cache_root: std::path::PathBuf,
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

fn option_error(error: ConfigOptionError) -> StartupError {
    let app = AppError::from(error);
    StartupError::new(app.kind, app.message, app.suggestion)
}

fn parse_error(error: ConfigParseError) -> StartupError {
    StartupError::new(
        AppErrorKind::Validation,
        format!("invalid config file {error}"),
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

/// Reads and validates `.env` and all config tiers once, keeping warnings
/// even on failure.
pub fn load_startup(process: &ProcessEnvSnapshot, files: &impl FileSource) -> StartupReport {
    let initial_settings = settings(process, None);
    let root = repo_root(&process.inputs.cwd, files);
    let dotenv = match load_env(&process.inputs, files, root.as_deref()) {
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
    let paths = discover_config_paths(&process.inputs, root.as_deref());
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
    let child_env = ChildEnvOverlay {
        os: process.inputs.os,
        values: overlay,
    };
    let ci = process.inputs.env("CI").map(str::to_owned);
    let pager = process.pager.clone();
    StartupReport {
        settings: display,
        diagnostics,
        result: Ok(StartupConfig {
            options,
            child_env,
            ci,
            pager,
            image_cache_root: crate::platform::markdown_assets::cache_root(
                process.inputs.env("TMPDIR"),
                process.inputs.env("TMP"),
                process.inputs.env("TEMP"),
            ),
            transport_env: TransportEnvInputs::from_process(process),
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
        DiagnosticReason::InvalidLines(keys) => (
            format!(
                "Warning: Ignoring {} in {}: the line could not be parsed.",
                keys.join(", "),
                diagnostic.path.display()
            ),
            "  Check for an unclosed quote or a malformed KEY=value line.",
        ),
    };
    if color {
        format!("\x1b[33m{message}\x1b[39m\n\x1b[90m{suggestion}\x1b[39m\n")
    } else {
        format!("{message}\n{suggestion}\n")
    }
}
