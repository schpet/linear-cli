//! Reading the credentials file, with a size limit.
use std::fs::OpenOptions;
use std::io::{self, Read};
use std::path::Path;

use crate::config::MAX_CONFIG_BYTES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialReadFailure {
    NotRegular,
    TooLarge,
    Io(io::ErrorKind),
}

pub trait CredentialFileSource {
    fn read_credentials(&self, path: &Path) -> Result<Option<Vec<u8>>, CredentialReadFailure>;
}

pub struct RealCredentialFileSource;

impl CredentialFileSource for RealCredentialFileSource {
    fn read_credentials(&self, path: &Path) -> Result<Option<Vec<u8>>, CredentialReadFailure> {
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NONBLOCK);
        }
        let file = match options.open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(CredentialReadFailure::Io(error.kind())),
        };
        let metadata = file
            .metadata()
            .map_err(|error| CredentialReadFailure::Io(error.kind()))?;
        if !metadata.is_file() {
            return Err(CredentialReadFailure::NotRegular);
        }
        if metadata.len() > MAX_CONFIG_BYTES {
            return Err(CredentialReadFailure::TooLarge);
        }
        let mut bytes = Vec::new();
        file.take(MAX_CONFIG_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| CredentialReadFailure::Io(error.kind()))?;
        if u64::try_from(bytes.len()).is_ok_and(|len| len > MAX_CONFIG_BYTES) {
            return Err(CredentialReadFailure::TooLarge);
        }
        Ok(Some(bytes))
    }
}
