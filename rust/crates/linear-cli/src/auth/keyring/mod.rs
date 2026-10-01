//! Explicit startup readers and separate mutation backends; tests inject fakes.
#[cfg(any(target_os = "linux", target_os = "macos"))]
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

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub mod process;
#[cfg(windows)]
pub mod windows;
pub mod windows_spec;

/// Only new mutation leaves opt into the full native startup readers.
pub fn native_auth_route(command: Option<&crate::cli::RootCommand>) -> bool {
    matches!(
        command,
        Some(crate::cli::RootCommand::Auth(crate::cli::auth::Auth {
            command: Some(
                crate::cli::auth::AuthCommand::Login(_)
                    | crate::cli::auth::AuthCommand::Logout(_)
                    | crate::cli::auth::AuthCommand::Migrate(_)
            )
        }))
    )
}
pub struct RoutedKeyringReader {
    native_auth: bool,
}
impl RoutedKeyringReader {
    pub fn new(native_auth: bool) -> Self {
        Self { native_auth }
    }
}
impl KeyringReader for RoutedKeyringReader {
    fn lookup(&self, workspace: &str) -> LookupResult {
        #[cfg(target_os = "linux")]
        {
            let _native_auth = self.native_auth;
            LinuxKeyringReader::new().lookup(workspace)
        }
        #[cfg(target_os = "macos")]
        {
            if self.native_auth {
                linux::LinuxKeyringReader::macos().lookup(workspace)
            } else {
                UnsupportedKeyringReader.lookup(workspace)
            }
        }
        #[cfg(windows)]
        {
            if self.native_auth {
                windows::lookup_windows(windows::credential(workspace))
            } else {
                UnsupportedKeyringReader.lookup(workspace)
            }
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
        {
            let _native_auth = self.native_auth;
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
            crate::error::AppError::new(
                crate::error::AppErrorKind::IoProcess,
                "System keyring is unsupported on this platform",
            ),
        ))
    }
    async fn delete(&self, _: &str) -> Result<(), crate::auth::mutation::MutationFailure> {
        Err(crate::auth::mutation::MutationFailure::Typed(
            crate::error::AppError::new(
                crate::error::AppErrorKind::IoProcess,
                "System keyring is unsupported on this platform",
            ),
        ))
    }
}
