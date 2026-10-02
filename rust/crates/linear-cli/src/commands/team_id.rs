use crate::commands::team_key::configured_team_key;
use crate::ctx::Ctx;
use crate::error::{Error, Result};

pub fn render(ctx: &Ctx) -> Result<String> {
    match configured_team_key(ctx.options()) {
        Some(key) => Ok(format!("{key}\n")),
        None => Err(Error::new("No team id configured")
            .context("Failed to get team id")
            .with_hint("Run `linear config` to set a team.")),
    }
}
