//! `issue describe`: a commit message for an issue, with Linear trailers.
use crate::{
    cli::issue::IssueDescribe,
    commands::issue::details,
    ctx::Ctx,
    error::{Error, Result, ResultExt},
    graphql::{
        operations::issue_details::{GetIssueDetails, IssueDetails},
        transport::GraphQlTransport,
    },
};
pub fn run(ctx: &Ctx, args: &IssueDescribe) -> Result<()> {
    describe(ctx, args).context("Failed to get issue description")
}

fn describe(ctx: &Ctx, args: &IssueDescribe) -> Result<()> {
    let identifier = super::require(ctx, args.issue_id.as_deref())?;
    let client = ctx.client()?;
    let detail = ctx.spin(true, fetch(client, &identifier))?;
    ctx.print(format(
        &identifier,
        &detail.title,
        &detail.url,
        args.references,
    ))
}
pub async fn fetch(transport: &GraphQlTransport, identifier: &str) -> Result<IssueDetails, Error> {
    let request = details::request(identifier.to_owned());
    let response: GetIssueDetails = transport.execute(&request).await?;
    Ok(response.issue)
}
pub fn format(identifier: &str, title: &str, url: &str, references: bool) -> Vec<u8> {
    let magic = if references { "References" } else { "Fixes" };
    format!("{identifier} {title}\n\nLinear-issue: {magic} {identifier}\nLinear-issue-url: {url}\n")
        .into_bytes()
}
