//! The system keyring, which holds each workspace's API key when the
//! credentials file lists workspaces without keys.
//!
//! Entries are named by service `linear-cli` and the workspace as account
//! (on Windows, target `linear-cli:<workspace>`), so keys stored by earlier
//! releases stay readable.
#[cfg(any(target_os = "linux", target_os = "macos", all(test, unix)))]
mod process;
#[cfg(any(target_os = "linux", all(test, unix)))]
mod secret_tool;
#[cfg(any(target_os = "macos", all(test, unix)))]
mod security;
#[cfg(windows)]
mod windows;

use crate::config::{ChildEnvOverlay, ConfigSecret};
use crate::error::Result;

/// A workspace's entry in the keyring.
#[derive(Clone, Debug)]
pub enum LookupResult {
    Hit(ConfigSecret),
    Miss,
    Failed(LookupFailureCategory),
}

/// Why a keyring entry could not be read. Never carries the keyring's
/// output, which may include the secret.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LookupFailureCategory {
    /// There is no keyring to ask: the tool is missing, no keyring session
    /// exists, or the platform has none.
    Unavailable,
    /// The keyring tool could not be run.
    #[cfg(any(target_os = "linux", target_os = "macos", all(test, unix)))]
    Permission,
    Other,
}

/// Reads, stores and deletes API keys by workspace.
pub trait Keyring {
    /// The key stored for `workspace`. A missing or empty entry is a miss.
    fn get(&self, workspace: &str) -> LookupResult;
    /// Stores `secret` for `workspace`, replacing any existing entry.
    fn set(&self, workspace: &str, secret: &ConfigSecret) -> Result<()>;
    /// Deletes `workspace`'s entry; deleting a missing entry succeeds.
    fn delete(&self, workspace: &str) -> Result<()>;
    /// Whether a keyring is there to use, checked before asking for a key
    /// so a missing keyring is reported before any other work.
    fn available(&self) -> bool {
        true
    }
}

/// The keyring for this platform. Keyring tools run with the `.env`
/// overlay, like every other subprocess.
pub fn native(overlay: &ChildEnvOverlay) -> Box<dyn Keyring> {
    #[cfg(target_os = "linux")]
    {
        Box::new(secret_tool::SecretTool::new(overlay.clone()))
    }
    #[cfg(target_os = "macos")]
    {
        Box::new(security::Security::new(overlay.clone()))
    }
    #[cfg(windows)]
    {
        let _ = overlay;
        Box::new(windows::CredentialManager)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        let _ = overlay;
        Box::new(Unsupported)
    }
}

/// No system keyring on this platform.
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
struct Unsupported;

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
impl Keyring for Unsupported {
    fn get(&self, _: &str) -> LookupResult {
        LookupResult::Failed(LookupFailureCategory::Unavailable)
    }
    fn set(&self, _: &str, _: &ConfigSecret) -> Result<()> {
        Err(unsupported())
    }
    fn delete(&self, _: &str) -> Result<()> {
        Err(unsupported())
    }
    fn available(&self) -> bool {
        false
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn unsupported() -> crate::error::Error {
    crate::error::Error::new("System keyring is unsupported on this platform")
        .with_hint("Pass --plaintext to store the key in the credentials file.")
}

#[cfg(test)]
mod tests;
