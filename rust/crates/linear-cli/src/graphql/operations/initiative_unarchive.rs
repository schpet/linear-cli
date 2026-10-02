//! Archived-inclusive initiative lookups for `initiative unarchive` and `delete`.
use super::initiative_reference::{InitiativeNameConnection, NameVariablesFields};
use crate::graphql::scalars::DateTime;
use crate::graphql::schema;

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "NameVariables")]
pub struct ResolveInitiativeByNameIncludingArchived {
    #[arguments(filter: { name: { eqIgnoreCase: $name } }, includeArchived: true)]
    pub initiatives: InitiativeNameConnection,
}
#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct DetailVariables {
    pub id: cynic::Id,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "DetailVariables"
)]
pub struct GetInitiativeForUnarchive {
    #[arguments(filter: { id: { eq: $id } }, includeArchived: true)]
    pub initiatives: UnarchiveDetails,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "InitiativeConnection")]
pub struct UnarchiveDetails {
    pub nodes: Vec<UnarchiveDetail>,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct UnarchiveDetail {
    pub id: cynic::Id,
    pub slug_id: String,
    pub name: String,
    pub archived_at: Option<DateTime>,
}
#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct UnarchiveVariables {
    pub id: String,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "UnarchiveVariables"
)]
pub struct UnarchiveInitiative {
    #[arguments(id: $id)]
    pub initiative_unarchive: UnarchivePayload,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "InitiativeArchivePayload")]
pub struct UnarchivePayload {
    pub success: bool,
    pub entity: Option<UnarchivedInitiative>,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct UnarchivedInitiative {
    pub id: cynic::Id,
    pub slug_id: String,
    pub name: String,
    pub url: String,
}
