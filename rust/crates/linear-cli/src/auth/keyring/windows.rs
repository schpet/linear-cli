//! Windows Credential Manager access through the `keyring` crate.
use super::windows_spec::{WindowsCredentialSpec, WindowsReadFailure, classify_windows_lookup};
use crate::{
    auth::{LookupFailureCategory, LookupResult, mutation::KeyringBackend},
    config::ConfigSecret,
    error::{Error, Result},
};
use keyring::{credential::CredentialApi, windows::WinCredential};

/// Stores generic credentials with target `linear-cli:<workspace>` and user
/// `<workspace>`, built directly with `WinCredential` so no other attributes
/// are read or written.
pub struct WindowsMutationBackend;

fn failure(error: keyring::Error) -> Error {
    match error {
        keyring::Error::BadEncoding(_) => {
            Error::new("System keyring returned an invalid secret encoding")
        }
        other => Error::new(other.to_string()),
    }
}

impl KeyringBackend for WindowsMutationBackend {
    async fn available(&self) -> bool {
        true
    }
    async fn store(&self, workspace: &str, secret: &ConfigSecret) -> Result<()> {
        credential(workspace)?
            .set_password(secret.expose())
            .map_err(failure)
    }
    async fn delete(&self, workspace: &str) -> Result<()> {
        match credential(workspace)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(failure(error)),
        }
    }
}

pub fn lookup_windows(credential: Result<WinCredential>) -> LookupResult {
    let Ok(credential) = credential else {
        return LookupResult::Failed(LookupFailureCategory::Other);
    };
    classify_windows_lookup(credential.get_secret().map_err(|error| match error {
        keyring::Error::NoEntry => WindowsReadFailure::NoEntry,
        _ => WindowsReadFailure::NativeFailure,
    }))
}

pub fn credential(workspace: &str) -> Result<WinCredential> {
    let spec = WindowsCredentialSpec::new(workspace)?;
    Ok(WinCredential {
        username: spec.username,
        target_name: spec.target_name,
        target_alias: spec.target_alias,
        comment: spec.comment,
    })
}
