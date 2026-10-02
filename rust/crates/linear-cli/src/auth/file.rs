//! Reading the credentials file, with a size limit.
use std::fs::OpenOptions;
use std::io::{self, Read};
use std::path::Path;

use crate::auth::keyring::KeyringReader;
use crate::auth::{
    CredentialFormatErrorKind, CredentialManifest, CredentialStore, parse_credentials,
};
use crate::config::{MAX_CONFIG_BYTES, RawConfigFile, parse_config_tier};
use crate::error::{Error, Result};

/// Reads and parses the credentials file at `path`. A missing file (or no
/// known path) is an empty store; keyring entries are read later, on demand.
pub fn load(path: Option<&Path>, keyring: Box<dyn KeyringReader>) -> Result<CredentialStore> {
    let manifest = match path {
        Some(path) => match read(path).map_err(|error| invalid(path, error))? {
            Some(bytes) => {
                let tier = parse_config_tier(RawConfigFile {
                    path: path.to_path_buf(),
                    bytes,
                })
                .map_err(|error| invalid(path, error.kind.to_string()))?;
                parse_credentials(tier).map_err(|error| {
                    invalid(
                        path,
                        match error.kind {
                            CredentialFormatErrorKind::MixedFormat => "mixed credential formats",
                            CredentialFormatErrorKind::WrongType => "invalid value type",
                            CredentialFormatErrorKind::EmptyWorkspace => "empty workspace name",
                            CredentialFormatErrorKind::TooManyWorkspaces => "too many workspaces",
                        },
                    )
                })?
            }
            None => CredentialManifest::empty(),
        },
        None => CredentialManifest::empty(),
    };
    Ok(CredentialStore::new(manifest, keyring))
}

fn invalid(path: &Path, detail: impl std::fmt::Display) -> Error {
    Error::new(format!(
        "invalid credentials file {}: {detail}",
        path.display()
    ))
    .with_hint("Fix or remove the credentials file, then run `linear auth login`.")
}

/// The file's bytes, or `None` when it does not exist.
fn read(path: &Path) -> std::result::Result<Option<Vec<u8>>, String> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // A FIFO in place of the file must not block startup.
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = match options.open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("read failed: {}", error.kind())),
    };
    let metadata = file
        .metadata()
        .map_err(|error| format!("read failed: {}", error.kind()))?;
    if !metadata.is_file() {
        return Err("not a regular file".to_owned());
    }
    if metadata.len() > MAX_CONFIG_BYTES {
        return Err("too large".to_owned());
    }
    let mut bytes = Vec::new();
    file.take(MAX_CONFIG_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read failed: {}", error.kind()))?;
    if u64::try_from(bytes.len()).is_ok_and(|len| len > MAX_CONFIG_BYTES) {
        return Err("too large".to_owned());
    }
    Ok(Some(bytes))
}
