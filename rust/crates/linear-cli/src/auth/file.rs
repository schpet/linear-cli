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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::test_support::keyring;
    use crate::auth::{CredentialWarning, LookupFailureCategory, LookupResult};

    /// Reads `path` with a keyring that cannot be reached.
    fn read(path: &Path) -> Result<CredentialStore> {
        let unavailable = LookupResult::Failed(LookupFailureCategory::Unavailable);
        load(
            Some(path),
            keyring(&[("a", unavailable.clone()), ("b", unavailable)]),
        )
    }

    fn failure(path: &Path) -> String {
        read(path).expect_err("unreadable file").to_string()
    }

    #[test]
    fn a_missing_file_is_an_empty_store_and_other_unreadable_paths_fail() {
        let dir = tempfile::tempdir().expect("temp dir");
        let missing = read(&dir.path().join("missing")).expect("missing file");
        assert!(missing.workspaces().is_empty());
        let file = dir.path().join("credentials.toml");
        std::fs::write(&file, b"default = 'demo'\n").expect("write");
        assert!(read(&file).is_ok());
        assert!(failure(dir.path()).contains("not a regular file"));
        assert!(failure(&file.join("child")).contains("read failed"));
        std::fs::write(&file, vec![b'x'; 1024 * 1024 + 1]).expect("write");
        assert!(failure(&file).contains("too large"));
    }

    #[cfg(unix)]
    #[test]
    fn a_fifo_is_refused_without_blocking() {
        let dir = tempfile::tempdir().expect("temp dir");
        let fifo = dir.path().join("fifo");
        let status = std::process::Command::new("/usr/bin/mkfifo")
            .arg(&fifo)
            .status()
            .expect("run mkfifo");
        assert!(status.success());
        assert!(failure(&fifo).contains("not a regular file"));
    }

    #[test]
    fn keyring_entries_are_read_only_when_a_key_is_needed() {
        let dir = tempfile::tempdir().expect("temp dir");
        let file = dir.path().join("credentials.toml");
        std::fs::write(&file, b"default = 'a'\nworkspaces = ['a', 'b']\n").expect("write");
        let store = read(&file).expect("store");
        assert!(store.take_warnings().is_empty());
        assert!(store.key("b").is_none());
        assert_eq!(
            store.take_warnings(),
            [CredentialWarning::LookupFailed {
                workspace: "b".to_owned(),
                category: LookupFailureCategory::Unavailable,
            }]
        );
    }
}
