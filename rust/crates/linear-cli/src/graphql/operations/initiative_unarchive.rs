use super::initiative_view::{NameVariablesFields, SlugVariablesFields};
// Exact archived lookups, confirmation details and unarchive selection.
pub use super::initiative_view::{
    InitiativeNameResults, InitiativeSlugResults, NameVariables, SlugVariables,
};
use crate::graphql::scalars::DateTime;
use crate::graphql::schema;

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "SlugVariables")]
pub struct GetInitiativeBySlugIncludeArchived {
    #[arguments(filter: { slugId: { eq: $slug_id } }, includeArchived: true)]
    pub initiatives: InitiativeSlugResults,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "NameVariables")]
pub struct GetInitiativeByNameIncludeArchived {
    #[arguments(filter: { name: { eqIgnoreCase: $name } }, includeArchived: true)]
    pub initiatives: InitiativeNameResults,
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
