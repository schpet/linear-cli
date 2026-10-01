//! Explicit credential-file plans; no ambient path discovery or keyring writes.
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::format::js_index;
use super::{CredentialFormat, CredentialStore};
use crate::error::{AppError, AppErrorKind};

pub trait CredentialFileWriter {
    fn write_credentials(&self, path: &Path, contents: &[u8]) -> io::Result<()>;
}

pub struct RealCredentialFileWriter;
impl CredentialFileWriter for RealCredentialFileWriter {
    fn write_credentials(&self, path: &Path, contents: &[u8]) -> io::Result<()> {
        let parent = path.parent().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "credentials path has no parent",
            )
        })?;
        fs::create_dir_all(parent)?;
        // Match direct source truncation/write, preserving an existing file's mode.
        // An arbitrary write failure may leave a partial file; no rollback is promised.
        fs::write(path, contents)
    }
}

pub struct CredentialWritePlan {
    path: PathBuf,
    contents: Vec<u8>,
}
impl fmt::Debug for CredentialWritePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CredentialWritePlan")
            .field("path", &self.path)
            .field("contents", &"<redacted>")
            .finish()
    }
}
impl CredentialWritePlan {
    pub fn save(&self, writer: &impl CredentialFileWriter) -> Result<(), AppError> {
        writer
            .write_credentials(&self.path, &self.contents)
            .map_err(|error| {
                AppError::new(
                    AppErrorKind::IoProcess,
                    format!(
                        "Failed to write credentials file at {}: {error}",
                        self.path.display()
                    ),
                )
                .with_source(error)
            })
    }
}

fn json_string(value: &str) -> Result<String, AppError> {
    serde_json::to_string(value).map_err(|error| {
        AppError::new(
            AppErrorKind::Invariant,
            "could not serialize credential string",
        )
        .with_source(error)
    })
}
fn key_text(key: &str) -> Result<String, AppError> {
    if !key.is_empty()
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        Ok(key.to_owned())
    } else {
        json_string(key)
    }
}
fn line(output: &mut String, key: &str, value: &str) -> Result<(), AppError> {
    output.push_str(&key_text(key)?);
    output.push_str(" = ");
    output.push_str(&json_string(value)?);
    output.push('\n');
    Ok(())
}

/// Prepare the entire exact file before opening it. The store is already typed/loaded.
pub fn prepare_default_write(
    store: &CredentialStore,
    workspace: &str,
    path: Option<&Path>,
) -> Result<CredentialWritePlan, AppError> {
    if !store.workspaces().iter().any(|name| name == workspace) {
        return Err(AppError::new(
            AppErrorKind::Invariant,
            "default write requires a stored workspace",
        ));
    }
    let path = path.ok_or_else(|| {
        AppError::new(
            AppErrorKind::IoProcess,
            "Could not determine credentials path",
        )
    })?;
    let mut names = store
        .workspaces()
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    names.sort_by(|left, right| left.encode_utf16().cmp(right.encode_utf16()));
    let mut output = String::new();
    match store.format() {
        CredentialFormat::Metadata => {
            line(&mut output, "default", workspace)?;
            let array = serde_json::to_string(&names).map_err(|error| {
                AppError::new(
                    AppErrorKind::Invariant,
                    "could not serialize credential workspaces",
                )
                .with_source(error)
            })?;
            output.push_str("workspaces = ");
            output.push_str(&array);
            output.push('\n');
        }
        CredentialFormat::Inline => {
            let (mut numeric, ordinary): (Vec<_>, Vec<_>) =
                names.into_iter().partition(|name| js_index(name).is_some());
            numeric.sort_by_key(|name| js_index(name));
            for name in numeric {
                let secret = store.key(name).ok_or_else(|| missing_cache(name))?;
                line(&mut output, name, secret.expose())?;
            }
            line(&mut output, "default", workspace)?;
            for name in ordinary {
                // Shared parsing already drops this property. Guard an impossible store
                // rather than silently serializing a property the source never loaded.
                if name == "__proto__" {
                    return Err(AppError::new(
                        AppErrorKind::Invariant,
                        "inline cache contains a source-omitted property",
                    ));
                }
                let secret = store.key(name).ok_or_else(|| missing_cache(name))?;
                line(&mut output, name, secret.expose())?;
            }
        }
    }
    Ok(CredentialWritePlan {
        path: path.to_owned(),
        contents: output.into_bytes(),
    })
}
fn missing_cache(workspace: &str) -> AppError {
    AppError::new(
        AppErrorKind::Invariant,
        format!(
            "Cannot save inline credentials: API key for workspace \"{workspace}\" is missing from cache"
        ),
    )
}
