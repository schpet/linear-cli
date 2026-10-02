//! Client construction for commands that do not use `Ctx::client` yet.
use crate::auth::{CredentialSelectionInputs, CredentialStore};
use crate::config::{ConfigOptions, TransportEnvInputs};
use crate::error::Result;
use crate::graphql::transport::GraphQlTransport;

pub fn prepare_transport(
    options: &ConfigOptions,
    credentials: &CredentialStore,
    cli_workspace: Option<&str>,
    transport_env: &TransportEnvInputs,
) -> Result<GraphQlTransport> {
    let inputs = selection_inputs(options, cli_workspace);
    prepare_transport_with_inputs(options, credentials, &inputs, transport_env)
}

pub(crate) fn selection_inputs<'a>(
    options: &'a ConfigOptions,
    cli_workspace: Option<&'a str>,
) -> CredentialSelectionInputs<'a> {
    crate::ctx::selection_inputs(options, cli_workspace)
}

pub(crate) fn prepare_transport_with_inputs(
    options: &ConfigOptions,
    credentials: &CredentialStore,
    inputs: &CredentialSelectionInputs<'_>,
    transport_env: &TransportEnvInputs,
) -> Result<GraphQlTransport> {
    crate::ctx::connect(options, credentials, inputs, transport_env)
}
