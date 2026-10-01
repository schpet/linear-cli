use crate::{
    auth::{
        CredentialFormat,
        mutation::{
            CredentialMutationBackend, CredentialMutationFileWriter, CredentialMutationState,
            MutationFailure,
        },
    },
    error::{AppError, AppErrorKind},
};
use std::path::Path;
pub const CONTEXT: &str = "Failed to migrate credentials";
pub async fn run(
    state: &mut CredentialMutationState,
    path: Option<&Path>,
    backend: &impl CredentialMutationBackend,
    writer: &impl CredentialMutationFileWriter,
) -> Result<Vec<u8>, MutationFailure> {
    if state.format() != CredentialFormat::Inline {
        return Ok(b"Credentials are already using the system keyring.\n".to_vec());
    }
    if !backend.available().await {
        return Err(MutationFailure::Typed(AppError::new(AppErrorKind::Validation,
            "No system keyring found. Cannot migrate credentials.")
            .with_suggestion("Install libsecret (e.g. `apt install libsecret-tools` or `pacman -S libsecret`), or set `LINEAR_API_KEY` instead.")));
    }
    let migrated = state.migrate(path, backend, writer).await?;
    if migrated.is_empty() {
        return Ok(b"No credentials to migrate.\n".to_vec());
    }
    let mut output = format!(
        "Migrated {} workspace(s) to system keyring:\n",
        migrated.len()
    );
    for name in migrated {
        output.push_str(&format!("  {name}\n"));
    }
    Ok(output.into_bytes())
}
