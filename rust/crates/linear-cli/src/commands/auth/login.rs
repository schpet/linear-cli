//! `auth login`: read an API key, verify it, then store it.
use crate::{
    auth::{
        CredentialFormat,
        mutation::{
            CredentialMutationBackend, CredentialMutationFileWriter, CredentialMutationState,
            MutationFailure,
        },
    },
    config::{ConfigOptions, ConfigSecret, TransportEnvInputs},
    error::Error,
    graphql::{
        bulk_error::{ObservedExchangeFailure, execute_observed},
        envelope::GraphQlRequest,
        operations::auth_login_viewer::AuthLoginViewer,
        transport::{ApiKey, GraphQlTransport},
    },
};
use cynic::QueryBuilder;
use std::path::Path;
pub const CONTEXT: &str = "Failed to login";
pub const SECRET_MESSAGE: &str = "Enter your Linear API key";
pub const SECRET_HINT: &str = "Create one at https://linear.app/settings/account/security";
pub const MIGRATE_MESSAGE: &str =
    "Migrate all credentials to the system keyring for better security?";

pub fn supplied_key(input: Option<&str>) -> Option<ConfigSecret> {
    input
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(|key| ConfigSecret::new(key.to_owned()))
}
pub fn clean_key(key: ConfigSecret) -> Result<ConfigSecret, Error> {
    let trimmed = key
        .expose()
        .trim()
        .trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_');
    if trimmed.is_empty() {
        return Err(Error::new("No API key provided").with_hint(SECRET_HINT));
    }
    Ok(ConfigSecret::new(trimmed.to_owned()))
}
pub fn prepare_transport(
    options: &ConfigOptions,
    env: &TransportEnvInputs,
    key: &ConfigSecret,
) -> Result<GraphQlTransport, Error> {
    let api_key = ApiKey::new(key.expose().to_owned()).map_err(|error| {
        Error::new("API key cannot be used as an HTTP header").with_source(error)
    })?;
    GraphQlTransport::new(
        options.endpoint().value().clone(),
        api_key,
        env.production(),
    )
    .map_err(Error::from)
}
pub async fn authenticate(
    transport: &GraphQlTransport,
) -> Result<AuthLoginViewer, MutationFailure> {
    let mut request = GraphQlRequest::without_variables(AuthLoginViewer::build(()));
    request.query = request.query.trim_end_matches('\n').to_owned();
    execute_observed(transport, &request)
        .await
        .map_err(|failure| match failure {
            ObservedExchangeFailure::Ordinary(error) => MutationFailure::Ordinary(error.message),
            ObservedExchangeFailure::Strict(error) => MutationFailure::Typed(error),
        })
}
fn yellow(value: &str, no_color: bool) -> String {
    if no_color {
        value.to_owned()
    } else {
        format!("\x1b[33m{value}\x1b[39m")
    }
}
#[derive(Clone, Copy, Debug)]
pub struct LoginSaveOptions {
    pub plaintext: bool,
    pub no_color: bool,
}
pub async fn add_authenticated(
    state: &mut CredentialMutationState,
    viewer: AuthLoginViewer,
    key: ConfigSecret,
    options: LoginSaveOptions,
    path: Option<&Path>,
    backend: &impl CredentialMutationBackend,
    writer: &impl CredentialMutationFileWriter,
) -> Result<Vec<u8>, MutationFailure> {
    let LoginSaveOptions {
        plaintext,
        no_color,
    } = options;
    if !plaintext && state.format() != CredentialFormat::Inline && !backend.available().await {
        return Err(MutationFailure::Typed(Error::new(
            "No system keyring found. Use `--plaintext` to store credentials in the config file, or set `LINEAR_API_KEY`.",
        )));
    }
    let org = &viewer.viewer.organization;
    let existed = state.has_workspace(&org.url_key);
    // Preserve absence, never Some(false) from clap bool=false.
    state
        .add(
            &org.url_key,
            key,
            plaintext.then_some(true),
            path,
            backend,
            writer,
        )
        .await?;
    let mut output = format!(
        "{} credentials for workspace: {} ({})\n",
        "Updated", org.name, org.url_key
    );
    if !existed {
        output = format!("Logged in to workspace: {} ({})\n", org.name, org.url_key);
    }
    output.push_str(&format!(
        "  User: {} <{}>\n",
        viewer.viewer.name, viewer.viewer.email
    ));
    if state.workspaces().len() == 1 {
        output.push_str("  Set as default workspace\n");
    }
    if !plaintext && state.format() == CredentialFormat::Inline {
        output.push_str(&yellow(
            "Note: Credential stored as plaintext to match existing format.",
            no_color,
        ));
        output.push('\n');
    }
    Ok(output.into_bytes())
}
pub async fn offer_migration(
    state: &CredentialMutationState,
    backend: &impl CredentialMutationBackend,
) -> bool {
    state.format() == CredentialFormat::Inline && backend.available().await
}
pub fn migration_notice(no_color: bool) -> Vec<u8> {
    format!(
        "\n{}\n",
        yellow(
            "Your credentials are stored as plaintext in the credentials file.",
            no_color
        )
    )
    .into_bytes()
}
pub async fn migrate(
    state: &mut CredentialMutationState,
    path: Option<&Path>,
    backend: &impl CredentialMutationBackend,
    writer: &impl CredentialMutationFileWriter,
) -> Result<Vec<u8>, MutationFailure> {
    let migrated = state.migrate(path, backend, writer).await?;
    Ok(format!(
        "Migrated {} workspace(s) to system keyring.\n",
        migrated.len()
    )
    .into_bytes())
}
pub fn environment_warning(options: &ConfigOptions, no_color: bool) -> Result<Vec<u8>, Error> {
    let input = crate::auth::ApiKeyInput::from_options(options);
    if !matches!(input, crate::auth::ApiKeyInput::Raw { value, .. } if !value.expose().is_empty()) {
        return Ok(Vec::new());
    }
    let mut output = String::from("\n");
    for line in [
        "Warning: LINEAR_API_KEY environment variable is set.",
        "It takes precedence over stored credentials.",
        "Remove it from your shell config to use multi-workspace auth.",
    ] {
        output.push_str(&yellow(line, no_color));
        output.push('\n');
    }
    Ok(output.into_bytes())
}
