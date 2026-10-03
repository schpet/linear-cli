//! `issue describe`: a commit message for an issue, with Linear trailers.
use crate::{
    cli::issue::IssueDescribe,
    commands::issue::details,
    ctx::Ctx,
    error::{Result, ResultExt},
};
pub fn run(ctx: &Ctx, args: &IssueDescribe) -> Result<()> {
    describe(ctx, args).context("Failed to get issue description")
}

fn describe(ctx: &Ctx, args: &IssueDescribe) -> Result<()> {
    let identifier = super::require(ctx, args.issue_id.as_deref())?;
    let client = ctx.client()?;
    let detail = ctx.spin(true, details::fetch(client, identifier.clone()))?;
    ctx.print(format(
        &identifier,
        &detail.title,
        &detail.url,
        args.references,
    ))
}
pub fn format(identifier: &str, title: &str, url: &str, references: bool) -> Vec<u8> {
    let magic = if references { "References" } else { "Fixes" };
    format!("{identifier} {title}\n\nLinear-issue: {magic} {identifier}\nLinear-issue-url: {url}\n")
        .into_bytes()
}
