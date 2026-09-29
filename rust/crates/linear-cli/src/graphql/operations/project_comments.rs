//! Typed root comments query for `project comment list`.

use crate::graphql::schema;

use super::comments::CommentConnection;

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct GetProjectCommentsVariables {
    pub id: String,
    pub filter_id: cynic::Id,
    pub after: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetProjectCommentsVariables"
)]
pub struct GetProjectComments {
    #[arguments(id: $id)]
    pub project: Option<CommentProject>,
    #[arguments(first: 50, after: $after, orderBy: createdAt, filter: { project: { id: { eq: $filter_id } } })]
    pub comments: CommentConnection,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Project")]
pub struct CommentProject {
    pub id: cynic::Id,
    pub name: String,
}
