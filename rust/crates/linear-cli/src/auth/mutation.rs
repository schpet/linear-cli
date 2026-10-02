//! Source-ordered credential transitions and explicit backend/file effects.
use crate::{
    auth::{CredentialFormat, CredentialStore},
    config::ConfigSecret,
    error::{AppError, AppErrorKind},
};
use std::{collections::BTreeMap, future::Future, io, path::Path};

pub enum MutationFailure {
    /// Source ordinary Error.message: the login inner catch must inspect all phases.
    Ordinary(String),
    /// Source custom errors and named native boundaries bypass that catch.
    Typed(AppError),
}
impl MutationFailure {
    pub fn outer(self) -> AppError {
        match self {
            Self::Ordinary(message) => AppError::new(AppErrorKind::IoProcess, message),
            Self::Typed(error) => error,
        }
    }
    pub fn login(self) -> AppError {
        match self {
            Self::Typed(error) => error,
            Self::Ordinary(message) if message.contains("401") => {
                AppError::new(AppErrorKind::Auth, "Invalid API key")
                    .with_suggestion("Check that your API key is correct and not expired.")
            }
            Self::Ordinary(message) => AppError::new(
                AppErrorKind::GraphQl,
                format!("Failed to authenticate: {message}"),
            ),
        }
    }
}
pub trait CredentialMutationBackend {
    fn available(&self) -> impl Future<Output = bool>;
    fn store(
        &self,
        workspace: &str,
        secret: &ConfigSecret,
    ) -> impl Future<Output = Result<(), MutationFailure>>;
    fn delete(&self, workspace: &str) -> impl Future<Output = Result<(), MutationFailure>>;
}
/// Separate directory preparation is observable before a missing cached key.
/// Existing auth-default's file-writer/public contract remains unchanged.
pub trait CredentialMutationFileWriter {
    fn prepare_directory(&self, path: &Path) -> io::Result<()>;
    fn write_file(&self, path: &Path, contents: &[u8]) -> io::Result<()>;
}
pub struct RealCredentialMutationFileWriter;
impl CredentialMutationFileWriter for RealCredentialMutationFileWriter {
    fn prepare_directory(&self, path: &Path) -> io::Result<()> {
        let parent = path.parent().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "credentials path has no parent",
            )
        })?;
        std::fs::create_dir_all(parent)
    }
    fn write_file(&self, path: &Path, contents: &[u8]) -> io::Result<()> {
        std::fs::write(path, contents)
    }
}

pub struct CredentialMutationState {
    format: CredentialFormat,
    workspaces: Vec<String>,
    default: Option<String>,
    keys: BTreeMap<String, ConfigSecret>,
}
impl std::fmt::Debug for CredentialMutationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CredentialMutationState")
            .field("format", &self.format)
            .field("workspaces", &self.workspaces)
            .field("default", &self.default)
            .field("keys", &"<redacted>")
            .finish()
    }
}
impl CredentialMutationState {
    pub fn from_store(store: &CredentialStore) -> Self {
        let (format, workspaces, default, keys) = store.mutation_parts();
        Self {
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
    pub fn default(&self) -> Option<&str> {
        self.default.as_deref()
    }
    pub fn has_workspace(&self, workspace: &str) -> bool {
        self.workspaces.iter().any(|name| name == workspace)
    }
    fn remember(&mut self, workspace: &str, secret: ConfigSecret) {
        self.keys.insert(workspace.to_owned(), secret);
        if !self.has_workspace(workspace) {
            self.workspaces.push(workspace.to_owned());
            if self.workspaces.len() == 1 {
                self.default = Some(workspace.to_owned());
            }
        }
    }
    fn path<'a>(&self, path: Option<&'a Path>) -> Result<&'a Path, MutationFailure> {
        path.ok_or_else(|| {
            MutationFailure::Ordinary("Could not determine credentials path".to_owned())
        })
    }
    fn prepare_path<'a>(
        &self,
        path: Option<&'a Path>,
        writer: &impl CredentialMutationFileWriter,
    ) -> Result<&'a Path, MutationFailure> {
        let path = self.path(path)?;
        writer
            .prepare_directory(path)
            .map_err(|error| MutationFailure::Ordinary(error.to_string()))?;
        Ok(path)
    }
    fn save(
        &self,
        inline: bool,
        path: Option<&Path>,
        writer: &impl CredentialMutationFileWriter,
    ) -> Result<(), MutationFailure> {
        let path = self.prepare_path(path, writer)?;
        let format = if inline {
            CredentialFormat::Inline
        } else {
            CredentialFormat::Metadata
        };
        let contents = crate::auth::write::credentials_text(
            format,
            self.default(),
            &self.workspaces,
            &self.keys,
        )
        .map_err(MutationFailure::Typed)?;
        writer
            .write_file(path, contents.as_bytes())
            .map_err(|error| MutationFailure::Ordinary(error.to_string()))
    }
    pub async fn add(
        &mut self,
        workspace: &str,
        secret: ConfigSecret,
        plaintext: Option<bool>,
        path: Option<&Path>,
        backend: &impl CredentialMutationBackend,
        writer: &impl CredentialMutationFileWriter,
    ) -> Result<(), MutationFailure> {
        let use_inline = plaintext.unwrap_or(self.format == CredentialFormat::Inline);
        if plaintext == Some(false) && self.format == CredentialFormat::Inline {
            // Internal explicit-false transition: CLI bool=false is NEVER mapped here.
            self.remember(workspace, secret);
            for name in &self.workspaces {
                let Some(key) = self.keys.get(name) else {
                    continue;
                };
                backend
                    .store(name, key)
                    .await
                    .map_err(|failure| store_failure(name, failure))?;
            }
            self.format = CredentialFormat::Metadata;
            return self.save(false, path, writer);
        }
        if !use_inline {
            backend
                .store(workspace, &secret)
                .await
                .map_err(|failure| store_failure(workspace, failure))?;
        }
        self.remember(workspace, secret);
        // Deliberately do NOT mutate original format for metadata -> plaintext.
        self.save(use_inline, path, writer)
    }
    pub async fn remove(
        &mut self,
        workspace: &str,
        path: Option<&Path>,
        backend: &impl CredentialMutationBackend,
        writer: &impl CredentialMutationFileWriter,
    ) -> Result<(), MutationFailure> {
        if self.format == CredentialFormat::Metadata {
            backend.delete(workspace).await.map_err(|failure| match failure {
                MutationFailure::Typed(error) => MutationFailure::Typed(error),
                MutationFailure::Ordinary(message) => MutationFailure::Ordinary(format!(
                    "Failed to remove API key from system keyring for workspace \"{workspace}\": {message}")),
            })?;
        }
        self.keys.remove(workspace);
        self.workspaces.retain(|name| name != workspace);
        if self.default() == Some(workspace) {
            self.default = self.workspaces.first().cloned();
        }
        self.save(self.format == CredentialFormat::Inline, path, writer)
    }
    pub async fn migrate(
        &mut self,
        path: Option<&Path>,
        backend: &impl CredentialMutationBackend,
        writer: &impl CredentialMutationFileWriter,
    ) -> Result<Vec<String>, MutationFailure> {
        if self.format != CredentialFormat::Inline {
            return Ok(Vec::new());
        }
        let mut migrated = Vec::<String>::new();
        for name in &self.workspaces {
            let Some(key) = self.keys.get(name) else {
                continue;
            };
            if let Err(failure) = backend.store(name, key).await {
                for written in &migrated {
                    // Exact source best-effort rollback: forward order, every entry,
                    // including preexisting overwritten entries; never restores old keys.
                    let _cleanup_result = backend.delete(written).await;
                }
                return Err(match store_failure(name, failure) {
                    MutationFailure::Typed(error) => MutationFailure::Typed(error),
                    MutationFailure::Ordinary(message) => MutationFailure::Ordinary(format!(
                        "{message}. Rolled back {} already-written entries.",
                        migrated.len()
                    )),
                });
            }
            migrated.push(name.clone());
        }
        self.format = CredentialFormat::Metadata;
        // No default membership check; stale inline default survives migration.
        // Save failure retains all completed backend stores without rollback.
        self.save(false, path, writer)?;
        Ok(migrated)
    }
}
fn store_failure(workspace: &str, failure: MutationFailure) -> MutationFailure {
    match failure {
        MutationFailure::Typed(error) => MutationFailure::Typed(error),
        MutationFailure::Ordinary(message) => MutationFailure::Ordinary(format!(
            "Failed to store API key in system keyring for workspace \"{workspace}\": {message}"
        )),
    }
}
