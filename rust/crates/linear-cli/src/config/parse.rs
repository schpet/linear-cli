//! TOML parsing for an already selected, size-bounded config or credentials file.

use std::error::Error as StdError;
use std::fmt;
use std::path::PathBuf;

use super::source::{MAX_CONFIG_BYTES, RawConfigFile};

/// A parsed TOML file. Values may include credentials, so it has no
/// value-bearing formatter.
pub struct ConfigTier {
    pub path: PathBuf,
    pub table: toml::Table,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfigParseErrorKind {
    TooLarge,
    InvalidUtf8,
    ByteOrderMark,
    /// The parser's message and 1-based position. The message never quotes
    /// the surrounding source text, which may hold an API key.
    InvalidToml {
        line: usize,
        column: usize,
        message: String,
    },
}

impl fmt::Display for ConfigParseErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => write!(f, "larger than {MAX_CONFIG_BYTES} bytes"),
            Self::InvalidUtf8 => f.write_str("invalid UTF-8"),
            Self::ByteOrderMark => f.write_str("starts with a byte-order mark"),
            Self::InvalidToml {
                line,
                column,
                message,
            } => write!(f, "invalid TOML at line {line}, column {column}: {message}"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ConfigParseError {
    pub path: PathBuf,
    pub kind: ConfigParseErrorKind,
}

impl fmt::Display for ConfigParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.kind)
    }
}

impl StdError for ConfigParseError {}

pub fn parse_config_tier(raw: RawConfigFile) -> Result<ConfigTier, ConfigParseError> {
    let RawConfigFile { path, bytes } = raw;
    let fail = |kind| ConfigParseError {
        path: path.clone(),
        kind,
    };
    if !u64::try_from(bytes.len()).is_ok_and(|len| len <= MAX_CONFIG_BYTES) {
        return Err(fail(ConfigParseErrorKind::TooLarge));
    }
    let text = String::from_utf8(bytes).map_err(|_| fail(ConfigParseErrorKind::InvalidUtf8))?;
    if text.starts_with('\u{feff}') {
        return Err(fail(ConfigParseErrorKind::ByteOrderMark));
    }
    let table = text.parse::<toml::Table>().map_err(|error| {
        let offset = error.span().map_or(0, |span| span.start);
        let (line, column) = line_column(&text, offset);
        fail(ConfigParseErrorKind::InvalidToml {
            line,
            column,
            message: error.message().trim_end().to_owned(),
        })
    })?;
    Ok(ConfigTier { path, table })
}

/// 1-based line and character column of a byte offset.
fn line_column(text: &str, offset: usize) -> (usize, usize) {
    let before = text.get(..offset).unwrap_or(text);
    let line = before.matches('\n').count() + 1;
    let line_start = before.rfind('\n').map_or(0, |index| index + 1);
    let column = before
        .get(line_start..)
        .map_or(0, |rest| rest.chars().count())
        + 1;
    (line, column)
}
