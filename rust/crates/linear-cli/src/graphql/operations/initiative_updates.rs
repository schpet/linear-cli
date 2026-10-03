//! Status updates of a resolved initiative, newest first.

use crate::graphql::operations::initiatives::InitiativeUpdateHealthType;
use crate::graphql::pagination::PageInfo;
use crate::graphql::scalars::DateTime;
use crate::graphql::schema;

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct ListInitiativeUpdatesVariables {
    pub id: String,
    pub first: Option<i32>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "ListInitiativeUpdatesVariables"
)]
pub struct ListInitiativeUpdates {
    #[arguments(id: $id)]
    pub initiative: Option<UpdateInitiative>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Initiative",
    variables = "ListInitiativeUpdatesVariables"
)]
pub struct UpdateInitiative {
    pub name: String,
    pub slug_id: String,
    #[arguments(first: $first, after: $after)]
    pub initiative_updates: UpdateConnection,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "InitiativeUpdateConnection")]
pub struct UpdateConnection {
    pub nodes: Vec<UpdateNode>,
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "InitiativeUpdate")]
pub struct UpdateNode {
    pub id: cynic::Id,
    pub body: String,
    pub health: InitiativeUpdateHealthType,
    pub url: String,
    pub created_at: DateTime,
    pub user: Option<UpdateUser>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "User")]
pub struct UpdateUser {
    pub name: String,
    pub display_name: String,
}
