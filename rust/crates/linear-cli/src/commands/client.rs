//! Build a GraphQL transport from already-loaded config and credentials.
//!
//! The caller owns command-specific error context. This boundary returns
//! uncontextualized errors so commands whose client setup is outside a catch
//! path can preserve that behavior.

use crate::auth::{
    self, ApiKeyInput, CredentialSelection, CredentialSelectionInputs, CredentialStore,
};
use crate::config::{ConfigOptions, TransportEnvInputs};
use crate::error::{AppError, AppErrorKind};
use crate::graphql::transport::GraphQlTransport;

const NO_KEY: &str = "No API key configured. Set LINEAR_API_KEY, add api_key to .linear.toml, or run `linear auth login`.";
const RAW_WORKSPACE_CONFLICT: &str = "Cannot use --workspace flag when LINEAR_API_KEY environment variable is set. Either unset LINEAR_API_KEY or remove the --workspace flag.";

pub fn prepare_transport(
    options: &ConfigOptions,
    credentials: &CredentialStore,
    cli_workspace: Option<&str>,
    transport_env: &TransportEnvInputs,
) -> Result<GraphQlTransport, AppError> {
    let inputs = selection_inputs(options, cli_workspace)?;
    prepare_transport_with_inputs(options, credentials, &inputs, transport_env)
}

/// Build selection inputs once when a command also needs URL workspace
/// provenance. The ordinary transport path uses this same construction.
pub(crate) fn selection_inputs<'a>(
    options: &'a ConfigOptions,
    cli_workspace: Option<&'a str>,
) -> Result<CredentialSelectionInputs<'a>, AppError> {
    let api_key = ApiKeyInput::from_options(options)
        .map_err(|error| AppError::new(AppErrorKind::Invariant, error.to_string()))?;
    let sourced_workspace = options
        .workspace()
        .map(|resolved| (resolved.value().as_str(), resolved.source().clone()));
    Ok(CredentialSelectionInputs {
        api_key,
        cli_workspace,
        sourced_workspace,
    })
}

/// `inputs` must come from `selection_inputs` for these same `options`.
pub(crate) fn prepare_transport_with_inputs(
    options: &ConfigOptions,
    credentials: &CredentialStore,
    inputs: &CredentialSelectionInputs<'_>,
    transport_env: &TransportEnvInputs,
) -> Result<GraphQlTransport, AppError> {
    let selected = auth::resolve(inputs, credentials);
    let secret = match selected {
        CredentialSelection::Selected { secret, .. } => secret,
        CredentialSelection::NoKey => return Err(credential_error(NO_KEY)),
        CredentialSelection::EnvWorkspaceConflict => {
            return Err(credential_error(RAW_WORKSPACE_CONFLICT));
        }
        CredentialSelection::MissingExplicitWorkspace { workspace } => {
            return Err(credential_error(format!(
                "Workspace \"{workspace}\" not found in credentials. Run `linear auth login` to add it, or `linear auth list` to see configured workspaces."
            )));
        }
    };
    let key = auth::header::to_api_key(secret).map_err(|error| {
        AppError::new(
            AppErrorKind::Validation,
            "API key cannot be used as an HTTP header",
        )
        .with_source(error)
    })?;
    let config = transport_env.production();
    GraphQlTransport::new(options.endpoint().value().clone(), key, config).map_err(AppError::from)
}

fn credential_error(message: impl Into<String>) -> AppError {
    AppError::new(AppErrorKind::Validation, message)
}
