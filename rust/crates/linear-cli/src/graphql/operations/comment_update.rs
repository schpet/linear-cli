//! Selections for issue comment update; no issue resolver or extra user fields.
use crate::graphql::{scalars::DateTime, schema};

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct GetCommentVariables {
    pub id: String,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetCommentVariables"
)]
pub struct GetComment {
    #[arguments(id: $id)]
    pub comment: Option<ExistingComment>,
}
#[derive(cynic::QueryFragment, serde::Deserialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Comment", no_deserialize)]
pub struct ExistingComment {
    // A missing or null body is treated as empty.
    #[serde(default)]
    pub body: Option<String>,
}
#[derive(cynic::InputObject, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "CommentUpdateInput")]
pub struct CommentUpdateInput {
    pub body: String,
}
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct UpdateCommentVariables {
    pub id: String,
    pub input: CommentUpdateInput,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "UpdateCommentVariables"
)]
pub struct UpdateComment {
    #[arguments(id: $id, input: $input)]
    pub comment_update: UpdatedPayload,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "CommentPayload")]
pub struct UpdatedPayload {
    pub success: bool,
    pub comment: Option<UpdatedComment>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Comment")]
pub struct UpdatedComment {
    pub id: cynic::Id,
    pub body: String,
    pub updated_at: DateTime,
    pub url: String,
    pub user: Option<UpdatedUser>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "User")]
pub struct UpdatedUser {
    pub name: String,
    pub display_name: String,
}
