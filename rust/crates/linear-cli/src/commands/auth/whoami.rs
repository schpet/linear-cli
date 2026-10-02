//! One read-only `AuthStatus` request and its human presentation.
use std::future::Future;

use cynic::QueryBuilder;

use crate::auth::CredentialStore;
use crate::commands::client;
use crate::config::{ConfigOptions, TransportEnvInputs};
use crate::error::{Error, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::auth_whoami::AuthStatus;
use crate::graphql::transport::GraphQlTransport;

const CONTEXT: &str = "Failed to get user info";

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
pub async fn run_with<F, Fut>(fetch: F) -> Result<Vec<u8>, Error>
where
    F: FnOnce(GraphQlRequest<()>) -> Fut,
    Fut: Future<Output = Result<AuthStatus, Error>>,
{
    let request = GraphQlRequest::without_variables(AuthStatus::build(()));
    let status = fetch(request).await.context(CONTEXT)?;
    Ok(render(&status))
}

pub async fn run(transport: &GraphQlTransport) -> Result<Vec<u8>, Error> {
    run_with(|request| async move { transport.execute(&request).await.map_err(Error::from) }).await
}

/// Select from already loaded credentials, then resolve the ambient
/// transport policy. No process state or credential file is read here.
pub fn prepare_transport(
    options: &ConfigOptions,
    credentials: &CredentialStore,
    cli_workspace: Option<&str>,
    transport_env: &TransportEnvInputs,
) -> Result<GraphQlTransport, Error> {
    client::prepare_transport(options, credentials, cli_workspace, transport_env).context(CONTEXT)
}
