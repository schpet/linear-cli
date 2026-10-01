//! Source-shaped, typed C040 operations; raw status strings are an explicit update opt-in.
use super::initiative_view::{
    DetailVariablesFields, InitiativeNameResults, InitiativeSlugResults, NameVariablesFields,
    SlugVariablesFields,
};
use crate::graphql::{operations::initiatives::InitiativeStatus, scalars::TimelessDate, schema};
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "SlugVariables")]
pub struct GetInitiativeBySlug {
    #[arguments(filter: {slugId: {eq: $slug_id}})]
    pub initiatives: InitiativeSlugResults,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "NameVariables")]
pub struct GetInitiativeByName {
    #[arguments(filter: {name: {eqIgnoreCase: $name}})]
    pub initiatives: InitiativeNameResults,
}
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
    pub id: cynic::Id,
    pub slug_id: String,
    pub name: String,
    pub description: Option<String>,
    pub status: Option<InitiativeStatus>,
    pub target_date: Option<TimelessDate>,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub owner: Option<CurrentOwner>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "User")]
pub struct CurrentOwner {
    pub id: cynic::Id,
    pub display_name: String,
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
    pub id: cynic::Id,
    pub slug_id: String,
    pub name: String,
    pub url: String,
}
