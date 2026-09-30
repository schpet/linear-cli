//! Typed nested issue comment connection.
use super::comments::CommentConnection;
use crate::graphql::schema;
#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct GetIssueCommentsVariables {
    pub id: String,
    pub after: Option<String>,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetIssueCommentsVariables"
)]
pub struct GetIssueComments {
    #[arguments(id: $id)]
    pub issue: Option<CommentIssue>,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Issue",
    variables = "GetIssueCommentsVariables"
)]
pub struct CommentIssue {
    #[arguments(first: 50, after: $after, orderBy: createdAt)]
    pub comments: CommentConnection,
}
