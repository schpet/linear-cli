//! Credential stores built from TOML text with a canned keyring.
use std::path::PathBuf;

use crate::auth::keyring::KeyringReader;
use crate::auth::{
    CredentialFormatError, CredentialManifest, CredentialStore, LookupResult, parse_credentials,
};
use crate::config::{ConfigSecret, RawConfigFile, parse_config_tier};

/// A credentials file parsed from `text`.
pub(crate) fn manifest(text: &str) -> Result<CredentialManifest, CredentialFormatError> {
    let tier = parse_config_tier(RawConfigFile {
        path: PathBuf::from("/fake/credentials.toml"),
        bytes: text.as_bytes().to_vec(),
    })
    .expect("valid TOML fixture");
    parse_credentials(tier)
}

/// A store whose keyring answers each listed workspace and misses the rest.
pub(crate) fn store(
    manifest: CredentialManifest,
    keyring: &[(&str, LookupResult)],
) -> CredentialStore {
    let replies = keyring
        .iter()
        .map(|(workspace, result)| ((*workspace).to_owned(), result.clone()))
        .collect();
    CredentialStore::new(manifest, Box::new(CannedKeyring(replies)))
}

pub(crate) fn hit(key: &str) -> LookupResult {
    LookupResult::Hit(ConfigSecret::new(key.to_owned()))
}

struct CannedKeyring(Vec<(String, LookupResult)>);

impl KeyringReader for CannedKeyring {
    fn lookup(&self, workspace: &str) -> LookupResult {
        self.0
            .iter()
            .find(|(name, _)| name == workspace)
            .map_or(LookupResult::Miss, |(_, result)| result.clone())
    }
}
