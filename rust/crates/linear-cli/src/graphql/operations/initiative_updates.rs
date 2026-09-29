//! One page of status updates for a resolved initiative.

use crate::graphql::operations::initiatives::InitiativeUpdateHealthType;
use crate::graphql::scalars::DateTime;
use crate::graphql::schema;

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct ListInitiativeUpdatesVariables {
    pub id: String,
    pub first: Option<i32>,
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
    #[arguments(first: $first)]
    pub initiative_updates: UpdateConnection,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "InitiativeUpdateConnection")]
pub struct UpdateConnection {
    pub nodes: Vec<UpdateNode>,
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
}
