//! Raw selected token output, without HTTP normalization or post-resolution lookup.
use crate::auth::{self, CredentialSelection, CredentialStore};
use crate::commands::client;
use crate::config::ConfigOptions;
use crate::error::Error;

pub const CONTEXT: &str = "Failed to get API token";

pub fn run(
    options: &ConfigOptions,
    store: &CredentialStore,
    workspace: Option<&str>,
) -> Result<Vec<u8>, Error> {
    let inputs = client::selection_inputs(options, workspace);
    match auth::resolve(&inputs, store) {
        CredentialSelection::Selected { secret, .. } => {
            let mut bytes = secret.expose().as_bytes().to_vec();
            bytes.push(b'\n');
            Ok(bytes)
        }
        CredentialSelection::NoKey => Err(Error::auth("No API key configured")
            .with_hint("Set LINEAR_API_KEY, add api_key to .linear.toml, or run `linear auth login`.")
            .context(CONTEXT)),
        CredentialSelection::EnvWorkspaceConflict => Err(Error::new("Cannot use --workspace flag when LINEAR_API_KEY environment variable is set. Either unset LINEAR_API_KEY or remove the --workspace flag.").context(CONTEXT)),
        CredentialSelection::MissingExplicitWorkspace { workspace } => Err(Error::new(format!("Workspace \"{workspace}\" not found in credentials. Run `linear auth login` to add it, or `linear auth list` to see configured workspaces.")).context(CONTEXT)),
    }
}
