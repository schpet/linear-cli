use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use super::source::{ConfigInputs, FileKind, FileSource, MAX_CONFIG_BYTES, absent, lexical};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiagnosticReason {
    Unusable(String),
    SkippedExpansion(Vec<String>),
    UnterminatedQuote(Vec<String>),
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

/// Splits `KEY=value` (optionally prefixed with `export`) and keeps only the
/// keys this program reads.
fn assignment(line: &str) -> Option<(&str, &str)> {
    let line = line.trim_start();
    let line = line
        .strip_prefix("export")
        .filter(|rest| rest.starts_with([' ', '\t']))
        .map_or(line, str::trim_start);
    let (key, raw) = line.split_once('=')?;
    let key = key.trim_end();
    let mut chars = key.chars();
    let first = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_')
        || !chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return None;
    }
    if !["LINEAR_", "GH_", "GITHUB_"]
        .iter()
        .any(|prefix| key.starts_with(prefix))
    {
        return None;
    }
    Some((key, raw))
}

/// Whether `value` contains `$NAME` or `${...}`, which this parser does not
/// expand. `\$` is a literal dollar sign.
fn references_variable(value: &str) -> bool {
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => {
                chars.next();
            }
            '$' => {
                if chars
                    .peek()
                    .is_some_and(|next| *next == '{' || *next == '_' || next.is_ascii_alphabetic())
                {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

enum Value {
    Parsed(String),
    Expansion,
    Unterminated,
}

fn parse_value(raw: &str) -> Value {
    let value = raw.trim();
    if let Some(rest) = value.strip_prefix('\'') {
        return match rest.split_once('\'') {
            Some((literal, _)) => Value::Parsed(literal.to_owned()),
            None => Value::Unterminated,
        };
    }
    if let Some(rest) = value.strip_prefix('"') {
        let mut parsed = String::new();
        let mut chars = rest.chars();
        while let Some(ch) = chars.next() {
            match ch {
                '"' => {
                    return if references_variable(rest) {
                        Value::Expansion
                    } else {
                        Value::Parsed(parsed)
                    };
                }
                '\\' => match chars.next() {
                    Some('n') => parsed.push('\n'),
                    Some('r') => parsed.push('\r'),
                    Some('t') => parsed.push('\t'),
                    Some(other) => parsed.push(other),
                    None => return Value::Unterminated,
                },
                other => parsed.push(other),
            }
        }
        return Value::Unterminated;
    }
    // An unquoted value ends at a ` #` comment.
    let unquoted = value
        .find(" #")
        .or_else(|| value.find("\t#"))
        .map_or(value, |end| value.get(..end).unwrap_or(value))
        .trim_end();
    if references_variable(unquoted) {
        Value::Expansion
    } else {
        Value::Parsed(unquoted.to_owned())
    }
}

fn parse_selected(
    text: &str,
    process_env: &BTreeMap<String, String>,
    path: &Path,
) -> (BTreeMap<String, String>, Vec<ConfigDiagnostic>) {
    let mut parsed = BTreeMap::new();
    let mut expansion = Vec::new();
    let mut unterminated = Vec::new();
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    for line in text.lines() {
        if line.trim_start().starts_with('#') {
            continue;
        }
        let Some((key, raw)) = assignment(line) else {
            continue;
        };
        match parse_value(raw) {
            Value::Parsed(value) => {
                parsed.insert(key.to_owned(), value);
            }
            Value::Expansion => expansion.push(key.to_owned()),
            Value::Unterminated => unterminated.push(key.to_owned()),
        }
    }
    let applied = parsed
        .into_iter()
        .filter(|(key, _)| !process_env.contains_key(key))
        .collect::<BTreeMap<_, _>>();
    let warnable = |key: &String| !process_env.contains_key(key) && !applied.contains_key(key);
    expansion.retain(warnable);
    unterminated.retain(warnable);
    let mut diagnostics = Vec::new();
    if !expansion.is_empty() {
        diagnostics.push(ConfigDiagnostic {
            path: path.to_owned(),
            reason: DiagnosticReason::SkippedExpansion(expansion),
        });
    }
    if !unterminated.is_empty() {
        diagnostics.push(ConfigDiagnostic {
            path: path.to_owned(),
            reason: DiagnosticReason::UnterminatedQuote(unterminated),
        });
    }
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
