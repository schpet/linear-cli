//! Strict, owned TOML parsing for an already selected bounded config file.

use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use super::source::{MAX_CONFIG_BYTES, RawConfigFile};

pub const MAX_CONFIG_DEPTH: usize = 64;

/// A selected config file with all TOML values retained in source order.
///
/// Values may include credentials. Deliberately has no value-bearing formatter.
pub struct ConfigTier {
    pub path: PathBuf,
    pub entries: Vec<(String, ConfigValue)>,
}

/// Owned value tree; parser-specific types never cross the config boundary.
///
/// Deliberately has no value-bearing Debug or Display implementation.
pub enum ConfigValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Datetime(String),
    Array(Vec<ConfigValue>),
    Table(Vec<(String, ConfigValue)>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigParseErrorKind {
    TooLarge,
    InvalidUtf8,
    ByteOrderMark,
    InvalidToml,
    TooDeep,
}

/// A path and fixed category only. No source text or parser error is retained.
pub struct ConfigParseError {
    pub path: PathBuf,
    pub kind: ConfigParseErrorKind,
}

impl fmt::Debug for ConfigParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConfigParseError")
            .field("path", &self.path)
            .field("kind", &self.kind)
            .finish()
    }
}

impl fmt::Display for ConfigParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "config parse error {:?} at {}",
            self.kind,
            self.path.display()
        )
    }
}

impl Error for ConfigParseError {}

fn convert(value: toml::Value, depth: usize) -> Result<ConfigValue, ConfigParseErrorKind> {
    if depth > MAX_CONFIG_DEPTH {
        return Err(ConfigParseErrorKind::TooDeep);
    }
    match value {
        toml::Value::String(value) => Ok(ConfigValue::String(value)),
        toml::Value::Integer(value) => Ok(ConfigValue::Integer(value)),
        toml::Value::Float(value) => Ok(ConfigValue::Float(value)),
        toml::Value::Boolean(value) => Ok(ConfigValue::Boolean(value)),
        toml::Value::Datetime(value) => Ok(ConfigValue::Datetime(value.to_string())),
        toml::Value::Array(values) => values
            .into_iter()
            .map(|value| convert(value, depth + 1))
            .collect::<Result<Vec<_>, _>>()
            .map(ConfigValue::Array),
        toml::Value::Table(entries) => entries
            .into_iter()
            .map(|(key, value)| Ok((key, convert(value, depth + 1)?)))
            .collect::<Result<Vec<_>, _>>()
            .map(ConfigValue::Table),
    }
}

/// Parse only an A1 `ReadCandidate::Contents` payload; discovery remains separate.
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
    // The TOML error owns source snippets; drop it immediately and never chain it.
    let table: toml::Table = text
        .parse()
        .map_err(|_| fail(ConfigParseErrorKind::InvalidToml))?;
    let entries = table
        .into_iter()
        .map(|(key, value)| Ok((key, convert(value, 1).map_err(&fail)?)))
        .collect::<Result<Vec<_>, ConfigParseError>>()?;
    Ok(ConfigTier { path, entries })
}
