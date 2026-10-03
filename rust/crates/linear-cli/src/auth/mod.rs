//! Pure credential formats and selection. Backends and process inputs live elsewhere.
pub mod file;
mod format;
pub mod header;
pub mod keyring;
mod path;
mod resolve;

pub use format::{
    CredentialFormat, CredentialFormatErrorKind, CredentialManifest, CredentialStore,
    CredentialWarning, LookupFailureCategory, LookupResult, parse_credentials,
};
pub use path::credentials_path;
pub use resolve::{ApiKeyInput, CredentialSelection, CredentialSelectionInputs, resolve};
pub mod mutation;

#[cfg(test)]
pub(crate) mod test_support;
