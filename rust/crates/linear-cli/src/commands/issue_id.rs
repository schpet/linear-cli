//! Resolve an issue identifier to its id, mapping a missing issue to "not found".
use crate::error::AppError;
use crate::graphql::envelope::{GraphQlRequest, is_not_found};
use crate::graphql::operations::issue_id::{GetIssueId, Variables};
use crate::graphql::transport::{GraphQlTransport, TransportFailure};
use cynic::QueryBuilder;

pub fn request(identifier: &str) -> GraphQlRequest<Variables> {
    GraphQlRequest::with_variables(GetIssueId::build(Variables {
        id: identifier.to_owned(),
    }))
}

pub fn lookup_error(failure: TransportFailure, identifier: &str) -> AppError {
    if let TransportFailure::GraphQl { errors, .. } = &failure
        && is_not_found(errors)
    {
        return AppError::not_found("Issue", identifier);
    }
    AppError::from(failure)
}

pub async fn fetch(transport: &GraphQlTransport, identifier: &str) -> Result<String, AppError> {
    let result: GetIssueId = transport
        .execute(&request(identifier))
        .await
        .map_err(|failure| lookup_error(failure, identifier))?;
    let id = result.issue.id.into_inner();
    // An empty id is treated as not found rather than passed to a second
    // lookup or a mutation.
    if id.is_empty() {
        return Err(AppError::not_found("Issue", identifier));
    }
    Ok(id)
}
