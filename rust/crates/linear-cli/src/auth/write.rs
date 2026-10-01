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

// Append to existing auth/write.rs: reuse its key/string/line formatter.
// Old prepare_default_write/member invariant and old callers remain unchanged.
pub(crate) fn mutation_metadata_text(
    default: Option<&str>,
    workspaces: &[String],
) -> Result<String, crate::auth::mutation::MutationFailure> {
    use crate::auth::mutation::MutationFailure;
    let mut output = String::new();
    if let Some(default) = default {
        line(&mut output, "default", default).map_err(MutationFailure::Typed)?;
    }
    let mut names = workspaces.to_vec();
    names.sort_by(|left, right| left.encode_utf16().cmp(right.encode_utf16()));
    output.push_str("workspaces = ");
    output.push_str(
        &serde_json::to_string(&names)
            .map_err(|error| MutationFailure::Ordinary(error.to_string()))?,
    );
    output.push('\n');
    Ok(output)
}
pub(crate) fn mutation_inline_text(
    default: Option<&str>,
    workspaces: &[String],
    keys: &std::collections::BTreeMap<String, crate::config::ConfigSecret>,
) -> Result<String, crate::auth::mutation::MutationFailure> {
    use crate::auth::mutation::MutationFailure;
    let mut entries = Vec::<(String, String)>::new();
    if let Some(default) = default {
        entries.push(("default".to_owned(), default.to_owned()));
    }
    let mut names = workspaces.to_vec();
    names.sort_by(|left, right| left.encode_utf16().cmp(right.encode_utf16()));
    for name in names {
        let key = keys.get(&name).ok_or_else(|| MutationFailure::Ordinary(format!(
            "Cannot save inline credentials: API key for workspace \"{name}\" is missing from cache")))?;
        // JS ordinary-object setter does not add __proto__; lookup still precedes assignment.
        if name == "__proto__" {
            continue;
        }
        if let Some((_, value)) = entries.iter_mut().find(|(key, _)| key == &name) {
            *value = key.expose().to_owned();
        } else {
            entries.push((name, key.expose().to_owned()));
        }
    }
    let (mut numeric, ordinary): (Vec<_>, Vec<_>) = entries
        .into_iter()
        .partition(|(name, _)| super::format::js_index(name).is_some());
    numeric.sort_by_key(|(name, _)| super::format::js_index(name));
    let mut output = String::new();
    for (name, value) in numeric.into_iter().chain(ordinary) {
        line(&mut output, &name, &value).map_err(MutationFailure::Typed)?;
    }
    Ok(output)
}
