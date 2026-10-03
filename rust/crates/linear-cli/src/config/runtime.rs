//! A snapshot of the process environment variables this program reads.
use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

use super::source::{ConfigInputs, OsFamily};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProcessEnvError {
    InvalidName,
    InvalidValue { name: String },
    DuplicateName { name: String },
}

impl fmt::Display for ProcessEnvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName => f.write_str("a relevant environment variable name is not UTF-8"),
            Self::InvalidValue { name } => write!(f, "{name} is not valid UTF-8"),
            Self::DuplicateName { name } => write!(f, "duplicate environment variable {name}"),
        }
    }
}

impl std::error::Error for ProcessEnvError {}

#[derive(Clone, Eq, PartialEq)]
pub struct ProcessEnvSnapshot {
    pub inputs: ConfigInputs,
    /// Kept losslessly so invalid UTF-8 only fails if a pager is actually used.
    pub pager: Option<OsString>,
}

impl ProcessEnvSnapshot {
    pub fn capture(cwd: PathBuf, os: OsFamily) -> Result<Self, ProcessEnvError> {
        Self::from_vars_os(cwd, os, env::vars_os())
    }

    pub fn from_vars_os(
        cwd: PathBuf,
        os: OsFamily,
        variables: impl IntoIterator<Item = (OsString, OsString)>,
    ) -> Result<Self, ProcessEnvError> {
        let mut process_env = BTreeMap::new();
        let mut pager = None;
        for (raw_name, raw_value) in variables {
            let Some(name) = raw_name.to_str() else {
                if relevant(&raw_name.to_string_lossy(), os) {
                    return Err(ProcessEnvError::InvalidName);
                }
                continue;
            };
            let key = if os == OsFamily::Windows {
                name.to_ascii_uppercase()
            } else {
                name.to_owned()
            };
            if key == "PAGER" {
                if pager.replace(raw_value).is_some() {
                    return Err(ProcessEnvError::DuplicateName { name: key });
                }
                continue;
            }
            if !relevant(&key, os) {
                continue;
            }
            let Some(value) = raw_value.to_str() else {
                return Err(ProcessEnvError::InvalidValue {
                    name: name.to_owned(),
                });
            };
            if process_env.insert(key, value.to_owned()).is_some() {
                return Err(ProcessEnvError::DuplicateName {
                    name: name.to_owned(),
                });
            }
        }
        Ok(Self {
            pager,
            inputs: ConfigInputs {
                cwd,
                os,
                process_env,
            },
        })
    }
}

fn relevant(name: &str, os: OsFamily) -> bool {
    let key = if os == OsFamily::Windows {
        name.to_ascii_uppercase()
    } else {
        name.to_owned()
    };
    key.starts_with("LINEAR_")
        || key.starts_with("GH_")
        || key.starts_with("GITHUB_")
        || matches!(
            key.as_str(),
            "NO_COLOR"
                | "TMPDIR"
                | "TMP"
                | "TEMP"
                | "HOME"
                | "XDG_CONFIG_HOME"
                | "APPDATA"
                | "SSL_CERT_FILE"
                | "DENO_CERT"
        )
}

#[cfg(test)]
mod tests;
