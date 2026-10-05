use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use super::source::{ConfigInputs, FileKind, FileSource, MAX_CONFIG_BYTES, absent, lexical};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiagnosticReason {
    Unusable(String),
    /// Lines for these keys could not be parsed.
    InvalidLines(Vec<String>),
    /// The value for `key` contains `reference` (`$NAME` or `${NAME}`).
    /// Values are never expanded, so the key is ignored rather than set to
    /// the literal text.
    Unexpanded {
        key: String,
        reference: String,
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

/// Why a `.env` entry was not used.
enum Problem {
    Malformed,
    /// The value contains `$NAME` or `${NAME}` (the text is kept).
    Reference(String),
}

/// One `KEY=value` entry. `key` is the name the line meant to set, when it
/// names one.
struct Entry {
    key: Option<String>,
    value: Result<String, Problem>,
}

/// Parses `.env` text into entries, in file order.
///
/// Lines are `[export] KEY=value`; blank lines and lines starting with `#`
/// are skipped. Values are literal, never expanded:
/// - `'single quoted'`: taken as is.
/// - `"double quoted"`: backslash escapes (`\n`, `\\`, `\"`, `\'`, `\$`,
///   `\ `, `\#`).
/// - unquoted: the same escapes; ends at a `#` that follows whitespace, and
///   unescaped trailing whitespace is dropped. A quote character must be
///   escaped (quote the whole value instead).
///
/// After a closing quote only whitespace or a `#` comment may follow. Quoted
/// values may span lines; a quote that is never properly closed invalidates
/// only its own line, and the lines after it are parsed normally. A
/// double-quoted or unquoted value containing `$NAME` or `${NAME}` is
/// rejected, so a variable reference is never sent as a literal secret.
fn entries(text: &str) -> Vec<Entry> {
    let text = text.replace("\r\n", "\n");
    let lines: Vec<&str> = text.split('\n').collect();
    let mut entries = Vec::new();
    let mut index = 0;
    while let Some(rest) = lines.get(index..).filter(|rest| !rest.is_empty()) {
        let (entry, used) = entry(rest);
        entries.extend(entry);
        index += used;
    }
    entries
}

/// The entry that starts at the first of `lines`, and how many lines it
/// takes. Blank and comment lines yield no entry.
fn entry(lines: &[&str]) -> (Option<Entry>, usize) {
    let (first, following) = lines.split_first().expect("entry needs a line");
    let line = first.trim_start();
    if line.is_empty() || line.starts_with('#') {
        return (None, 1);
    }
    let malformed = |key: &str| Entry {
        key: Some(key.to_owned()).filter(|key| !key.is_empty()),
        value: Err(Problem::Malformed),
    };
    let Some((head, rest)) = line.split_once('=') else {
        let key = without_export(line).split_whitespace().next().unwrap_or("");
        return (Some(malformed(key)), 1);
    };
    let key = without_export(head.trim());
    if !valid_key(key) {
        return (Some(malformed(key)), 1);
    }
    let rest = rest.trim_start_matches([' ', '\t']);
    let (value, used) = match rest.chars().next() {
        Some(quote @ ('\'' | '"')) => quoted(quote, &rest[1..], following),
        _ => (unquoted(rest), 1),
    };
    let entry = Entry {
        key: Some(key.to_owned()),
        value,
    };
    (Some(entry), used)
}

/// `head` without a leading `export` and the whitespace after it.
fn without_export(head: &str) -> &str {
    match head.strip_prefix("export") {
        Some(rest) if rest.starts_with(char::is_whitespace) => rest.trim_start(),
        Some(_) | None => head,
    }
}

fn valid_key(key: &str) -> bool {
    key.chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.'))
}

/// A quoted value whose text after the opening quote is `rest`, continuing
/// onto `following` lines until the closing quote. A malformed value takes
/// only its own line, so the lines after it are parsed again.
fn quoted(quote: char, rest: &str, following: &[&str]) -> (Result<String, Problem>, usize) {
    let mut raw = rest.to_owned();
    let mut used = 1;
    loop {
        if let Some(end) = closing(quote, &raw) {
            let tail = raw[end + 1..].trim_start_matches([' ', '\t']);
            if !(tail.is_empty() || tail.starts_with('#')) {
                return (Err(Problem::Malformed), 1);
            }
            let content = &raw[..end];
            return match quote {
                '\'' => (Ok(content.to_owned()), used),
                _ => match double_quoted(content) {
                    Ok(value) => (Ok(value), used),
                    Err(Problem::Malformed) => (Err(Problem::Malformed), 1),
                    Err(problem @ Problem::Reference(_)) => (Err(problem), used),
                },
            };
        }
        // A value never runs over a line that sets a key this program reads:
        // that line is far more likely a real entry after an unclosed quote.
        let Some(next) = following
            .get(used - 1)
            .filter(|next| !sets_admitted_key(next))
        else {
            return (Err(Problem::Malformed), 1);
        };
        raw.push('\n');
        raw.push_str(next);
        used += 1;
    }
}

/// The byte index of the quote that closes `raw`. Inside double quotes a
/// backslash escapes the next character.
fn closing(quote: char, raw: &str) -> Option<usize> {
    let mut chars = raw.char_indices();
    while let Some((index, c)) = chars.next() {
        if c == quote {
            return Some(index);
        }
        if c == '\\' && quote == '"' {
            chars.next();
        }
    }
    None
}

fn sets_admitted_key(line: &str) -> bool {
    line.split_once('=')
        .is_some_and(|(head, _)| admitted(without_export(head.trim())))
}

fn double_quoted(content: &str) -> Result<String, Problem> {
    let mut out = String::new();
    let mut chars = content.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.push(escape(chars.next())?),
            '$' => {
                reference(&mut chars)?;
                out.push('$');
            }
            c => out.push(c),
        }
    }
    Ok(out)
}

fn unquoted(text: &str) -> Result<String, Problem> {
    let mut out = String::new();
    // `out` up to its last character that is not unescaped whitespace.
    let mut kept = 0;
    let mut after_space = true;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '#' if after_space => break,
            ' ' | '\t' => {
                out.push(c);
                after_space = true;
                continue;
            }
            '\'' | '"' => return Err(Problem::Malformed),
            '\\' => out.push(escape(chars.next())?),
            '$' => {
                reference(&mut chars)?;
                out.push('$');
            }
            c => out.push(c),
        }
        after_space = false;
        kept = out.len();
    }
    out.truncate(kept);
    Ok(out)
}

/// The character a backslash before `next` stands for.
fn escape(next: Option<char>) -> Result<char, Problem> {
    match next {
        Some('n') => Ok('\n'),
        Some(c @ ('\\' | '\'' | '"' | '$' | ' ' | '#')) => Ok(c),
        _ => Err(Problem::Malformed),
    }
}

/// Rejects the `$NAME` or `${NAME}` that follows a `$`; any other `$` is
/// literal.
fn reference(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Result<(), Problem> {
    let name_char = |c: &char| c.is_ascii_alphanumeric() || *c == '_';
    match chars.peek() {
        Some('{') => {
            let mut text = String::from("$");
            for c in chars.by_ref() {
                text.push(c);
                if c == '}' {
                    break;
                }
            }
            Err(Problem::Reference(text))
        }
        Some(c) if c.is_ascii_alphabetic() || *c == '_' => {
            let name: String = std::iter::from_fn(|| chars.next_if(name_char)).collect();
            Err(Problem::Reference(format!("${name}")))
        }
        Some(_) | None => Ok(()),
    }
}

/// Parses a `.env` file and keeps the admitted keys the process environment
/// does not already set. The last entry for a key wins; when it is invalid
/// the key is ignored with a warning.
fn parse_selected(
    text: &str,
    process_env: &BTreeMap<String, String>,
    path: &Path,
) -> (BTreeMap<String, String>, Vec<ConfigDiagnostic>) {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut last = BTreeMap::new();
    for entry in entries(text) {
        let Some(key) = entry.key.filter(|key| admitted(key)) else {
            continue;
        };
        if !process_env.contains_key(&key) {
            last.insert(key, entry.value);
        }
    }
    let mut applied = BTreeMap::new();
    let mut invalid = Vec::new();
    let mut unexpanded = Vec::new();
    for (key, value) in last {
        match value {
            Ok(value) => {
                applied.insert(key, value);
            }
            Err(Problem::Malformed) => invalid.push(key),
            Err(Problem::Reference(reference)) => unexpanded.push(ConfigDiagnostic {
                path: path.to_owned(),
                reason: DiagnosticReason::Unexpanded { key, reference },
            }),
        }
    }
    let mut diagnostics = Vec::new();
    if !invalid.is_empty() {
        diagnostics.push(ConfigDiagnostic {
            path: path.to_owned(),
            reason: DiagnosticReason::InvalidLines(invalid),
        });
    }
    diagnostics.extend(unexpanded);
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

#[cfg(test)]
mod tests;
