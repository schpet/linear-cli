//! Keyring readers used at startup and backends used by `auth` commands.
mod process;
pub mod process_spec;

use crate::auth::{LookupFailureCategory, LookupResult};

pub use process::{ProcessKeyringReader, ProcessLookupFailure, ProcessMutationBackend};
pub use process_spec::ReaderFlavor;

pub trait KeyringReader: Sync {
    fn lookup(&self, workspace: &str) -> LookupResult;
}

pub struct UnsupportedKeyringReader;

impl KeyringReader for UnsupportedKeyringReader {
    fn lookup(&self, _workspace: &str) -> LookupResult {
        LookupResult::Failed(LookupFailureCategory::UnsupportedPlatform)
    }
}

#[cfg(windows)]
pub mod windows;
pub mod windows_spec;

/// The keyring reader for the current platform.
pub struct NativeKeyringReader;
impl KeyringReader for NativeKeyringReader {
    fn lookup(&self, workspace: &str) -> LookupResult {
        #[cfg(target_os = "linux")]
        {
            ProcessKeyringReader::new(ReaderFlavor::SecretTool).lookup(workspace)
        }
        #[cfg(target_os = "macos")]
        {
            ProcessKeyringReader::new(ReaderFlavor::MacSecurity).lookup(workspace)
        }
        #[cfg(windows)]
        {
            windows::lookup_windows(windows::credential(workspace))
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
        {
            UnsupportedKeyringReader.lookup(workspace)
        }
    }
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub use process::ProcessMutationBackend as NativeMutationBackend;
#[cfg(windows)]
pub use windows::WindowsMutationBackend as NativeMutationBackend;
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub struct NativeMutationBackend {
    pub overlay: crate::config::ChildEnvOverlay,
}
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
impl crate::auth::mutation::CredentialMutationBackend for NativeMutationBackend {
    async fn available(&self) -> bool {
        false
    }
    async fn store(
        &self,
        _: &str,
        _: &crate::config::ConfigSecret,
    ) -> Result<(), crate::auth::mutation::MutationFailure> {
        Err(crate::auth::mutation::MutationFailure::Typed(
            crate::error::Error::new("System keyring is unsupported on this platform"),
        ))
    }
    async fn delete(&self, _: &str) -> Result<(), crate::auth::mutation::MutationFailure> {
        Err(crate::auth::mutation::MutationFailure::Typed(
            crate::error::Error::new("System keyring is unsupported on this platform"),
        ))
    }
}
