//! `auth login`: check an API key with Linear, then store it.
use std::io::Read;

use crate::auth::keyring::Keyring;
use crate::auth::mutation::Credentials;
use crate::auth::{ApiKeyInput, CredentialFormat};
use crate::cli::auth::AuthLogin;
use crate::client::{ApiKey, LinearClient, RequestError};
use crate::config::{ConfigSecret, OptionSource};
use crate::ctx::Ctx;
use crate::error::{Error, Failure, Result, ResultExt};
use crate::graphql::operations::user::GetViewerAccount;
use crate::platform::style;

const KEY_HINT: &str = "Create one at https://linear.app/settings/account/security";

pub fn run(ctx: &Ctx, args: &AuthLogin) -> Result<()> {
    login(ctx, args).context("Failed to login")
}

fn login(ctx: &Ctx, args: &AuthLogin) -> Result<()> {
    let mut credentials = super::credentials(ctx)?;
    let store = ctx.credentials()?;
    let keyring = store.keyring();
    // A configured key already authenticates every command, so with nothing to
    // store and no `--plaintext` there is no work for `auth login` to do. It
    // succeeds instead of failing on a keyring it does not need.
    if args.key.is_none()
        && !args.plaintext
        && let Some(source) = configured_key(ctx)
    {
        return ctx.print(already_configured(&source));
    }
    let plaintext = credentials.stores_plaintext(args.plaintext);
    if !plaintext && !keyring.available() {
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
        ApiKey::new(key.expose()).map_err(|error| {
            Error::auth("API key cannot be used as an HTTP header").with_source(error)
        })?,
        ctx.config().network_env.client_config(),
    )?;
    let viewer = ctx
        .spin(true, client.query::<GetViewerAccount, _>(()))
        .map_err(rejected_key)?
        .viewer;
    let organization = &viewer.organization;
    let existed = credentials.has_workspace(&organization.url_key);
    let was_keyring = credentials.format() == CredentialFormat::Metadata;
    {
        let _spinner = ctx.spinner(true, "");
        credentials.add(&organization.url_key, key, args.plaintext, store, keyring)?;
    }

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
        offer_migration(ctx, &mut credentials, keyring)?;
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

/// Where a key that makes `auth login` unnecessary came from. `None` when no
/// key is set or it is empty: `auth::resolve` treats an empty `LINEAR_API_KEY`
/// as absent.
fn configured_key(ctx: &Ctx) -> Option<OptionSource> {
    let key = ctx.options().api_key()?;
    if key.value().expose().is_empty() {
        return None;
    }
    Some(key.source().clone())
}

/// What `auth login` prints when a configured key leaves it nothing to store.
fn already_configured(source: &OptionSource) -> String {
    let origin = match source {
        OptionSource::Env => "LINEAR_API_KEY is set".to_owned(),
        OptionSource::ProjectEnv { path } => {
            format!("LINEAR_API_KEY is set in {}", path.display())
        }
        OptionSource::ProjectConfig { path } | OptionSource::GlobalConfig { path } => {
            format!("An API key is set in {}", path.display())
        }
        OptionSource::Cli => "An API key is set on the command line".to_owned(),
    };
    format!(
        "{origin}, so there is nothing to store. Commands will use it.\n\
         Pass --plaintext to store a copy in the credentials file, or --key to use a different key.\n"
    )
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
    let key = ctx.prompter()?.secret("API key:", KEY_HINT)?;
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
    match failure.failure() {
        Failure::Auth => Error::auth("Invalid API key")
            .with_hint("Check that your API key is correct and not expired.")
            .with_source(failure),
        Failure::General | Failure::NotFound | Failure::Unavailable => Error::from(failure),
    }
}

/// Offers to move plaintext keys to the keyring. Only asked on a terminal;
/// otherwise the command is suggested.
fn offer_migration(ctx: &Ctx, credentials: &mut Credentials, keyring: &dyn Keyring) -> Result<()> {
    if !keyring.available() {
        return Ok(());
    }
    let color = ctx.terminal().stderr_color();
    let notice = style::yellow(
        "Your credentials are stored as plaintext in the credentials file.",
        color,
    );
    if !ctx.interactive() {
        return ctx.eprint(format!(
            "\n{notice}\nRun `linear auth migrate` to move them to the system keyring.\n"
        ));
    }
    ctx.eprint(format!("\n{notice}\n"))?;
    let migrate = ctx.prompter()?.confirm(
        "Migrate all credentials to the system keyring for better security?",
        true,
    )?;
    if !migrate {
        return Ok(());
    }
    let migrated = {
        let _spinner = ctx.spinner(true, "");
        credentials.migrate(keyring)?
    };
    ctx.print(format!(
        "Migrated {} workspace(s) to system keyring.\n",
        migrated.len()
    ))
}
