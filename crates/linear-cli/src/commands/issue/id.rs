//! `issue id`: the issue IDs an argument names, and resolving an issue
//! identifier to its UUID.
use crate::cli::issue::IssueId;
use crate::client::LinearClient;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::common::IdVariables;
use crate::graphql::operations::issue::GetIssueId;

pub async fn fetch(client: &LinearClient, identifier: &str) -> Result<String, Error> {
    let result: GetIssueId = client
        .query(IdVariables {
            id: identifier.to_owned(),
        })
        .await
        .map_err(|failure| failure.or_not_found("Issue", identifier))?;
    let id = result.issue.id.into_inner();
    // An empty id is treated as not found rather than passed to a second
    // lookup or a mutation.
    if id.is_empty() {
        return Err(Error::not_found("Issue", identifier));
    }
    Ok(id)
}

/// Prints each issue ID `args` resolves to on its own line. Nothing is
/// fetched: an ID is printed as it would be sent.
pub fn run(ctx: &Ctx, args: &IssueId) -> Result<()> {
    let identifiers = super::reference::identifiers(ctx, &args.reference)
        .context("Failed to resolve the issue ID")?;
    let mut out = String::new();
    for identifier in identifiers {
        out.push_str(&identifier);
        out.push('\n');
    }
    ctx.print(out)
}
