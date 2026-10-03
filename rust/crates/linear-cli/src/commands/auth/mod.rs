//! `linear auth`: stored workspace credentials.
mod default;
mod list;
mod login;
mod logout;
mod migrate;
mod token;
mod whoami;

use crate::auth::mutation::Credentials;
use crate::cli::auth::AuthCommand;
use crate::ctx::Ctx;
use crate::error::{Error, Result};

pub fn run(ctx: &Ctx, command: &AuthCommand) -> Result<()> {
    match command {
        AuthCommand::Login(args) => login::run(ctx, args),
        AuthCommand::Logout(args) => logout::run(ctx, args),
        AuthCommand::List(_) => list::run(ctx),
        AuthCommand::Default(args) => default::run(ctx, args),
        AuthCommand::Token(_) => token::run(ctx),
        AuthCommand::Whoami(_) => whoami::run(ctx),
        AuthCommand::Migrate(_) => migrate::run(ctx),
    }
}

/// The workspace a command names: its positional argument, or else the
/// global `--workspace`. Naming two different workspaces is an error.
fn named_workspace<'a>(ctx: &'a Ctx, positional: Option<&'a str>) -> Result<Option<&'a str>> {
    match (positional, ctx.workspace()) {
        (Some(named), Some(flag)) if named != flag => Err(Error::new(format!(
            "Two different workspaces given: {named} and --workspace {flag}"
        ))
        .with_hint("Name the workspace once.")),
        (Some(named), _) => Ok(Some(named)),
        (None, flag) => Ok(flag),
    }
}

/// The credentials file, ready to change.
fn credentials(ctx: &Ctx) -> Result<Credentials> {
    let path = ctx.credentials_path().ok_or_else(|| {
        Error::new("Could not determine where the credentials file lives")
            .with_hint("Set HOME (or XDG_CONFIG_HOME).")
    })?;
    Ok(Credentials::new(ctx.credentials()?, path))
}
