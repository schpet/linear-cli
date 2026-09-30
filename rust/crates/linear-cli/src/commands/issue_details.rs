//! Shared typed fetching for issue title and issue URL.
use crate::error::{AppError, AppErrorKind};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::issue_details::{GetIssueDetails, IssueDetails, Variables};
use crate::graphql::transport::GraphQlTransport;
use cynic::QueryBuilder;

pub fn request(id: String) -> GraphQlRequest<Variables> {
    GraphQlRequest::with_variables(GetIssueDetails::build(Variables { id }))
}
pub async fn fetch(transport: &GraphQlTransport, id: String) -> Result<IssueDetails, AppError> {
    let result: GetIssueDetails = transport
        .execute(&request(id))
        .await
        .map_err(AppError::from)?;
    Ok(result.issue)
}
pub fn unresolved(id_command: bool) -> AppError {
    AppError::new(AppErrorKind::Validation, "Could not determine issue ID").with_suggestion(
        if id_command {
            "Please provide an issue ID or run from a branch with an issue identifier."
        } else {
            "Please provide an issue ID like 'ENG-123'."
        },
    )
}
