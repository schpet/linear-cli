//! `auth login`: check an API key with Linear, then store it.
use std::io::Read;

use reqwest::StatusCode;

use crate::auth::keyring::native_backend;
use crate::auth::mutation::{Credentials, KeyringBackend};
use crate::auth::{ApiKeyInput, CredentialFormat};
use crate::cli::auth::AuthLogin;
use crate::client::{ApiKey, LinearClient, RequestError};
use crate::config::ConfigSecret;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::ResponseGraphQlError;
use crate::graphql::operations::user::AuthLoginViewer;
use crate::platform::style;

const KEY_HINT: &str = "Create one at https://linear.app/settings/account/security";

pub fn run(ctx: &Ctx, args: &AuthLogin) -> Result<()> {
    login(ctx, args).context("Failed to login")
}

fn login(ctx: &Ctx, args: &AuthLogin) -> Result<()> {
    let mut credentials = super::credentials(ctx)?;
    let store = ctx.credentials()?;
    let backend = native_backend(&ctx.config().child_env);
    let plaintext = credentials.stores_plaintext(args.plaintext);
    if !plaintext && !ctx.block_on(backend.available()) {
        return Err(Error::new("No system keyring found").with_hint(
            "Pass --plaintext to store the key in the credentials file, or set LINEAR_API_KEY.",
        ));
    }
    let key = clean_key(match &args.key {
        Some(key) => ConfigSecret::new(key.clone()),
        None => read_key(ctx)?,
    })?;
    let client = LinearClient::new(
        ctx.options().endpoint().value().clone(),
        ApiKey::new(key.expose().to_owned()).map_err(|error| {
            Error::new("API key cannot be used as an HTTP header").with_source(error)
        })?,
        ctx.config().transport_env.production(),
    )?;
    let viewer = ctx
        .spin(true, client.query::<AuthLoginViewer, _>(()))
        .map_err(rejected_key)?
        .viewer;
    let organization = &viewer.organization;
    let existed = credentials.has_workspace(&organization.url_key);
    let was_keyring = credentials.format() == CredentialFormat::Metadata;
    ctx.spin(
        true,
        credentials.add(&organization.url_key, key, args.plaintext, store, &backend),
    )?;

    let color = ctx.color();
    let mut output = if existed {
        format!(
            "Updated credentials for workspace: {} ({})\n",
            organization.name, organization.url_key
        )
    } else {
        format!(
            "Logged in to workspace: {} ({})\n",
            organization.name, organization.url_key
        )
    };
    output.push_str(&format!("  User: {} <{}>\n", viewer.name, viewer.email));
    if credentials.workspaces().len() == 1 {
        output.push_str("  Set as default workspace\n");
    }
    if plaintext && !args.plaintext {
        output.push_str(&style::yellow(
            "Note: Credential stored as plaintext to match existing format.",
            color,
        ));
        output.push('\n');
    } else if args.plaintext && was_keyring && credentials.workspaces().len() > 1 {
        output.push_str(&style::yellow(
            "Note: Every workspace's key is now stored as plaintext in the credentials file.",
            color,
        ));
        output.push('\n');
    }
    ctx.print(output)?;

    if plaintext && !args.plaintext {
        offer_migration(ctx, &mut credentials, &backend)?;
    }
    if matches!(ApiKeyInput::from_options(ctx.options()), ApiKeyInput::Raw { value, .. } if !value.expose().is_empty())
    {
        let warning = "Warning: LINEAR_API_KEY is set and takes precedence over stored credentials.\nRemove it from your shell config to use multi-workspace auth.";
        ctx.eprint(format!(
            "{}\n",
            style::warning(warning, ctx.terminal().stderr_color())
        ))?;
    }
    Ok(())
}

/// The key from a prompt on a terminal, or from stdin when it is piped.
fn read_key(ctx: &Ctx) -> Result<ConfigSecret> {
    if !ctx.stdin_tty() {
        let mut key = String::new();
        std::io::stdin().read_to_string(&mut key).map_err(|error| {
            Error::new("Could not read the API key from stdin").with_source(error)
        })?;
        return Ok(ConfigSecret::new(key));
    }
    let key = ctx
        .prompter()?
        .secret("Enter your Linear API key", KEY_HINT)?;
    Ok(ConfigSecret::new(key))
}

/// Trims whitespace and pasted punctuation (quotes, brackets) around the key.
fn clean_key(key: ConfigSecret) -> Result<ConfigSecret> {
    let trimmed = key
        .expose()
        .trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_');
    if trimmed.is_empty() {
        return Err(Error::new("No API key provided").with_hint(KEY_HINT));
    }
    Ok(ConfigSecret::new(trimmed.to_owned()))
}

/// Linear refused the key: HTTP 401/403 or an authentication error.
fn rejected_key(failure: RequestError) -> Error {
    let refused = match &failure {
        RequestError::GraphQl { status, errors, .. } => {
            refused_status(*status) || errors.iter().any(authentication_error)
        }
        RequestError::Http { response, .. } => refused_status(response.status),
        RequestError::ResponseTooLarge { status, .. } => refused_status(*status),
        RequestError::RequestBody(_)
        | RequestError::Response(_)
        | RequestError::Timeout { .. }
        | RequestError::Network { .. } => false,
    };
    if refused {
        Error::auth("Invalid API key")
            .with_hint("Check that your API key is correct and not expired.")
            .with_source(failure)
    } else {
        Error::from(failure)
    }
}

fn refused_status(status: StatusCode) -> bool {
    status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN
}

fn authentication_error(error: &ResponseGraphQlError) -> bool {
    error
        .extensions
        .as_ref()
        .and_then(|extensions| extensions.get("code"))
        .and_then(serde_json::Value::as_str)
        == Some("AUTHENTICATION_ERROR")
}

/// Offers to move plaintext keys to the keyring. Only asked on a terminal;
/// otherwise the command is suggested.
fn offer_migration(
    ctx: &Ctx,
    credentials: &mut Credentials,
    backend: &impl KeyringBackend,
) -> Result<()> {
    if !ctx.block_on(backend.available()) {
        return Ok(());
    }
    let color = ctx.color();
    let notice = style::yellow(
        "Your credentials are stored as plaintext in the credentials file.",
        color,
    );
    if !ctx.interactive() {
        return ctx.print(format!(
            "\n{notice}\nRun `linear auth migrate` to move them to the system keyring.\n"
        ));
    }
    ctx.print(format!("\n{notice}\n"))?;
    let migrate = ctx.prompter()?.confirm(
        "Migrate all credentials to the system keyring for better security?",
        true,
    )?;
    if !migrate {
        return Ok(());
    }
    let migrated = ctx.spin(true, credentials.migrate(backend))?;
    ctx.print(format!(
        "Migrated {} workspace(s) to system keyring.\n",
        migrated.len()
    ))
}
