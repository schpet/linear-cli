//! Pure credential formats and selection. Backends and process inputs live elsewhere.
mod format;
pub mod header;
mod path;
mod resolve;

pub use format::{
    CredentialFormat, CredentialFormatError, CredentialFormatErrorKind, CredentialInvariantError,
    CredentialManifest, CredentialStore, CredentialWarning, LookupFailureCategory, LookupReply,
    LookupResult, hydrate, parse_credentials,
};
pub use path::credentials_path;
pub use resolve::{
    ApiKeyInput, CredentialSelection, CredentialSelectionInputs, CredentialSource, resolve,
};
