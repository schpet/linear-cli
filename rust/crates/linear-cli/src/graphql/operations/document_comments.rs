//! Typed nested document comment connection.
use super::comments::CommentConnection;
use crate::graphql::schema;
#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct GetDocumentCommentsVariables {
    pub id: String,
    pub after: Option<String>,
    pub first: i32,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetDocumentCommentsVariables"
)]
pub struct GetDocumentComments {
    #[arguments(id: $id)]
    pub document: Option<CommentDocument>,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Document",
    variables = "GetDocumentCommentsVariables"
)]
pub struct CommentDocument {
    pub id: cynic::Id,
    #[arguments(first: $first, after: $after, orderBy: createdAt)]
    pub comments: CommentConnection,
}
