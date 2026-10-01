//! Full typed issue read and exact plaintext description.
use crate::{
    commands::issue_details,
    error::{AppError, AppErrorKind},
    graphql::{
        bulk_error::{self, ObservedExchangeFailure},
        operations::issue_details::{GetIssueDetails, IssueDetails},
        transport::GraphQlTransport,
    },
};
pub const CONTEXT: &str = "Failed to get issue description";
pub fn exchange_failure(failure: ObservedExchangeFailure) -> AppError {
    match failure {
        ObservedExchangeFailure::Strict(error) => error,
        ObservedExchangeFailure::Ordinary(error) => AppError::new(
            AppErrorKind::GraphQl,
            error.preferred_message.unwrap_or(error.message),
        ),
    }
}
pub async fn fetch(
    transport: &GraphQlTransport,
    identifier: &str,
) -> Result<IssueDetails, AppError> {
    let mut request = issue_details::request(identifier.to_owned());
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
