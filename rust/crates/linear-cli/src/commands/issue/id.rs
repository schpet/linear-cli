//! `issue id`, and resolving an issue identifier to its UUID.
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::{GraphQlRequest, is_not_found};
use crate::graphql::operations::issue_id::{GetIssueId, Variables};
use crate::graphql::transport::{GraphQlTransport, TransportFailure};
use cynic::QueryBuilder;

/// Prints the issue the current git branch or jj change names.
pub fn run(ctx: &Ctx) -> Result<()> {
    let identifier = super::infer(ctx)
        .and_then(|identifier| identifier.ok_or_else(|| super::unresolved(ctx)))
        .context("Failed to get issue ID")?;
    ctx.print(format!("{identifier}\n"))
}

pub fn request(identifier: &str) -> GraphQlRequest<Variables> {
    GraphQlRequest::with_variables(GetIssueId::build(Variables {
        id: identifier.to_owned(),
    }))
}

pub fn lookup_error(failure: TransportFailure, identifier: &str) -> Error {
    if let TransportFailure::GraphQl { errors, .. } = &failure
        && is_not_found(errors)
    {
        return Error::not_found("Issue", identifier);
    }
    Error::from(failure)
}

pub async fn fetch(transport: &GraphQlTransport, identifier: &str) -> Result<String, Error> {
    let result: GetIssueId = transport
        .execute(&request(identifier))
        .await
        .map_err(|failure| lookup_error(failure, identifier))?;
    let id = result.issue.id.into_inner();
    // An empty id is treated as not found rather than passed to a second
    // lookup or a mutation.
    if id.is_empty() {
        return Err(Error::not_found("Issue", identifier));
    }
    Ok(id)
}
