use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use super::source::{ConfigInputs, FileKind, FileSource, MAX_CONFIG_BYTES, absent, lexical};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiagnosticReason {
    Unusable(String),
    /// Lines for these keys could not be parsed.
    InvalidLines(Vec<String>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigDiagnostic {
    pub path: PathBuf,
    pub reason: DiagnosticReason,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfigFailure {
    Oversize { path: PathBuf },
    InvalidUtf8 { path: PathBuf },
    InvalidInput(String),
}

impl fmt::Display for ConfigFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Oversize { path } => {
                write!(f, "{} exceeds {MAX_CONFIG_BYTES} bytes", path.display())
            }
            Self::InvalidUtf8 { path } => write!(f, "{} is not valid UTF-8", path.display()),
            Self::InvalidInput(reason) => f.write_str(reason),
        }
    }
}

impl std::error::Error for ConfigFailure {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadEnvError {
    pub diagnostics: Vec<ConfigDiagnostic>,
    pub failure: ConfigFailure,
}

impl fmt::Display for LoadEnvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.failure.fmt(f)
    }
}

impl std::error::Error for LoadEnvError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.failure)
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct SelectedEnv {
    pub applied: BTreeMap<String, String>,
    pub source_path: Option<PathBuf>,
    pub diagnostics: Vec<ConfigDiagnostic>,
}

enum EnvFile {
    Absent,
    Unusable(String),
    Loaded(String),
}

fn read_env(files: &impl FileSource, path: &Path) -> Result<EnvFile, ConfigFailure> {
    match files.kind(path) {
        Ok(None) => Ok(EnvFile::Absent),
        Ok(Some(FileKind::Directory)) => Ok(EnvFile::Unusable(
            "it is a directory, not a file".to_owned(),
        )),
        Ok(Some(FileKind::Other)) => Ok(EnvFile::Unusable("it is not a regular file".to_owned())),
        Ok(Some(FileKind::Regular)) => match files.read_bounded(path, MAX_CONFIG_BYTES) {
            Ok(bytes) if u64::try_from(bytes.len()).is_ok_and(|len| len <= MAX_CONFIG_BYTES) => {
                String::from_utf8(bytes).map(EnvFile::Loaded).map_err(|_| {
                    ConfigFailure::InvalidUtf8 {
                        path: path.to_owned(),
                    }
                })
            }
            Ok(_) => Err(ConfigFailure::Oversize {
                path: path.to_owned(),
            }),
            Err(error) if absent(&error) => Ok(EnvFile::Absent),
            Err(error) => Ok(EnvFile::Unusable(error.kind().to_string())),
        },
        Err(error) => Ok(EnvFile::Unusable(error.kind().to_string())),
    }
}

/// Only these variables are read from `.env` files.
fn admitted(key: &str) -> bool {
    ["LINEAR_", "GH_", "GITHUB_"]
        .iter()
        .any(|prefix| key.starts_with(prefix))
}

/// The key a line that failed to parse was meant to set, if any.
fn intended_key(line: &str) -> Option<&str> {
    let line = line.trim_start();
    let line = line.strip_prefix("export ").unwrap_or(line);
    let (key, _) = line.split_once('=')?;
    Some(key.trim())
}

/// Parses a `.env` file with dotenvy (which expands `$VAR` references) and
/// keeps the admitted keys the process environment does not already set.
fn parse_selected(
    text: &str,
    process_env: &BTreeMap<String, String>,
    path: &Path,
) -> (BTreeMap<String, String>, Vec<ConfigDiagnostic>) {
    let mut applied = BTreeMap::new();
    let mut invalid = Vec::new();
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    for item in dotenvy::from_read_iter(text.as_bytes()) {
        match item {
            Ok((key, value)) => {
                if admitted(&key) && !process_env.contains_key(&key) {
                    applied.insert(key, value);
                }
            }
            Err(dotenvy::Error::LineParse(line, _)) => {
                if let Some(key) = intended_key(&line).filter(|key| admitted(key)) {
                    invalid.push(key.to_owned());
                }
            }
            // Reading from memory cannot fail, and failed substitutions
            // expand to an empty string rather than an error.
            Err(error) => invalid.push(format!("(unreadable line: {error})")),
        }
    }
    invalid.retain(|key| !process_env.contains_key(key) && !applied.contains_key(key));
    let diagnostics = if invalid.is_empty() {
        Vec::new()
    } else {
        vec![ConfigDiagnostic {
            path: path.to_owned(),
            reason: DiagnosticReason::InvalidLines(invalid),
        }]
    };
    (applied, diagnostics)
}

/// Loads `.env` from the working directory, or else from the repository
/// root. Process environment values always win over the file.
pub fn load_env(
    inputs: &ConfigInputs,
    files: &impl FileSource,
    repo_root: Option<&Path>,
) -> Result<SelectedEnv, LoadEnvError> {
    if !inputs.cwd.is_absolute() {
        return Err(LoadEnvError {
            diagnostics: Vec::new(),
            failure: ConfigFailure::InvalidInput("cwd must be absolute".to_owned()),
        });
    }
    let mut result = SelectedEnv {
        applied: BTreeMap::new(),
        source_path: None,
        diagnostics: Vec::new(),
    };
    if matches!(inputs.env("LINEAR_IGNORE_ENV_FILE"), Some("1" | "true")) {
        return Ok(result);
    }
    let cwd_path = lexical(&inputs.cwd.join(".env"));
    let root_path = repo_root
        .map(|root| lexical(&root.join(".env")))
        .filter(|path| *path != cwd_path);
    for path in std::iter::once(cwd_path).chain(root_path) {
        match read_env(files, &path) {
            Err(failure) => {
                return Err(LoadEnvError {
                    diagnostics: result.diagnostics,
                    failure,
                });
            }
            Ok(EnvFile::Absent) => {}
            Ok(EnvFile::Unusable(reason)) => result.diagnostics.push(ConfigDiagnostic {
                path,
                reason: DiagnosticReason::Unusable(reason),
            }),
            Ok(EnvFile::Loaded(text)) => {
                let (applied, diagnostics) = parse_selected(&text, &inputs.process_env, &path);
                result.applied = applied;
                result.source_path = Some(path);
                result.diagnostics.extend(diagnostics);
                break;
            }
        }
    }
    Ok(result)
}
