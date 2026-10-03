//! Current fields and the mutation for `initiative update`.
use super::initiative_view::DetailVariablesFields;
use crate::graphql::{operations::initiatives::InitiativeStatus, scalars::TimelessDate, schema};
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "DetailVariables"
)]
pub struct GetInitiativeForUpdate {
    #[arguments(id: $id)]
    pub initiative: Option<CurrentInitiative>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct CurrentInitiative {
    pub name: String,
    pub description: Option<String>,
    pub status: Option<InitiativeStatus>,
    pub target_date: Option<TimelessDate>,
    pub color: Option<String>,
}
#[derive(cynic::InputObject, Clone, Debug, Default, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "InitiativeUpdateInput")]
pub struct InitiativeUpdateInput {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub status: Option<InitiativeStatus>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub target_date: Option<TimelessDate>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct UpdateVariables {
    pub id: String,
    pub input: InitiativeUpdateInput,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "UpdateVariables"
)]
pub struct UpdateInitiative {
    #[arguments(id: $id, input: $input)]
    pub initiative_update: UpdatedPayload,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "InitiativePayload")]
pub struct UpdatedPayload {
    pub success: bool,
    pub initiative: UpdatedInitiative,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct UpdatedInitiative {
    pub name: String,
    pub url: String,
}
