//! Typed project create/update and membership operations.
use crate::graphql::{
    edit::Edit,
    scalars::{DateTime, TimelessDate},
    schema,
};
use serde::Serialize;

#[derive(cynic::InputObject, Clone, Debug, Default, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "ProjectCreateInput")]
pub struct ProjectCreateInput {
    pub name: String,
    pub team_ids: Vec<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub lead_id: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub status_id: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub start_date: Option<TimelessDate>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub target_date: Option<TimelessDate>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub label_ids: Option<Vec<String>>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub member_ids: Option<Vec<String>>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub template_id: Option<String>,
}
#[derive(cynic::InputObject, Clone, Debug, Default, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "ProjectUpdateInput")]
pub struct ProjectUpdateInput {
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub name: Edit<String>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub description: Edit<String>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub content: Edit<String>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub status_id: Edit<String>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub lead_id: Edit<String>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub start_date: Edit<TimelessDate>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub target_date: Edit<TimelessDate>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub team_ids: Option<Vec<String>>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub label_ids: Option<Vec<String>>,
}
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct CreateProjectVariables {
    pub input: ProjectCreateInput,
}
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct UpdateProjectVariables {
    pub id: String,
    pub input: ProjectUpdateInput,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "CreateProjectVariables"
)]
pub struct CreateProject {
    #[arguments(input:$input)]
    pub project_create: CreatedProjectPayload,
}
#[derive(cynic::QueryFragment, Serialize, Debug)]
#[cynic(schema = "linear", graphql_type = "ProjectPayload")]
pub struct CreatedProjectPayload {
    pub success: bool,
    pub project: Option<CreatedProject>,
}
#[derive(cynic::QueryFragment, Serialize, Debug)]
#[cynic(schema = "linear", graphql_type = "Project")]
#[serde(rename_all = "camelCase")]
pub struct CreatedProject {
    pub id: cynic::Id,
    pub slug_id: String,
    pub name: String,
    pub url: String,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "UpdateProjectVariables"
)]
pub struct UpdateProject {
    #[arguments(id:$id,input:$input)]
    pub project_update: UpdatedProjectPayload,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "ProjectPayload")]
pub struct UpdatedProjectPayload {
    pub success: bool,
    pub project: Option<UpdatedProject>,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Project")]
pub struct UpdatedProject {
    pub id: cynic::Id,
    pub slug_id: String,
    pub name: String,
    pub description: Option<String>,
    pub url: String,
    pub updated_at: DateTime,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Query")]
pub struct GetProjectStatuses {
    pub project_statuses: ProjectStatuses,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "ProjectStatusConnection")]
pub struct ProjectStatuses {
    pub nodes: Vec<ProjectStatus>,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "ProjectStatus")]
pub struct ProjectStatus {
    pub id: cynic::Id,
    pub name: String,
    #[cynic(rename = "type")]
    pub status_type: super::projects::ProjectStatusType,
}
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct NameVariables {
    pub name: String,
}
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct SlugVariables {
    pub slug_id: String,
}
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct InitiativeIdVariables {
    pub id: cynic::Id,
}
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct IdVariables {
    pub id: String,
}
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct PageVariables {
    pub id: String,
    pub after: Option<String>,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "NameVariables")]
pub struct GetProjectLabelIdByName {
    #[arguments(filter:{name:{eqIgnoreCase:$name},isGroup:{eq:false}})]
    pub project_labels: ProjectLabels,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "ProjectLabelConnection")]
pub struct ProjectLabels {
    pub nodes: Vec<ProjectLabel>,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "ProjectLabel")]
pub struct ProjectLabel {
    pub id: cynic::Id,
    pub name: String,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "SlugVariables")]
pub struct GetInitiativeBySlugForCreate {
    #[arguments(filter:{slugId:{eq:$slug_id}})]
    pub initiatives: InitiativeSlugs,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "InitiativeConnection")]
pub struct InitiativeSlugs {
    pub nodes: Vec<InitiativeSlug>,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct InitiativeSlug {
    pub id: cynic::Id,
    pub slug_id: String,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "NameVariables")]
pub struct GetInitiativeByNameForCreate {
    #[arguments(filter:{name:{eqIgnoreCase:$name}})]
    pub initiatives: InitiativeNames,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "InitiativeIdVariables"
)]
pub struct GetInitiativeByIdForUpdate {
    #[arguments(filter:{id:{eq:$id}})]
    pub initiatives: InitiativeNames,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "InitiativeConnection")]
pub struct InitiativeNames {
    pub nodes: Vec<InitiativeName>,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct InitiativeName {
    pub id: cynic::Id,
    pub name: String,
}
#[derive(cynic::InputObject, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "InitiativeToProjectCreateInput")]
pub struct InitiativeLinkInput {
    pub initiative_id: String,
    pub project_id: String,
}
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct LinkVariables {
    pub input: InitiativeLinkInput,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "LinkVariables"
)]
pub struct AddProjectToInitiativeForCreate {
    #[arguments(input:$input)]
    pub initiative_to_project_create: LinkCreated,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "LinkVariables"
)]
pub struct AddProjectToInitiativeForUpdate {
    #[arguments(input:$input)]
    pub initiative_to_project_create: LinkCreated,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "IdVariables"
)]
pub struct RemoveProjectFromInitiativeForUpdate {
    #[arguments(id:$id)]
    pub initiative_to_project_delete: LinkDeleted,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "InitiativeToProjectPayload")]
pub struct LinkCreated {
    pub success: bool,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "DeletePayload")]
pub struct LinkDeleted {
    pub success: bool,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "PageVariables")]
pub struct GetProjectTeamsForUpdate {
    #[arguments(id:$id)]
    pub project: ProjectTeams,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Project",
    variables = "PageVariables"
)]
pub struct ProjectTeams {
    #[arguments(first:250,after:$after)]
    pub teams: TeamPage,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "TeamConnection")]
pub struct TeamPage {
    pub nodes: Vec<ProjectTeam>,
    pub page_info: super::teams::PageInfo,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "PageVariables")]
pub struct GetProjectLabelsForUpdate {
    #[arguments(id:$id)]
    pub project: ProjectLabelsPage,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Project",
    variables = "PageVariables"
)]
pub struct ProjectLabelsPage {
    #[arguments(first:250,after:$after)]
    pub labels: LabelPage,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "ProjectLabelConnection")]
pub struct LabelPage {
    pub nodes: Vec<ProjectLabel>,
    pub page_info: super::teams::PageInfo,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "PageVariables")]
pub struct GetProjectInitiativeLinksForUpdate {
    #[arguments(id:$id)]
    pub project: ProjectLinks,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Project",
    variables = "PageVariables"
)]
pub struct ProjectLinks {
    pub id: cynic::Id,
    pub name: String,
    pub url: String,
    #[arguments(first:250,after:$after)]
    pub initiative_to_projects: LinkPage,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "InitiativeToProjectConnection")]
pub struct LinkPage {
    pub nodes: Vec<LinkRow>,
    pub page_info: super::teams::PageInfo,
}
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "InitiativeToProject")]
pub struct LinkRow {
    pub id: cynic::Id,
    pub initiative: InitiativeName,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Team")]
pub struct ProjectTeam {
    pub id: cynic::Id,
    pub key: String,
    pub name: String,
}
