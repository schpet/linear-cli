//! Loads `.env` and config files into one startup result.
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt;

use crate::error::Error;
use crate::platform::style;

use super::discover::{ConfigCandidate, discover_config_paths};
use super::dotenv::{ConfigDiagnostic, ConfigFailure, DiagnosticReason, SelectedEnv, load_env};
use super::network::NetworkEnv;
use super::options::{
    ConfigOptionError, ConfigOptions, OptionInputs, OptionKey, OptionSource, retired_vcs,
};
use super::parse::{ConfigParseError, ConfigTier, parse_config_tier};
use super::runtime::ProcessEnvSnapshot;
use super::source::{FileSource, OsFamily, ReadCandidate, read_config_candidate, repo_root};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DisplaySettings {
    pub debug: bool,
    /// `NO_COLOR` is set to a nonempty value.
    pub no_color: bool,
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

    /// These values with `name` set to `value`, replacing any value it has.
    pub fn with(&self, name: &str, value: String) -> Self {
        let mut values = self.values.clone();
        if self.os == OsFamily::Windows {
            values.retain(|key, _| !key.eq_ignore_ascii_case(name));
        }
        values.insert(name.to_owned(), value);
        Self {
            os: self.os,
            values,
        }
    }

    /// No `.env` values.
    #[cfg(test)]
    pub(crate) fn empty() -> Self {
        Self {
            os: if cfg!(windows) {
                OsFamily::Windows
            } else {
                OsFamily::Unix
            },
            values: BTreeMap::new(),
        }
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
    pub network_env: NetworkEnv,
    /// Process PAGER value; dotenv files cannot set it either.
    pub pager: Option<OsString>,
    /// The per-user cache directory for downloads, when one can be found.
    pub cache_dir: Option<std::path::PathBuf>,
    /// The process environment names a terminal with kitty graphics.
    pub kitty_graphics: bool,
    /// Process `LESSUTFCHARDEF`, extended for the pager when it shows images.
    pub less_utf_chardef: Option<String>,
}

impl fmt::Debug for StartupConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StartupConfig(<redacted>)")
    }
}

/// A warning about the configuration, printed on stderr before the command
/// runs (or fails).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StartupDiagnostic {
    /// A `.env` file, or lines of it, that were not loaded.
    Dotenv(ConfigDiagnostic),
    /// A source that sets `vcs` (`LINEAR_VCS`), which 4.0 removed.
    RetiredVcs(OptionSource),
}

pub struct StartupReport {
    pub settings: DisplaySettings,
    pub diagnostics: Vec<StartupDiagnostic>,
    pub result: Result<StartupConfig, Error>,
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
    let no_color = process
        .inputs
        .env("NO_COLOR")
        .is_some_and(|value| !value.is_empty());
    let debug = match dotenv {
        Some(dotenv) => matches!(
            effective(process, dotenv, "LINEAR_DEBUG"),
            Some("1" | "true")
        ),
        None => matches!(process.inputs.env("LINEAR_DEBUG"), Some("1" | "true")),
    };
    DisplaySettings { debug, no_color }
}

fn option_error(error: ConfigOptionError) -> Error {
    Error::from(error)
}

fn parse_error(error: ConfigParseError) -> Error {
    Error::new(format!("invalid config file {error}")).with_hint("Fix or remove the config file.")
}

fn config_failure(error: ConfigFailure) -> Error {
    match error {
        ConfigFailure::Oversize { path } => Error::new(format!(
            "invalid environment file {}: too large",
            path.display()
        ))
        .with_hint("Fix the .env file or set LINEAR_IGNORE_ENV_FILE=1."),
        ConfigFailure::InvalidUtf8 { path } => Error::new(format!(
            "invalid environment file {}: invalid UTF-8",
            path.display()
        ))
        .with_hint("Fix the .env file or set LINEAR_IGNORE_ENV_FILE=1."),
        ConfigFailure::InvalidInput(_) => Error::new("config working directory must be absolute"),
    }
}

fn read_tier(
    candidates: &[ConfigCandidate],
    files: &impl FileSource,
) -> Result<Option<ConfigTier>, Error> {
    for candidate in candidates {
        match read_config_candidate(files, &candidate.path) {
            ReadCandidate::Absent => {}
            ReadCandidate::Contents(raw) => {
                return parse_config_tier(raw).map(Some).map_err(parse_error);
            }
            ReadCandidate::TooLarge { path } => {
                return Err(Error::new(format!(
                    "invalid config file {}: too large",
                    path.display()
                ))
                .with_hint("Fix or remove the config file."));
            }
            ReadCandidate::Poisoned { path, reason } => {
                return Err(Error::new(format!(
                    "cannot read config file {}: {reason}",
                    path.display()
                ))
                .with_hint("Fix or remove the config file."));
            }
        }
    }
    Ok(None)
}

fn fail(
    settings: DisplaySettings,
    diagnostics: Vec<StartupDiagnostic>,
    error: Error,
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
                error
                    .diagnostics
                    .into_iter()
                    .map(StartupDiagnostic::Dotenv)
                    .collect(),
                config_failure(error.failure),
            );
        }
    };
    let display = settings(process, Some(&dotenv));
    let mut diagnostics: Vec<StartupDiagnostic> = dotenv
        .diagnostics
        .iter()
        .cloned()
        .map(StartupDiagnostic::Dotenv)
        .collect();
    let paths = discover_config_paths(&process.inputs, root.as_deref());
    let global = match read_tier(&paths.global, files) {
        Ok(global) => global,
        Err(error) => return fail(display, diagnostics, error),
    };
    let project = match read_tier(&paths.project, files) {
        Ok(project) => project,
        Err(error) => return fail(display, diagnostics, error),
    };
    let inputs = OptionInputs {
        env: &process.inputs,
        dotenv: &dotenv,
        project: project.as_ref(),
        global: global.as_ref(),
    };
    // Collected first so that the warning survives another option's error.
    // An invalid value is not lost here: `from_inputs` checks the same
    // values and fails with it.
    if let Ok(sources) = retired_vcs(&inputs) {
        diagnostics.extend(sources.into_iter().map(StartupDiagnostic::RetiredVcs));
    }
    let options = match ConfigOptions::from_inputs(inputs) {
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
    let pager = process.pager.clone();
    StartupReport {
        settings: display,
        diagnostics,
        result: Ok(StartupConfig {
            options,
            child_env,
            pager,
            cache_dir: crate::platform::markdown_assets::cache_root(
                process.inputs.os,
                process.inputs.env("XDG_CACHE_HOME"),
                process.inputs.env("HOME"),
                process.inputs.env("LOCALAPPDATA"),
            ),
            network_env: NetworkEnv::from_process(process),
            kitty_graphics: crate::platform::kitty::supported(|name| process.inputs.env(name)),
            less_utf_chardef: process.inputs.env("LESSUTFCHARDEF").map(str::to_owned),
        }),
    }
}

pub fn render_diagnostic(diagnostic: &StartupDiagnostic, color: bool) -> String {
    let (message, suggestion) = match diagnostic {
        StartupDiagnostic::Dotenv(diagnostic) => render_dotenv(diagnostic),
        StartupDiagnostic::RetiredVcs(source) => {
            let name = OptionKey::RetiredVcs.spelling(source);
            (
                format!(
                    "Warning: {name} from {} was removed in 4.0 and does nothing.",
                    source.label()
                ),
                format!(
                    "  Remove {name}. Commands name their tool instead: `linear issue jj pr` and `linear issue jj start` for jj, `linear issue gh pr` and `linear issue git start` for git."
                ),
            )
        }
    };
    format!(
        "{}\n{}\n",
        style::warning(&message, color),
        style::gray(&suggestion, color)
    )
}

fn render_dotenv(diagnostic: &ConfigDiagnostic) -> (String, String) {
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
        DiagnosticReason::Unexpanded { key, reference } => (
            format!(
                "Warning: Ignoring {key} in {}: .env values are not expanded, so {reference} would be used literally.",
                diagnostic.path.display()
            ),
            "  Put the value itself in .env or the environment, or single-quote it to keep a literal $.",
        ),
    };
    (message, suggestion.to_owned())
}

#[cfg(test)]
mod tests;
