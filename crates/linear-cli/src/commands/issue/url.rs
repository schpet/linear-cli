//! `issue url`: print an issue's URL.
use crate::cli::issue::IssueUrl;
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};

pub fn run(ctx: &Ctx, args: &IssueUrl) -> Result<()> {
    url(ctx, args).context("Failed to get issue URL")
}

fn url(ctx: &Ctx, args: &IssueUrl) -> Result<()> {
    let identifier = super::require(ctx, args.issue_id.as_deref())?;
    let client = ctx.client()?;
    let details = ctx.spin(true, super::details::fetch(client, identifier))?;
    ctx.print(format!("{}\n", details.url))
}
