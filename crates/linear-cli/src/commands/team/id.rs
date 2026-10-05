//! `team id`: print the configured team key.
use crate::cli::team::TeamId;
use crate::commands::team_key::configured_team_key;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};

pub fn run(ctx: &Ctx, _args: &TeamId) -> Result<()> {
    id(ctx).context("Failed to get the default team")
}

fn id(ctx: &Ctx) -> Result<()> {
    let key = configured_team_key(ctx.options()).ok_or_else(|| {
        Error::new("No default team configured")
            .with_hint("Run `linear config` to set a default team.")
    })?;
    ctx.print(format!("{key}\n"))
}
