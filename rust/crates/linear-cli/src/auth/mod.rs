//! Stored credentials: the credentials file and its two formats, the system
//! keyring that holds keys for the keyring format, choosing which key a
//! command uses, and changing what is stored.
pub mod file;
mod format;
pub mod keyring;
pub mod mutation;
mod path;
mod resolve;

pub use format::{
    CredentialFormat, CredentialFormatErrorKind, CredentialManifest, CredentialStore,
    CredentialWarning, parse_credentials,
};
pub use keyring::LookupFailureCategory;
pub use path::credentials_path;
pub use resolve::{
    ApiKeyInput, CredentialSelection, CredentialSelectionInputs, WorkspaceChoice, resolve,
};

#[cfg(test)]
pub(crate) mod test_support;
