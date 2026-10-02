//! Writing the credentials file. Keyring writes live in `mutation`.
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use std::collections::BTreeMap;

use super::{CredentialFormat, CredentialStore};
use crate::config::ConfigSecret;
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
        // Overwrite in place so an existing file keeps its permissions.
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

/// A TOML basic string. JSON string escapes are a subset of TOML's.
fn toml_string(value: &str) -> String {
    serde_json::Value::from(value).to_string()
}

/// A TOML key: bare when it only uses bare-key characters, quoted otherwise.
fn toml_key(key: &str) -> String {
    if !key.is_empty()
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        key.to_owned()
    } else {
        toml_string(key)
    }
}

/// Renders a credentials file. Metadata files list workspaces whose keys
/// live in the keyring; inline files hold the keys themselves.
pub(crate) fn credentials_text(
    format: CredentialFormat,
    default: Option<&str>,
    workspaces: &[String],
    keys: &BTreeMap<String, ConfigSecret>,
) -> Result<String, AppError> {
    let mut names = workspaces.iter().map(String::as_str).collect::<Vec<_>>();
    names.sort_unstable();
    let mut output = String::new();
    if let Some(default) = default {
        output.push_str(&format!("default = {}\n", toml_string(default)));
    }
    match format {
        CredentialFormat::Metadata => {
            let list = names
                .iter()
                .map(|name| toml_string(name))
                .collect::<Vec<_>>();
            output.push_str(&format!("workspaces = [{}]\n", list.join(", ")));
        }
        CredentialFormat::Inline => {
            for name in names {
                if name == "default" || name == "workspaces" {
                    return Err(AppError::new(
                        AppErrorKind::Validation,
                        format!(
                            "A workspace named \"{name}\" cannot be stored in a plaintext credentials file"
                        ),
                    ));
                }
                let key = keys.get(name).ok_or_else(|| {
                    AppError::new(
                        AppErrorKind::Invariant,
                        format!(
                            "Cannot save inline credentials: API key for workspace \"{name}\" is missing from cache"
                        ),
                    )
                })?;
                output.push_str(&format!(
                    "{} = {}\n",
                    toml_key(name),
                    toml_string(key.expose())
                ));
            }
        }
    }
    Ok(output)
}

/// Plans rewriting the credentials file with a new default workspace.
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
    let (format, workspaces, _, keys) = store.mutation_parts();
    let contents = credentials_text(format, Some(workspace), &workspaces, &keys)?;
    Ok(CredentialWritePlan {
        path: path.to_owned(),
        contents: contents.into_bytes(),
    })
}
