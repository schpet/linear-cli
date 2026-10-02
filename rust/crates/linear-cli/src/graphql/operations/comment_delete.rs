//! The `DeleteComment` mutation and its `success`-only selection.
use crate::graphql::schema;

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct DeleteCommentVariables {
    pub id: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "DeleteCommentVariables"
)]
pub struct DeleteComment {
    #[arguments(id: $id)]
    pub comment_delete: DeleteCommentPayload,
}

/// `commentDelete` and `success` are non-null in the schema. A null payload
/// or missing `success` is a decode failure.
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "DeletePayload")]
pub struct DeleteCommentPayload {
    pub success: bool,
}
