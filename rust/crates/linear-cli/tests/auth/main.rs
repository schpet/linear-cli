mod file;
mod format;
mod header;
#[cfg(unix)]
mod keyring;
mod path;
mod resolve;

mod write;

mod source_properties;

mod auth_mutation;

mod native_reader_spec;

use linear_cli::auth::keyring::KeyringReader;
use linear_cli::auth::{CredentialManifest, CredentialStore, LookupResult};

/// A canned keyring answer for one workspace.
pub struct LookupReply {
    pub workspace: String,
    pub result: LookupResult,
}

struct Replies(Vec<LookupReply>);

impl KeyringReader for Replies {
    fn lookup(&self, workspace: &str) -> LookupResult {
        self.0
            .iter()
            .find(|reply| reply.workspace == workspace)
            .map_or(LookupResult::Miss, |reply| reply.result.clone())
    }
}

/// A store whose keyring answers with `replies`.
pub fn hydrate(
    manifest: CredentialManifest,
    replies: Vec<LookupReply>,
) -> Result<CredentialStore, std::convert::Infallible> {
    Ok(CredentialStore::new(manifest, Box::new(Replies(replies))))
}
