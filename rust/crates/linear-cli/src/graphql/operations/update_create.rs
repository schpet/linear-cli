//! Exact status-update mutations and command-local initiative lookup documents.
use crate::graphql::operations::{
    initiatives::InitiativeUpdateHealthType, projects::ProjectUpdateHealthType,
};
use crate::graphql::{scalars::DateTime, schema};

#[derive(cynic::Enum, Clone, Copy, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "ProjectUpdateHealthType",
    rename_all = "camelCase"
)]
pub enum ProjectHealthInput {
    OnTrack,
    AtRisk,
    OffTrack,
}
#[derive(cynic::InputObject, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "ProjectUpdateCreateInput")]
pub struct ProjectInput {
    pub project_id: String,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub health: Option<ProjectHealthInput>,
}
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct ProjectVariables {
    pub input: ProjectInput,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "ProjectVariables"
)]
pub struct CreateProjectUpdate {
    #[arguments(input: $input)]
    pub project_update_create: ProjectPayload,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "ProjectUpdatePayload")]
pub struct ProjectPayload {
    pub success: bool,
    pub project_update: ProjectUpdate,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "ProjectUpdate")]
pub struct ProjectUpdate {
    pub id: cynic::Id,
    pub body: String,
    pub health: Option<ProjectUpdateHealthType>,
    pub url: String,
    pub created_at: DateTime,
    pub project: Option<ProjectParent>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Project")]
pub struct ProjectParent {
    pub name: String,
    pub slug_id: String,
}

#[derive(cynic::Enum, Clone, Copy, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "InitiativeUpdateHealthType",
    rename_all = "camelCase"
)]
pub enum InitiativeHealthInput {
    OnTrack,
    AtRisk,
    OffTrack,
}
#[derive(cynic::InputObject, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "InitiativeUpdateCreateInput")]
pub struct InitiativeInput {
    pub initiative_id: String,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub health: Option<InitiativeHealthInput>,
}
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct InitiativeVariables {
    pub input: InitiativeInput,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "InitiativeVariables"
)]
pub struct CreateInitiativeUpdate {
    #[arguments(input: $input)]
    pub initiative_update_create: InitiativePayload,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "InitiativeUpdatePayload")]
pub struct InitiativePayload {
    pub success: bool,
    pub initiative_update: InitiativeUpdate,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "InitiativeUpdate")]
pub struct InitiativeUpdate {
    pub id: cynic::Id,
    pub body: String,
    pub health: Option<InitiativeUpdateHealthType>,
    pub url: String,
    pub created_at: DateTime,
    pub initiative: Option<InitiativeParent>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct InitiativeParent {
    pub name: String,
    pub slug_id: String,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct SlugVariables {
    pub slug_id: String,
}
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct NameVariables {
    pub name: String,
}
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct IdVariables {
    pub id: String,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "SlugVariables")]
pub struct GetInitiativeBySlugForStatusUpdate {
    #[arguments(filter: { slugId: { eq: $slug_id } })]
    pub initiatives: SlugConnection,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "InitiativeConnection")]
pub struct SlugConnection {
    pub nodes: Vec<SlugNode>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct SlugNode {
    pub id: cynic::Id,
    pub slug_id: String,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "NameVariables")]
pub struct GetInitiativeByNameForStatusUpdate {
    #[arguments(filter: { name: { eqIgnoreCase: $name } })]
    pub initiatives: NameConnection,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "InitiativeConnection")]
pub struct NameConnection {
    pub nodes: Vec<NameNode>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct NameNode {
    pub id: cynic::Id,
    pub name: String,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "IdVariables")]
pub struct GetInitiativeNameForStatusUpdate {
    #[arguments(id: $id)]
    pub initiative: Option<InitiativeParent>,
}
