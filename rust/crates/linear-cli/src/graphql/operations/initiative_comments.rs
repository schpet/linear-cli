//! Typed root comments query for `initiative comment list`.

use crate::graphql::schema;

use super::comments::CommentConnection;

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct GetInitiativeCommentsVariables {
    pub id: String,
    pub filter_id: cynic::Id,
    pub after: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetInitiativeCommentsVariables"
)]
pub struct GetInitiativeComments {
    #[arguments(id: $id)]
    pub initiative: Option<CommentInitiative>,
    #[arguments(first: 50, after: $after, orderBy: createdAt, filter: { initiative: { id: { eq: $filter_id } } })]
    pub comments: CommentConnection,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct CommentInitiative {
    pub id: cynic::Id,
    pub name: String,
}
