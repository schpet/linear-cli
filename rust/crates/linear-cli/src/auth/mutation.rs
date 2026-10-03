//! Changing stored credentials: keyring writes plus rewriting the credentials
//! file in one of its two formats.
use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::future::Future;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::auth::{CredentialFormat, CredentialStore};
use crate::config::ConfigSecret;
use crate::error::{Error, Result, ResultExt};

/// Stores and deletes API keys in the system keyring.
pub trait KeyringBackend {
    /// Whether the keyring can be used at all.
    fn available(&self) -> impl Future<Output = bool>;
    fn store(&self, workspace: &str, secret: &ConfigSecret) -> impl Future<Output = Result<()>>;
    /// Deleting a workspace with no entry succeeds.
    fn delete(&self, workspace: &str) -> impl Future<Output = Result<()>>;
}

/// The credentials file, edited in memory and written back whole after each
/// change.
pub struct Credentials {
    path: PathBuf,
    format: CredentialFormat,
    workspaces: Vec<String>,
    default: Option<String>,
    /// Every workspace's key in a plaintext file; empty when the keys live in
    /// the keyring.
    keys: BTreeMap<String, ConfigSecret>,
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("path", &self.path)
            .field("format", &self.format)
            .field("workspaces", &self.workspaces)
            .field("default", &self.default)
            .finish_non_exhaustive()
    }
}

impl Credentials {
    /// The credentials in `store`, which was read from `path`. No keyring
    /// entry is read.
    pub fn new(store: &CredentialStore, path: &Path) -> Self {
        let (format, workspaces, default, keys) = store.file_parts();
        Self {
            path: path.to_owned(),
            format,
            workspaces,
            default,
            keys,
        }
    }

    pub fn format(&self) -> CredentialFormat {
        self.format
    }

    pub fn workspaces(&self) -> &[String] {
        &self.workspaces
    }

    /// The default workspace; with a single workspace, that one.
    pub fn default(&self) -> Option<&str> {
        self.default
            .as_deref()
            .or(match self.workspaces.as_slice() {
                [only] => Some(only.as_str()),
                _ => None,
            })
    }

    pub fn has_workspace(&self, workspace: &str) -> bool {
        self.workspaces.iter().any(|name| name == workspace)
    }

    /// Whether `add` keeps the key in the file: with `plaintext`, or when the
    /// file already holds plaintext keys.
    pub fn stores_plaintext(&self, plaintext: bool) -> bool {
        plaintext || (self.format == CredentialFormat::Inline && !self.workspaces.is_empty())
    }

    /// Adds or replaces `workspace`'s key. A plaintext key added to a
    /// keyring-backed file turns it into a plaintext file: the other keys are
    /// read from `store`'s keyring and written to the file.
    pub async fn add(
        &mut self,
        workspace: &str,
        secret: ConfigSecret,
        plaintext: bool,
        store: &CredentialStore,
        keyring: &impl KeyringBackend,
    ) -> Result<()> {
        if self.stores_plaintext(plaintext) {
            if self.format == CredentialFormat::Metadata {
                for name in self.workspaces.iter().filter(|name| *name != workspace) {
                    let key = store.key(name).ok_or_else(|| {
                        Error::new(format!(
                            "Could not read the API key for workspace \"{name}\" from the system keyring"
                        ))
                        .with_hint(format!(
                            "Log in without --plaintext, or run `linear auth logout {name}` first."
                        ))
                    })?;
                    self.keys.insert(name.clone(), key.clone());
                }
                self.format = CredentialFormat::Inline;
            }
            self.keys.insert(workspace.to_owned(), secret);
        } else {
            keyring.store(workspace, &secret).await.context(format!(
                "Failed to store API key in system keyring for workspace \"{workspace}\""
            ))?;
        }
        if !self.has_workspace(workspace) {
            self.workspaces.push(workspace.to_owned());
        }
        if self.workspaces.len() == 1 {
            self.default = Some(workspace.to_owned());
        }
        self.save()
    }

    /// Removes `workspace`, deleting its keyring entry first. The next
    /// workspace becomes the default when the default is removed.
    pub async fn remove(&mut self, workspace: &str, keyring: &impl KeyringBackend) -> Result<()> {
        if self.format == CredentialFormat::Metadata {
            keyring.delete(workspace).await.context(format!(
                "Failed to remove API key from system keyring for workspace \"{workspace}\""
            ))?;
        }
        self.keys.remove(workspace);
        self.workspaces.retain(|name| name != workspace);
        if self.default.as_deref() == Some(workspace) {
            self.default = self.workspaces.first().cloned();
        }
        self.save()
    }

    /// Moves every plaintext key to the keyring and returns the migrated
    /// workspaces. If one store fails, the entries this call wrote are
    /// deleted again and the file is left as it was.
    pub async fn migrate(&mut self, keyring: &impl KeyringBackend) -> Result<Vec<String>> {
        if self.format != CredentialFormat::Inline {
            return Ok(Vec::new());
        }
        let mut migrated: Vec<String> = Vec::new();
        for name in &self.workspaces {
            let key = self
                .keys
                .get(name)
                .expect("a plaintext credentials file has a key for every workspace");
            if let Err(error) = keyring.store(name, key).await {
                let mut error = error.context(format!(
                    "Failed to store API key in system keyring for workspace \"{name}\""
                ));
                for written in &migrated {
                    if let Err(cleanup) = keyring.delete(written).await {
                        error.push_message(&format!(
                            "; could not remove the new keyring entry for \"{written}\": {cleanup}"
                        ));
                    }
                }
                return Err(error);
            }
            migrated.push(name.clone());
        }
        self.format = CredentialFormat::Metadata;
        self.keys.clear();
        // If saving fails, the keys already stored in the keyring stay there.
        self.save()?;
        Ok(migrated)
    }

    /// Makes `workspace`, which must be stored, the default.
    pub fn set_default(&mut self, workspace: &str) -> Result<()> {
        assert!(
            self.has_workspace(workspace),
            "only a stored workspace can be the default"
        );
        self.default = Some(workspace.to_owned());
        self.save()
    }

    fn save(&self) -> Result<()> {
        let contents = self.render()?;
        write_private(&self.path, contents.as_bytes()).map_err(|error| {
            Error::new(format!(
                "Failed to write credentials file at {}: {error}",
                self.path.display()
            ))
            .with_source(error)
        })
    }

    /// The file's TOML: `default` first, then either the workspace list or
    /// one `<workspace> = "<key>"` entry per workspace, sorted by name.
    fn render(&self) -> Result<String> {
        let mut names: Vec<&str> = self.workspaces.iter().map(String::as_str).collect();
        names.sort_unstable();
        let rendered = match self.format {
            CredentialFormat::Metadata => toml::to_string(&KeyringFile {
                default: self.default.as_deref(),
                workspaces: names,
            }),
            CredentialFormat::Inline => {
                if let Some(reserved) = names
                    .iter()
                    .find(|name| matches!(**name, "default" | "workspaces"))
                {
                    return Err(Error::new(format!(
                        "A workspace named \"{reserved}\" cannot be stored in a plaintext credentials file"
                    )));
                }
                let keys = names
                    .into_iter()
                    .map(|name| {
                        let key = self
                            .keys
                            .get(name)
                            .expect("a plaintext credentials file has a key for every workspace");
                        (name, key.expose())
                    })
                    .collect();
                toml::to_string(&PlaintextFile {
                    default: self.default.as_deref(),
                    keys,
                })
            }
        };
        Ok(rendered.expect("credentials always serialize as TOML"))
    }
}

#[derive(Serialize)]
struct KeyringFile<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    default: Option<&'a str>,
    workspaces: Vec<&'a str>,
}

#[derive(Serialize)]
struct PlaintextFile<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    default: Option<&'a str>,
    #[serde(flatten)]
    keys: BTreeMap<&'a str, &'a str>,
}

/// Writes `contents` to `path`, creating its directory. A new file is
/// readable only by its owner; an existing file keeps its permissions.
fn write_private(path: &Path, contents: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(contents)
}

#[cfg(test)]
mod tests;
