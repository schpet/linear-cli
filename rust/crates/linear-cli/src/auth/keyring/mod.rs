//! Read-only keyring abstraction. Startup injects fakes; commands never store here.
#[cfg(target_os = "linux")]
mod linux;

use crate::auth::{LookupFailureCategory, LookupResult};

#[cfg(target_os = "linux")]
pub use linux::{LinuxKeyringReader, LinuxLookupFailure};

pub trait KeyringReader: Sync {
    fn lookup(&self, workspace: &str) -> LookupResult;
}

pub struct UnsupportedKeyringReader;

impl KeyringReader for UnsupportedKeyringReader {
    fn lookup(&self, _workspace: &str) -> LookupResult {
        LookupResult::Failed(LookupFailureCategory::UnsupportedPlatform)
    }
}
