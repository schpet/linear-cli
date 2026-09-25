//! One read-only `AuthStatus` request and its human presentation.
use std::future::Future;

use cynic::QueryBuilder;

use crate::auth::{
    self, ApiKeyInput, CredentialSelection, CredentialSelectionInputs, CredentialStore,
};
use crate::config::{ConfigOptions, TransportEnvInputs};
use crate::error::{AppError, AppErrorKind};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::auth_whoami::AuthStatus;
use crate::graphql::transport::GraphQlTransport;

const CONTEXT: &str = "Failed to get user info";
const NO_KEY: &str = "No API key configured. Set LINEAR_API_KEY, add api_key to .linear.toml, or run `linear auth login`.";
const RAW_WORKSPACE_CONFLICT: &str = "Cannot use --workspace flag when LINEAR_API_KEY environment variable is set. Either unset LINEAR_API_KEY or remove the --workspace flag.";

pub fn render(status: &AuthStatus) -> Vec<u8> {
    let viewer = &status.viewer;
    let organization = &viewer.organization;
    let mut output = format!(
        "Workspace: {}\n  Slug: {}\n  URL: https://linear.app/{}\nUser: {}\n",
        organization.name, organization.url_key, organization.url_key, viewer.name
    );
    if viewer.display_name != viewer.name {
        output.push_str(&format!("  Display name: {}\n", viewer.display_name));
    }
    output.push_str(&format!("  Email: {}\n", viewer.email));
    if viewer.admin {
        output.push_str("  Role: admin\n");
    } else if viewer.guest {
        output.push_str("  Role: guest\n");
    }
    output.into_bytes()
}

/// The operation is injected at this boundary so the command can be tested
/// without process credentials, network policy or a live workspace.
pub async fn run_with<F, Fut>(fetch: F) -> Result<Vec<u8>, AppError>
where
    F: FnOnce(GraphQlRequest<()>) -> Fut,
    Fut: Future<Output = Result<AuthStatus, AppError>>,
{
    let request = GraphQlRequest::without_variables(AuthStatus::build(()));
    let status = fetch(request)
        .await
        .map_err(|error| error.with_context(CONTEXT))?;
    Ok(render(&status))
}

pub async fn run(transport: &GraphQlTransport) -> Result<Vec<u8>, AppError> {
    run_with(|request| async move { transport.execute(&request).await.map_err(AppError::from) })
        .await
}

/// Select from already loaded credentials, then resolve the reviewed ambient
/// transport policy. No process state or credential file is read here.
pub fn prepare_transport(
    options: &ConfigOptions,
    credentials: &CredentialStore,
    cli_workspace: Option<&str>,
    transport_env: &TransportEnvInputs,
) -> Result<GraphQlTransport, AppError> {
    let api_key = ApiKeyInput::from_options(options).map_err(|error| {
        AppError::new(AppErrorKind::Invariant, error.to_string()).with_context(CONTEXT)
    })?;
    let sourced_workspace = options
        .workspace()
        .map(|resolved| (resolved.value().as_str(), resolved.source().clone()));
    let inputs = CredentialSelectionInputs {
        api_key,
        cli_workspace,
        sourced_workspace,
    };
    let selected = auth::resolve(&inputs, credentials);
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
        .with_context(CONTEXT)
    })?;
    let config = transport_env
        .production()
        .map_err(|error| AppError::from(error).with_context(CONTEXT))?;
    GraphQlTransport::new(options.endpoint().value().clone(), key, config)
        .map_err(|error| AppError::from(error).with_context(CONTEXT))
}

fn credential_error(message: impl Into<String>) -> AppError {
    AppError::new(AppErrorKind::Validation, message).with_context(CONTEXT)
}
