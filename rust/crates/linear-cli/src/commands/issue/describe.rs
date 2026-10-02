//! `issue describe`: a commit message for an issue, with Linear trailers.
use crate::{
    cli::issue::IssueDescribe,
    commands::issue::details,
    ctx::Ctx,
    error::{Error, Result, ResultExt},
    graphql::{
        bulk_error::{self, ObservedExchangeFailure},
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
pub fn exchange_failure(failure: ObservedExchangeFailure) -> Error {
    match failure {
        ObservedExchangeFailure::Strict(error) => error,
        ObservedExchangeFailure::Ordinary(error) => {
            Error::new(error.preferred_message.unwrap_or(error.message))
        }
    }
}
pub async fn fetch(transport: &GraphQlTransport, identifier: &str) -> Result<IssueDetails, Error> {
    let mut request = details::request(identifier.to_owned());
    request.query = request.query.trim_end_matches('\n').to_owned();
    let response: GetIssueDetails = bulk_error::execute_observed(transport, &request)
        .await
        .map_err(exchange_failure)?;
    Ok(response.issue)
}
pub fn format(identifier: &str, title: &str, url: &str, references: bool) -> Vec<u8> {
    let magic = if references { "References" } else { "Fixes" };
    format!("{identifier} {title}\n\nLinear-issue: {magic} {identifier}\nLinear-issue-url: {url}\n")
        .into_bytes()
}
