//! Resolving an issue identifier to its UUID.
use crate::client::LinearClient;
use crate::error::{Error, Result};
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
