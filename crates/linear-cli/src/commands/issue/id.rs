//! `issue id`, and resolving an issue identifier to its UUID.
use crate::client::LinearClient;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::common::IdVariables;
use crate::graphql::operations::issue::GetIssueId;

/// Prints the issue the current git branch or jj change names.
pub fn run(ctx: &Ctx) -> Result<()> {
    let identifier = super::infer(ctx)
        .and_then(|identifier| identifier.ok_or_else(|| super::unresolved(ctx)))
        .context("Failed to get issue ID")?;
    ctx.print(format!("{identifier}\n"))
}

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
