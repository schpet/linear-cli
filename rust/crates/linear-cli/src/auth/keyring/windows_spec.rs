//! Windows credential names and secret decoding.
use crate::{config::ConfigSecret, error::Error};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowsCredentialSpec {
    pub username: String,
    pub target_name: String,
    pub target_alias: String,
    pub comment: String,
}
impl WindowsCredentialSpec {
    pub fn new(workspace: &str) -> Result<Self, Error> {
        let spec = Self {
            username: workspace.to_owned(),
            target_name: format!("linear-cli:{workspace}"),
            target_alias: String::new(),
            comment: String::new(),
        };
        // The same length and NUL checks the `keyring` crate applies.
        for (name, value, limit) in [
            ("username", &spec.username, 513),
            ("target", &spec.target_name, 32767),
            ("alias", &spec.target_alias, 256),
            ("comment", &spec.comment, 256),
        ] {
            if value.len() > limit || value.contains('\0') {
                return Err(Error::new(format!(
                    "Windows credential {name} is not representable"
                )));
            }
        }
        Ok(spec)
    }
}
pub fn decode_windows_secret(bytes: Vec<u8>) -> Result<Option<ConfigSecret>, Error> {
    if bytes.is_empty() {
        return Ok(None);
    }
    let invalid = || Error::new("System keyring secret is not valid UTF-16LE");
    if !bytes.len().is_multiple_of(2) {
        return Err(invalid());
    }
    let units = bytes
        .chunks_exact(2)
        .map(|pair| match pair {
            [low, high] => u16::from_le_bytes([*low, *high]),
            _ => unreachable!("chunks_exact supplies two bytes"),
        })
        .collect::<Vec<_>>();
    String::from_utf16(&units)
        .map(|value| Some(ConfigSecret::new(value)))
        .map_err(|_| invalid())
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowsReadFailure {
    NoEntry,
    NativeFailure,
}
/// Only a missing entry or an empty secret is a miss; a badly encoded secret
/// is a lookup failure, so it is reported rather than silently ignored.
pub fn classify_windows_lookup(
    result: Result<Vec<u8>, WindowsReadFailure>,
) -> crate::auth::LookupResult {
    use crate::auth::{LookupFailureCategory, LookupResult};
    match result {
        Err(WindowsReadFailure::NoEntry) => LookupResult::Miss,
        Err(WindowsReadFailure::NativeFailure) => {
            LookupResult::Failed(LookupFailureCategory::Other)
        }
        Ok(bytes) => match decode_windows_secret(bytes) {
            Ok(Some(secret)) => LookupResult::Hit(secret),
            Ok(None) => LookupResult::Miss,
            Err(_) => LookupResult::Failed(LookupFailureCategory::Other),
        },
    }
}
