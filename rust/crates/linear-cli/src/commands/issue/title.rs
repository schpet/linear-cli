//! `issue title`: print an issue's title.
use crate::cli::issue::IssueTitle;
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};

pub fn run(ctx: &Ctx, args: &IssueTitle) -> Result<()> {
    title(ctx, args).context("Failed to get issue title")
}

fn title(ctx: &Ctx, args: &IssueTitle) -> Result<()> {
    let identifier = super::require(ctx, args.issue_id.as_deref())?;
    let client = ctx.client()?;
    let details = ctx.spin(true, super::details::fetch(client, identifier))?;
    ctx.print(format!("{}\n", details.title))
}
