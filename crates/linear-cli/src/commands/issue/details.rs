//! An issue's details, for the commands that print or build on them.
use crate::client::LinearClient;
use crate::error::Result;
use crate::graphql::operations::common::IdVariables;
use crate::graphql::operations::issue::{
    GetIssueDetails, GetIssueTransitionDetails, IssueDetails, IssueTransitionDetails,
};

pub async fn fetch(client: &LinearClient, id: String) -> Result<IssueDetails> {
    let result: GetIssueDetails = client
        .query(IdVariables { id: id.clone() })
        .await
        .map_err(|failure| failure.or_not_found("Issue", &id))?;
    Ok(result.issue)
}

/// The issue's current status, team, and the names a branch or change for it
/// is built from.
pub async fn fetch_for_transition(
    client: &LinearClient,
    id: String,
) -> Result<IssueTransitionDetails> {
    let result: GetIssueTransitionDetails = client
        .query(IdVariables { id: id.clone() })
        .await
        .map_err(|failure| failure.or_not_found("Issue", &id))?;
    Ok(result.issue)
}
