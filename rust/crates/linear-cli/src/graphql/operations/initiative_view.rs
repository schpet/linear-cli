//! Typed selections for `initiative view`.

use crate::graphql::operations::initiatives::{InitiativeStatus, InitiativeUpdateHealthType};
use crate::graphql::operations::projects::ProjectStatusType;
use crate::graphql::scalars::{DateTime, TimelessDate};
use crate::graphql::schema;

#[derive(cynic::QueryVariables, Clone, Debug, Eq, PartialEq)]
pub struct DetailVariables {
    pub id: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "DetailVariables"
)]
pub struct GetInitiativeDetails {
    // Handled as nullable even though the schema declares non-null.
    #[arguments(id: $id)]
    pub initiative: Option<InitiativeDetails>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct InitiativeDetails {
    pub id: cynic::Id,
    pub slug_id: String,
    pub name: String,
    pub description: Option<String>,
    pub status: InitiativeStatus,
    pub target_date: Option<TimelessDate>,
    pub health: Option<InitiativeUpdateHealthType>,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub url: String,
    pub archived_at: Option<DateTime>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub owner: Option<InitiativeViewOwner>,
    pub projects: InitiativeViewProjects,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "User")]
pub struct InitiativeViewOwner {
    pub id: cynic::Id,
    pub name: String,
    pub display_name: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "ProjectConnection")]
pub struct InitiativeViewProjects {
    pub nodes: Vec<InitiativeViewProject>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Project")]
pub struct InitiativeViewProject {
    pub id: cynic::Id,
    pub slug_id: String,
    pub name: String,
    pub status: InitiativeViewProjectStatus,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "ProjectStatus")]
pub struct InitiativeViewProjectStatus {
    pub name: String,
    #[cynic(rename = "type")]
    pub status_type: ProjectStatusType,
}

#[derive(cynic::QueryVariables, Clone, Debug, Eq, PartialEq)]
pub struct SlugVariables {
    pub slug_id: String,
}

#[derive(cynic::QueryVariables, Clone, Debug, Eq, PartialEq)]
pub struct NameVariables {
    pub name: String,
}

#[derive(cynic::QueryVariables, Clone, Debug, Eq, PartialEq)]
pub struct UrlSlugVariables {
    pub slug_id: String,
    pub include_archived: Option<bool>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "SlugVariables")]
pub struct GetInitiativeBySlugForView {
    #[arguments(filter: { slugId: { eq: $slug_id } })]
    pub initiatives: InitiativeSlugResults,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "InitiativeConnection")]
pub struct InitiativeSlugResults {
    pub nodes: Vec<InitiativeSlugNode>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct InitiativeSlugNode {
    pub id: cynic::Id,
    pub slug_id: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "NameVariables")]
pub struct GetInitiativeByNameForView {
    #[arguments(filter: { name: { eqIgnoreCase: $name } })]
    pub initiatives: InitiativeNameResults,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "InitiativeConnection")]
pub struct InitiativeNameResults {
    pub nodes: Vec<InitiativeNameNode>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct InitiativeNameNode {
    pub id: cynic::Id,
    pub name: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "UrlSlugVariables"
)]
pub struct ResolveInitiativeBySlug {
    #[arguments(filter: { slugId: { eq: $slug_id } }, includeArchived: $include_archived)]
    pub initiatives: InitiativeUrlResults,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "InitiativeConnection")]
pub struct InitiativeUrlResults {
    pub nodes: Vec<InitiativeUrlNode>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct InitiativeUrlNode {
    pub id: cynic::Id,
}
