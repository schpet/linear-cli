use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use super::source::{
    ConfigInputs, FileKind, FileSource, GitProbeError, GitProbeResult, GitRootProbe,
    MAX_CONFIG_BYTES, absent, lexical,
};

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
    GitProbe(GitProbeError),
}

impl fmt::Display for ConfigFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Oversize { path } => {
                write!(f, "{} exceeds {MAX_CONFIG_BYTES} bytes", path.display())
            }
            Self::InvalidUtf8 { path } => write!(f, "{} is not valid UTF-8", path.display()),
            Self::InvalidInput(reason) => f.write_str(reason),
            Self::GitProbe(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ConfigFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::GitProbe(error) => Some(error),
            Self::Oversize { .. } | Self::InvalidUtf8 { .. } | Self::InvalidInput(_) => None,
        }
    }
}

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

fn assignment(line: &str) -> Option<(&str, &str)> {
    // The source filter uses a JS regular expression whose dot cannot cross
    // these line terminators, even though it splits only on LF/CRLF first.
    if line.contains(['\r', '\u{2028}', '\u{2029}']) {
        return None;
    }
    let line = line.trim_start_matches([' ', '\t']);
    let line = if let Some(rest) = line.strip_prefix("export ") {
        rest.trim_start_matches([' ', '\t'])
    } else if let Some(rest) = line.strip_prefix("export\t") {
        rest.trim_start_matches([' ', '\t'])
    } else {
        line
    };
    let (key, raw) = line.split_once('=')?;
    let key = key.trim_end_matches([' ', '\t']);
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

fn unquoted_reference(value: &str) -> bool {
    let bytes = value.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if *byte != b'$' {
            continue;
        }
        let after = bytes.get(index + 1);
        if after == Some(&b'{') {
            if bytes
                .get(index + 3..)
                .is_some_and(|tail| tail.contains(&b'}'))
            {
                return true;
            }
        } else if index > 0 && bytes.get(index - 1) == Some(&b'\\') {
            continue;
        } else if after.is_some_and(|value| value.is_ascii_alphanumeric() || *value == b'_') {
            return true;
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
    let value = raw.trim_start();
    let Some(quote) = value.chars().next().filter(|ch| *ch == '\'' || *ch == '"') else {
        let unquoted = value
            .split_once('#')
            .map_or(value, |(before, _)| before)
            .trim_end();
        return if unquoted_reference(unquoted) {
            Value::Expansion
        } else {
            Value::Parsed(unquoted.to_owned())
        };
    };
    let mut escaped = false;
    let mut terminated = false;
    for ch in value.chars().skip(1) {
        if quote == '"' && escaped {
            escaped = false;
        } else if quote == '"' && ch == '\\' {
            escaped = true;
        } else if ch == quote {
            terminated = true;
            break;
        }
    }
    if !terminated {
        return Value::Unterminated;
    }
    // The source filter checks escapes before accepting a closing quote, but
    // pinned @std/dotenv 0.225.6 then captures through the first quote even
    // when escaped. Keep these two stages separate.
    let Some((content, _)) = value
        .strip_prefix(quote)
        .and_then(|rest| rest.split_once(quote))
    else {
        return Value::Unterminated;
    };
    if quote == '\'' {
        return Value::Parsed(content.to_owned());
    }
    let mut expanded = String::new();
    let mut chars = content.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            let mapped = match chars.peek() {
                Some('n') => Some('\n'),
                Some('r') => Some('\r'),
                Some('t') => Some('\t'),
                Some(_) | None => None,
            };
            if let Some(mapped) = mapped {
                chars.next();
                expanded.push(mapped);
                continue;
            }
        }
        expanded.push(ch);
    }
    Value::Parsed(expanded)
}

fn parse_selected(
    text: &str,
    process_env: &BTreeMap<String, String>,
    path: &Path,
) -> (BTreeMap<String, String>, Vec<ConfigDiagnostic>) {
    let mut parsed = BTreeMap::new();
    let mut expansion = Vec::new();
    let mut unterminated = Vec::new();
    for line in text.lines() {
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

pub fn load_env(
    inputs: &ConfigInputs,
    files: &impl FileSource,
    git: &impl GitRootProbe,
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
    let mut selected_path = cwd_path.clone();
    let mut file = read_env(files, &cwd_path).map_err(|failure| LoadEnvError {
        diagnostics: result.diagnostics.clone(),
        failure,
    })?;
    if let EnvFile::Unusable(reason) = &file {
        result.diagnostics.push(ConfigDiagnostic {
            path: cwd_path.clone(),
            reason: DiagnosticReason::Unusable(reason.clone()),
        });
    }
    if !matches!(file, EnvFile::Loaded(_)) {
        match git.probe() {
            GitProbeResult::Completed {
                success: true,
                stdout,
            } => {
                let root = stdout.trim();
                if !root.is_empty() {
                    let root_path = lexical(&PathBuf::from(root).join(".env"));
                    if root_path != cwd_path {
                        selected_path = root_path.clone();
                        file = read_env(files, &root_path).map_err(|failure| LoadEnvError {
                            diagnostics: result.diagnostics.clone(),
                            failure,
                        })?;
                        if let EnvFile::Unusable(reason) = &file {
                            result.diagnostics.push(ConfigDiagnostic {
                                path: root_path,
                                reason: DiagnosticReason::Unusable(reason.clone()),
                            });
                        }
                    }
                }
            }
            GitProbeResult::Failed(error) => {
                return Err(LoadEnvError {
                    diagnostics: result.diagnostics,
                    failure: ConfigFailure::GitProbe(error),
                });
            }
            GitProbeResult::SpawnFailure | GitProbeResult::Completed { success: false, .. } => {}
        }
    }
    if let EnvFile::Loaded(text) = file {
        let (applied, diagnostics) = parse_selected(&text, &inputs.process_env, &selected_path);
        result.applied = applied;
        result.source_path = Some(selected_path);
        result.diagnostics.extend(diagnostics);
    }
    Ok(result)
}
