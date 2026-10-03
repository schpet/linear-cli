//! `auth token`: print the API key the other commands would use.
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};

pub fn run(ctx: &Ctx) -> Result<()> {
    token(ctx).context("Failed to get API token")
}

fn token(ctx: &Ctx) -> Result<()> {
    let key = ctx.api_key()?;
    ctx.print(format!("{}\n", key.expose()))
}
