//! Safe direct Windows credential adapter; no extra attribute read before writes.
use super::windows_spec::{WindowsCredentialSpec, WindowsReadFailure, classify_windows_lookup};
use crate::{
    auth::{
        LookupFailureCategory, LookupResult,
        mutation::{CredentialMutationBackend, MutationFailure},
    },
    config::ConfigSecret,
    error::{AppError, AppErrorKind},
};
use keyring::{credential::CredentialApi, windows::WinCredential};

/// Factory must construct DIRECT WinCredential (not generic Entry::new), exact
/// target linear-cli:WS/userWS/empty comment+alias. The approved pinned library
/// owns GENERIC/Enterprise3 and UTF16LE/no terminator/one CredWrite/zero attrs.
pub struct WindowsMutationBackend {
    pub overlay: crate::config::ChildEnvOverlay,
}
fn failure(error: keyring::Error) -> MutationFailure {
    match error {
        keyring::Error::BadEncoding(_) => MutationFailure::Typed(AppError::new(
            AppErrorKind::Validation,
            "System keyring returned an invalid secret encoding",
        )),
        other => MutationFailure::Ordinary(other.to_string()),
    }
}
impl CredentialMutationBackend for WindowsMutationBackend {
    async fn available(&self) -> bool {
        true
    } // Construction only; no read/write.
    async fn store(&self, workspace: &str, secret: &ConfigSecret) -> Result<(), MutationFailure> {
        let credential = credential(workspace)?;
        // Safe dependency performs one CredWrite; no pre-read/update_attributes.
        credential.set_password(secret.expose()).map_err(failure)
    }
    async fn delete(&self, workspace: &str) -> Result<(), MutationFailure> {
        let credential = credential(workspace)?;
        match credential.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(failure(error)),
        }
    }
}
pub fn lookup_windows(credential: Result<WinCredential, MutationFailure>) -> LookupResult {
    let credential = match credential {
        Ok(value) => value,
        Err(_) => return LookupResult::Failed(LookupFailureCategory::Other),
    };
    classify_windows_lookup(credential.get_secret().map_err(|error| match error {
        keyring::Error::NoEntry => WindowsReadFailure::NoEntry,
        _ => WindowsReadFailure::NativeFailure,
    }))
}

pub fn credential(workspace: &str) -> Result<WinCredential, MutationFailure> {
    let spec = WindowsCredentialSpec::new(workspace).map_err(MutationFailure::Typed)?;
    Ok(WinCredential {
        username: spec.username,
        target_name: spec.target_name,
        target_alias: spec.target_alias,
        comment: spec.comment,
    })
}
