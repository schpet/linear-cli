//! Project and initiative status updates: lists and create.

use super::initiative::InitiativeUpdateHealthType;
use super::project::ProjectUpdateHealthType;
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
    pub initiative_updates: InitiativeUpdateConnection,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "InitiativeUpdateConnection")]
pub struct InitiativeUpdateConnection {
    pub nodes: Vec<InitiativeUpdateNode>,
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "InitiativeUpdate")]
pub struct InitiativeUpdateNode {
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

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct ListProjectUpdatesVariables {
    pub id: String,
    pub first: Option<i32>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "ListProjectUpdatesVariables"
)]
pub struct ListProjectUpdates {
    #[arguments(id: $id)]
    pub project: Option<UpdateProject>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Project",
    variables = "ListProjectUpdatesVariables"
)]
pub struct UpdateProject {
    pub name: String,
    pub slug_id: String,
    #[arguments(first: $first, after: $after)]
    pub project_updates: ProjectUpdateConnection,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "ProjectUpdateConnection")]
pub struct ProjectUpdateConnection {
    pub nodes: Vec<ProjectUpdateNode>,
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "ProjectUpdate")]
pub struct ProjectUpdateNode {
    pub id: cynic::Id,
    pub body: String,
    pub health: Option<ProjectUpdateHealthType>,
    pub url: String,
    pub created_at: DateTime,
    pub user: Option<UpdateUser>,
}

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
    pub health: Option<ProjectUpdateHealthType>,
    pub url: String,
    pub project: Option<ProjectParent>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Project")]
pub struct ProjectParent {
    pub name: String,
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
    pub health: Option<InitiativeUpdateHealthType>,
    pub url: String,
    pub initiative: Option<InitiativeParent>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct InitiativeParent {
    pub name: String,
}
