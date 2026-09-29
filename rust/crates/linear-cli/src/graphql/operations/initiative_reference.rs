//! Exact-name selection for the strict shared initiative resolver.
pub use super::initiative_view::{ResolveInitiativeBySlug, UrlSlugVariables};
use crate::graphql::schema;
#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct NameVariables {
    pub name: String,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "NameVariables")]
pub struct ResolveInitiativeByName {
    #[arguments(filter: { name: { eqIgnoreCase: $name } })]
    pub initiatives: InitiativeNameConnection,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "InitiativeConnection")]
pub struct InitiativeNameConnection {
    pub nodes: Vec<InitiativeName>,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct InitiativeName {
    pub id: cynic::Id,
    pub name: String,
    pub slug_id: String,
}
