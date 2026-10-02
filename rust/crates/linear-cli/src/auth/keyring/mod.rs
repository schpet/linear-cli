//! Keyring readers used when a key is needed, and the backends `auth`
//! commands use to store and delete keys.
mod process;
pub mod process_spec;

use crate::auth::{LookupFailureCategory, LookupResult};
use crate::config::ChildEnvOverlay;

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
pub type NativeMutationBackend = ProcessMutationBackend;
#[cfg(windows)]
pub type NativeMutationBackend = windows::WindowsMutationBackend;
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub type NativeMutationBackend = UnsupportedMutationBackend;

/// The keyring backend for the current platform. Tool subprocesses see the
/// `.env` overlay.
pub fn native_backend(overlay: &ChildEnvOverlay) -> NativeMutationBackend {
    #[cfg(target_os = "linux")]
    {
        ProcessMutationBackend::new(ReaderFlavor::SecretTool, overlay.clone())
    }
    #[cfg(target_os = "macos")]
    {
        ProcessMutationBackend::new(ReaderFlavor::MacSecurity, overlay.clone())
    }
    #[cfg(windows)]
    {
        let _ = overlay;
        windows::WindowsMutationBackend
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        let _ = overlay;
        UnsupportedMutationBackend
    }
}

/// No system keyring on this platform.
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub struct UnsupportedMutationBackend;

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
impl crate::auth::mutation::KeyringBackend for UnsupportedMutationBackend {
    async fn available(&self) -> bool {
        false
    }
    async fn store(&self, _: &str, _: &crate::config::ConfigSecret) -> crate::error::Result<()> {
        Err(crate::error::Error::new(
            "System keyring is unsupported on this platform",
        ))
    }
    async fn delete(&self, _: &str) -> crate::error::Result<()> {
        Err(crate::error::Error::new(
            "System keyring is unsupported on this platform",
        ))
    }
}
