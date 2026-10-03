use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use super::source::{ConfigInputs, FileKind, FileSource, MAX_CONFIG_BYTES, absent, lexical};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiagnosticReason {
    Unusable(String),
    /// Lines for these keys could not be parsed.
    InvalidLines(Vec<String>),
    /// The value for `key` refers to the variable `name` in a way that is not
    /// expanded: a bare `$name`, or `${name}` when it is not set.
    Reference {
        key: String,
        name: String,
        braced: bool,
    },
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

/// Why a `.env` line was not used.
enum Problem {
    Malformed,
    Reference { name: String, braced: bool },
}

/// One `KEY=value` line, or why it was rejected. `key` is `None` when the
/// line does not name one.
struct Entry {
    key: Option<String>,
    value: Result<String, Problem>,
}

/// Parses `.env` text: `[export] KEY=value` lines and `#` comments. Single
/// quotes keep their content literally; double-quoted and unquoted values
/// take `\` escapes and `${NAME}` references to the process environment or
/// an earlier key in the file. A bare `$NAME` is rejected rather than
/// guessed at, and an unset `${NAME}` is an error, never an empty string.
/// Quoted values may span lines.
fn entries(text: &str, process_env: &BTreeMap<String, String>) -> Vec<Entry> {
    let text = text.replace("\r\n", "\n");
    let mut chars = text.chars().peekable();
    let mut entries = Vec::new();
    let mut earlier = BTreeMap::new();
    loop {
        while chars.next_if(|c| c.is_whitespace()).is_some() {}
        let Some(first) = chars.peek().copied() else {
            break;
        };
        if first == '#' {
            skip_line(&mut chars);
            continue;
        }
        let head: String =
            std::iter::from_fn(|| chars.next_if(|c| !matches!(c, '=' | '\n'))).collect();
        if chars.next_if_eq(&'=').is_none() {
            entries.push(Entry {
                key: None,
                value: Err(Problem::Malformed),
            });
            continue;
        }
        let head = head.trim();
        let key = head
            .strip_prefix("export ")
            .unwrap_or(head)
            .trim()
            .to_owned();
        let value = if valid_key(&key) {
            value(&mut chars, process_env, &earlier)
        } else {
            skip_line(&mut chars);
            Err(Problem::Malformed)
        };
        if let Ok(value) = &value {
            earlier.insert(key.clone(), value.clone());
        }
        entries.push(Entry {
            key: Some(key),
            value,
        });
    }
    entries
}

type Chars<'a> = std::iter::Peekable<std::str::Chars<'a>>;

fn skip_line(chars: &mut Chars<'_>) {
    while chars.next_if(|c| *c != '\n').is_some() {}
}

fn valid_key(key: &str) -> bool {
    key.chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.'))
}

/// The value after `KEY=`, consuming the rest of its line.
fn value(
    chars: &mut Chars<'_>,
    process_env: &BTreeMap<String, String>,
    earlier: &BTreeMap<String, String>,
) -> Result<String, Problem> {
    while chars.next_if(|c| matches!(c, ' ' | '\t')).is_some() {}
    let mut out = String::new();
    // The first problem is reported once the whole value is consumed.
    let mut problem = None;
    let mut note = |result: Result<(), Problem>| {
        if let Err(error) = result {
            problem.get_or_insert(error);
        }
    };
    match chars.peek() {
        Some('\'') => {
            chars.next();
            loop {
                match chars.next() {
                    Some('\'') => break,
                    Some(c) => out.push(c),
                    None => return Err(Problem::Malformed),
                }
            }
            note(end_of_line(chars));
        }
        Some('"') => {
            chars.next();
            loop {
                match chars.next() {
                    Some('"') => break,
                    Some(c) => note(special(c, chars, &mut out, process_env, earlier)),
                    None => return Err(Problem::Malformed),
                }
            }
            note(end_of_line(chars));
        }
        _ => {
            let mut after_space = true;
            while let Some(c) = chars.next_if(|c| *c != '\n') {
                if c == '#' && after_space {
                    skip_line(chars);
                    break;
                }
                after_space = matches!(c, ' ' | '\t');
                note(special(c, chars, &mut out, process_env, earlier));
            }
            out.truncate(out.trim_end().len());
        }
    }
    match problem {
        Some(problem) => Err(problem),
        None => Ok(out),
    }
}

/// Appends `c` to `out`, handling a `\` escape or a `$` reference it starts.
fn special(
    c: char,
    chars: &mut Chars<'_>,
    out: &mut String,
    process_env: &BTreeMap<String, String>,
    earlier: &BTreeMap<String, String>,
) -> Result<(), Problem> {
    let name_char = |c: &char| c.is_ascii_alphanumeric() || *c == '_';
    match c {
        '\\' => match chars.next_if(|c| *c != '\n') {
            Some('n') => out.push('\n'),
            Some(escaped @ ('\\' | '\'' | '"' | '$' | ' ')) => out.push(escaped),
            _ => return Err(Problem::Malformed),
        },
        '$' if chars.next_if_eq(&'{').is_some() => {
            let name: String = std::iter::from_fn(|| chars.next_if(name_char)).collect();
            if name.is_empty() || chars.next_if_eq(&'}').is_none() {
                return Err(Problem::Malformed);
            }
            // The snapshot holds only the variables this program reads, so
            // other names come from the live environment.
            let value = process_env
                .get(&name)
                .cloned()
                .or_else(|| std::env::var(&name).ok())
                .or_else(|| earlier.get(&name).cloned());
            match value {
                Some(value) => out.push_str(&value),
                None => return Err(Problem::Reference { name, braced: true }),
            }
        }
        '$' if chars
            .peek()
            .is_some_and(|c| c.is_ascii_alphabetic() || *c == '_') =>
        {
            let name = std::iter::from_fn(|| chars.next_if(name_char)).collect();
            return Err(Problem::Reference {
                name,
                braced: false,
            });
        }
        c => out.push(c),
    }
    Ok(())
}

/// After a closing quote only whitespace or a `#` comment may follow.
fn end_of_line(chars: &mut Chars<'_>) -> Result<(), Problem> {
    while chars.next_if(|c| matches!(c, ' ' | '\t')).is_some() {}
    match chars.peek() {
        None | Some('\n') => Ok(()),
        Some('#') => {
            skip_line(chars);
            Ok(())
        }
        Some(_) => {
            skip_line(chars);
            Err(Problem::Malformed)
        }
    }
}

/// Parses a `.env` file and keeps the admitted keys the process environment
/// does not already set.
fn parse_selected(
    text: &str,
    process_env: &BTreeMap<String, String>,
    path: &Path,
) -> (BTreeMap<String, String>, Vec<ConfigDiagnostic>) {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut applied = BTreeMap::new();
    let mut rejected = Vec::new();
    for entry in entries(text, process_env) {
        let Some(key) = entry.key.filter(|key| admitted(key)) else {
            continue;
        };
        if process_env.contains_key(&key) {
            continue;
        }
        match entry.value {
            Ok(value) => {
                applied.insert(key, value);
            }
            Err(problem) => rejected.push((key, problem)),
        }
    }
    rejected.retain(|(key, _)| !applied.contains_key(key));
    let mut invalid = Vec::new();
    let mut diagnostics = Vec::new();
    for (key, problem) in rejected {
        match problem {
            Problem::Malformed => invalid.push(key),
            Problem::Reference { name, braced } => diagnostics.push(ConfigDiagnostic {
                path: path.to_owned(),
                reason: DiagnosticReason::Reference { key, name, braced },
            }),
        }
    }
    if !invalid.is_empty() {
        diagnostics.insert(
            0,
            ConfigDiagnostic {
                path: path.to_owned(),
                reason: DiagnosticReason::InvalidLines(invalid),
            },
        );
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
