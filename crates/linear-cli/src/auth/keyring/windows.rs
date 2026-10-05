//! Windows Credential Manager through the `keyring` crate: generic
//! credentials with target `linear-cli:<workspace>` and user `<workspace>`,
//! built directly with `WinCredential` so no other attributes are read or
//! written. Secrets are stored as UTF-16, the platform's string encoding.
use keyring::credential::CredentialApi;
use keyring::windows::WinCredential;

use super::{Keyring, LookupFailureCategory, LookupResult};
use crate::config::ConfigSecret;
use crate::error::{Error, Result};

pub struct CredentialManager;

fn credential(workspace: &str) -> WinCredential {
    WinCredential {
        username: workspace.to_owned(),
        target_name: format!("linear-cli:{workspace}"),
        target_alias: String::new(),
        comment: String::new(),
    }
}

fn failure(error: keyring::Error) -> Error {
    match error {
        keyring::Error::BadEncoding(_) => {
            Error::new("System keyring returned an invalid secret encoding")
        }
        other => Error::new(other.to_string()),
    }
}

impl Keyring for CredentialManager {
    fn get(&self, workspace: &str) -> LookupResult {
        match credential(workspace).get_password() {
            Ok(secret) if secret.is_empty() => LookupResult::Miss,
            Ok(secret) => LookupResult::Hit(ConfigSecret::new(secret)),
            Err(keyring::Error::NoEntry) => LookupResult::Miss,
            Err(keyring::Error::NoStorageAccess(_)) => {
                LookupResult::Failed(LookupFailureCategory::Unavailable)
            }
            Err(_) => LookupResult::Failed(LookupFailureCategory::Other),
        }
    }

    /// The `keyring` crate checks the names' lengths and rejects NULs.
    fn set(&self, workspace: &str, secret: &ConfigSecret) -> Result<()> {
        credential(workspace)
            .set_password(secret.expose())
            .map_err(failure)
    }

    fn delete(&self, workspace: &str) -> Result<()> {
        match credential(workspace).delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(failure(error)),
        }
    }
}
