//! Project operations: lists, details, create, update and delete.

use serde::Serialize;

use super::common::DeletePayload;
use super::common::IdVariablesFields;
use super::common::NameVariablesFields;
use super::team::StringComparator;
use super::team::TeamFilter;
use super::team::TeamKey;
use super::team::TeamRef;
use super::user::UserRef;
use crate::graphql::edit::Edit;
use crate::graphql::pagination::PageInfo;
use crate::graphql::scalars::DateTime;
use crate::graphql::scalars::Float;
use crate::graphql::scalars::TimelessDate;
use crate::graphql::schema;

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct DeleteProjectVariables {
    pub id: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "DeleteProjectVariables"
)]
pub struct DeleteProject {
    #[arguments(id: $id)]
    pub project_delete: ProjectDeletePayload,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "ProjectArchivePayload")]
pub struct ProjectDeletePayload {
    pub success: bool,
    pub entity: Option<ProjectRef>,
}

#[derive(cynic::QueryVariables, Clone, Debug, Eq, PartialEq)]
pub struct ProjectReferenceVariables {
    pub name: String,
}

#[derive(cynic::QueryVariables, Clone, Debug, Eq, PartialEq)]
pub struct ProjectSlugVariables {
    pub slug_id: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "ProjectReferenceVariables"
)]
pub struct GetProjectIdByName {
    #[arguments(filter: { name: { eq: $name } })]
    pub projects: ProjectIds,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "ProjectSlugVariables"
)]
pub struct GetProjectIdBySlugId {
    #[arguments(filter: { slugId: { eq: $slug_id } })]
    pub projects: ProjectIds,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "ProjectConnection")]
pub struct ProjectIds {
    pub nodes: Vec<ProjectId>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Project")]
pub struct ProjectId {
    pub id: cynic::Id,
}

#[derive(cynic::QueryVariables, Clone, Debug, Eq, PartialEq)]
pub struct ProjectDetailsVariables {
    pub id: String,
    pub first: i32,
}

#[derive(cynic::QueryVariables, Clone, Debug, Eq, PartialEq)]
pub struct ProjectIssuesVariables {
    pub id: String,
    pub first: i32,
    pub after: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "ProjectDetailsVariables"
)]
pub struct GetProjectDetails {
    #[arguments(id: $id)]
    pub project: Option<ProjectDetails>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "ProjectIssuesVariables"
)]
pub struct GetProjectIssuesPage {
    #[arguments(id: $id)]
    pub project: Option<ProjectIssuesPage>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Project",
    variables = "ProjectIssuesVariables"
)]
pub struct ProjectIssuesPage {
    pub id: cynic::Id,
    #[arguments(first: $first, after: $after)]
    pub issues: IssueConnection,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Project",
    variables = "ProjectDetailsVariables"
)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetails {
    pub id: cynic::Id,
    pub name: String,
    pub identifier: Option<String>,
    pub description: String,
    pub content: Option<String>,
    pub slug_id: String,
    pub icon: Option<String>,
    pub color: String,
    pub progress: Float,
    pub scope: Float,
    pub url: String,
    pub priority: i32,
    pub health: Option<ProjectUpdateHealthType>,
    pub health_updated_at: Option<DateTime>,
    pub start_date: Option<TimelessDate>,
    pub start_date_resolution: Option<DateResolutionType>,
    pub target_date: Option<TimelessDate>,
    pub target_date_resolution: Option<DateResolutionType>,
    pub started_at: Option<DateTime>,
    pub completed_at: Option<DateTime>,
    pub canceled_at: Option<DateTime>,
    pub archived_at: Option<DateTime>,
    pub auto_archived_at: Option<DateTime>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub status: ViewStatus,
    pub creator: Option<UserRef>,
    pub lead: Option<UserRef>,
    #[arguments(first: $first)]
    pub teams: ViewTeams,
    #[arguments(first: $first)]
    pub labels: ViewLabels,
    #[arguments(first: $first)]
    pub members: ViewMembers,
    #[arguments(first: $first)]
    pub initiatives: ViewInitiatives,
    #[arguments(first: $first)]
    pub project_milestones: ViewMilestones,
    #[arguments(first: $first)]
    pub external_links: ViewExternalLinks,
    #[arguments(first: $first)]
    pub documents: ViewDocuments,
    #[arguments(first: $first)]
    pub attachments: ViewAttachments,
    #[arguments(first: $first)]
    pub relations: ViewRelations,
    #[arguments(first: $first)]
    pub inverse_relations: ViewInverseRelations,
    #[arguments(first: $first)]
    pub issues: IssueConnection,
    pub last_update: Option<ViewUpdate>,
}

#[derive(cynic::Enum, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", rename_all = "camelCase")]
pub enum DateResolutionType {
    HalfYear,
    Month,
    Quarter,
    Year,
    #[cynic(fallback)]
    Unknown(String),
}

#[derive(cynic::Enum, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", rename_all = "camelCase")]
pub enum ProjectMilestoneStatus {
    Done,
    Next,
    Overdue,
    Unstarted,
    #[cynic(fallback)]
    Unknown(String),
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "ProjectStatus")]
#[serde(rename_all = "camelCase")]
pub struct ViewStatus {
    pub id: cynic::Id,
    pub name: String,
    pub color: String,
    #[cynic(rename = "type")]
    #[serde(rename = "type")]
    pub status_type: ProjectStatusType,
    pub position: Float,
}

macro_rules! connection {
    ($name:ident, $graphql:literal, $node:ty) => {
        #[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq)]
        #[cynic(schema = "linear", graphql_type = $graphql)]
        #[serde(transparent)]
        pub struct $name {
            pub nodes: Vec<$node>,
            #[serde(skip)]
            pub page_info: PageInfo,
        }
    };
}

connection!(ViewTeams, "TeamConnection", TeamRef);
connection!(ViewLabels, "ProjectLabelConnection", ViewLabel);
connection!(ViewMembers, "UserConnection", UserRef);
connection!(ViewInitiatives, "InitiativeConnection", ViewInitiative);
connection!(ViewMilestones, "ProjectMilestoneConnection", ViewMilestone);
connection!(
    ViewExternalLinks,
    "EntityExternalLinkConnection",
    ViewExternalLink
);
connection!(ViewDocuments, "DocumentConnection", ViewDocument);
connection!(
    ViewAttachments,
    "ProjectAttachmentConnection",
    ViewAttachment
);
connection!(ViewRelations, "ProjectRelationConnection", ViewRelation);
connection!(
    ViewInverseRelations,
    "ProjectRelationConnection",
    ViewInverseRelation
);
connection!(IssueConnection, "IssueConnection", ViewIssue);

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "ProjectLabel")]
pub struct ViewLabel {
    pub id: cynic::Id,
    pub name: String,
    pub color: String,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Initiative")]
pub struct ViewInitiative {
    pub id: cynic::Id,
    pub name: String,
    pub url: String,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "ProjectMilestone")]
#[serde(rename_all = "camelCase")]
pub struct ViewMilestone {
    pub id: cynic::Id,
    pub name: String,
    pub description: Option<String>,
    pub target_date: Option<TimelessDate>,
    pub progress: Float,
    pub status: ProjectMilestoneStatus,
    pub sort_order: Float,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "EntityExternalLink")]
#[serde(rename_all = "camelCase")]
pub struct ViewExternalLink {
    pub id: cynic::Id,
    pub label: String,
    pub url: String,
    pub sort_order: Float,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Document")]
#[serde(rename_all = "camelCase")]
pub struct ViewDocument {
    pub id: cynic::Id,
    pub title: String,
    pub url: String,
    pub sort_order: Float,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "ProjectAttachment")]
#[serde(rename_all = "camelCase")]
pub struct ViewAttachment {
    pub id: cynic::Id,
    pub title: String,
    pub subtitle: Option<String>,
    pub url: String,
    pub source_type: Option<String>,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "ProjectRelation")]
#[serde(rename_all = "camelCase")]
pub struct ViewRelation {
    pub id: cynic::Id,
    #[cynic(rename = "type")]
    #[serde(rename = "type")]
    pub relation_type: String,
    pub anchor_type: String,
    pub related_anchor_type: String,
    pub project_milestone: Option<ViewMilestoneRef>,
    pub related_project: ViewProjectRef,
    pub related_project_milestone: Option<ViewMilestoneRef>,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "ProjectRelation")]
#[serde(rename_all = "camelCase")]
pub struct ViewInverseRelation {
    pub id: cynic::Id,
    #[cynic(rename = "type")]
    #[serde(rename = "type")]
    pub relation_type: String,
    pub anchor_type: String,
    pub related_anchor_type: String,
    pub project_milestone: Option<ViewMilestoneRef>,
    pub project: ViewProjectRef,
    pub related_project_milestone: Option<ViewMilestoneRef>,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "ProjectMilestone")]
pub struct ViewMilestoneRef {
    pub id: cynic::Id,
    pub name: String,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Project")]
pub struct ViewProjectRef {
    pub id: cynic::Id,
    pub name: String,
    pub url: String,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Issue")]
pub struct ViewIssue {
    pub id: cynic::Id,
    pub identifier: String,
    pub title: String,
    pub state: ViewIssueState,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "WorkflowState")]
pub struct ViewIssueState {
    pub id: cynic::Id,
    pub name: String,
    #[cynic(rename = "type")]
    #[serde(rename = "type")]
    pub state_type: String,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "ProjectUpdate")]
#[serde(rename_all = "camelCase")]
pub struct ViewUpdate {
    pub id: cynic::Id,
    pub body: String,
    pub health: Option<ProjectUpdateHealthType>,
    pub created_at: DateTime,
    pub user: Option<UserRef>,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct PickerVariables {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub filter: Option<ProjectFilter>,
    pub first: i32,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "PickerVariables"
)]
pub struct GetProjectsForPicker {
    #[arguments(filter: $filter, first: $first, after: $after)]
    pub projects: PickerConnection,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "ProjectConnection")]
pub struct PickerConnection {
    pub nodes: Vec<PickerProject>,
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Project")]
pub struct PickerProject {
    pub id: cynic::Id,
    pub name: String,
    pub slug_id: String,
    pub sort_order: Float,
    pub status: PickerStatus,
    #[arguments(first: 10)]
    pub teams: PickerTeams,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "ProjectStatus")]
pub struct PickerStatus {
    pub name: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "TeamConnection")]
pub struct PickerTeams {
    pub nodes: Vec<TeamKey>,
    pub page_info: PageInfo,
}

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
    pub priority: Option<i32>,
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
    pub name: String,
    pub url: String,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Query")]
pub struct GetProjectStatuses {
    pub project_statuses: StatusOptions,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "ProjectStatusConnection")]
pub struct StatusOptions {
    pub nodes: Vec<StatusOption>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "ProjectStatus")]
pub struct StatusOption {
    pub id: cynic::Id,
    pub name: String,
    #[cynic(rename = "type")]
    pub status_type: ProjectStatusType,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct InitiativeIdVariables {
    pub id: cynic::Id,
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
pub struct AddProjectToInitiative {
    #[arguments(input:$input)]
    pub initiative_to_project_create: LinkCreated,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "IdVariables"
)]
pub struct RemoveProjectFromInitiative {
    #[arguments(id:$id)]
    pub initiative_to_project_delete: DeletePayload,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "InitiativeToProjectPayload")]
pub struct LinkCreated {
    pub success: bool,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "PageVariables")]
pub struct GetProjectTeamsForUpdate {
    #[arguments(id:$id)]
    pub project: ProjectTeamPage,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Project",
    variables = "PageVariables"
)]
pub struct ProjectTeamPage {
    #[arguments(first:250,after:$after)]
    pub teams: TeamPage,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "TeamConnection")]
pub struct TeamPage {
    pub nodes: Vec<TeamRef>,
    pub page_info: crate::graphql::pagination::PageInfo,
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
    pub page_info: crate::graphql::pagination::PageInfo,
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
    pub name: String,
    pub url: String,
    #[arguments(first:250,after:$after)]
    pub initiative_to_projects: LinkPage,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "InitiativeToProjectConnection")]
pub struct LinkPage {
    pub nodes: Vec<LinkRow>,
    pub page_info: crate::graphql::pagination::PageInfo,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "linear", graphql_type = "InitiativeToProject")]
pub struct LinkRow {
    pub id: cynic::Id,
    pub initiative: InitiativeName,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct GetProjectsVariables {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub filter: Option<ProjectFilter>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub first: Option<i32>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

#[derive(cynic::InputObject, Clone, Debug, Default, PartialEq, Eq)]
#[cynic(schema = "linear")]
pub struct ProjectFilter {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub accessible_teams: Option<TeamCollectionFilter>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub status: Option<ProjectStatusFilter>,
}

#[derive(cynic::InputObject, Clone, Debug, Default, PartialEq, Eq)]
#[cynic(schema = "linear")]
pub struct TeamCollectionFilter {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub some: Option<TeamFilter>,
}

#[derive(cynic::InputObject, Clone, Debug, Default, PartialEq, Eq)]
#[cynic(schema = "linear")]
pub struct ProjectStatusFilter {
    #[cynic(rename = "type", skip_serializing_if = "Option::is_none")]
    pub status_type: Option<StringComparator>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetProjectsVariables"
)]
pub struct GetProjects {
    #[arguments(filter: $filter, first: $first, after: $after)]
    pub projects: ProjectConnection,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear")]
pub struct ProjectConnection {
    pub nodes: Vec<Project>,
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear")]
pub struct Project {
    pub id: cynic::Id,
    pub name: String,
    pub description: String,
    pub slug_id: String,
    pub icon: Option<String>,
    pub color: String,
    pub sort_order: crate::graphql::scalars::Float,
    pub status: ProjectStatus,
    pub lead: Option<ProjectLead>,
    pub priority: i32,
    pub health: Option<ProjectUpdateHealthType>,
    pub start_date: Option<TimelessDate>,
    pub target_date: Option<TimelessDate>,
    pub started_at: Option<DateTime>,
    pub completed_at: Option<DateTime>,
    pub canceled_at: Option<DateTime>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub url: String,
    pub teams: ProjectTeams,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear")]
#[serde(rename_all = "camelCase")]
pub struct ProjectStatus {
    pub id: cynic::Id,
    pub name: String,
    pub color: String,
    #[cynic(rename = "type")]
    #[serde(rename = "type")]
    pub status_type: ProjectStatusType,
}

#[derive(cynic::Enum, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", rename_all = "camelCase", non_exhaustive)]
pub enum ProjectStatusType {
    Backlog,
    Canceled,
    Completed,
    Paused,
    Planned,
    Started,
    #[cynic(fallback)]
    Unknown(String),
}

impl ProjectStatusType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Backlog => "backlog",
            Self::Canceled => "canceled",
            Self::Completed => "completed",
            Self::Paused => "paused",
            Self::Planned => "planned",
            Self::Started => "started",
            Self::Unknown(value) => value,
        }
    }
}

#[derive(cynic::Enum, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", rename_all = "camelCase", non_exhaustive)]
pub enum ProjectUpdateHealthType {
    AtRisk,
    OffTrack,
    OnTrack,
    #[cynic(fallback)]
    Unknown(String),
}

impl ProjectUpdateHealthType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::AtRisk => "atRisk",
            Self::OffTrack => "offTrack",
            Self::OnTrack => "onTrack",
            Self::Unknown(value) => value,
        }
    }
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "User")]
#[serde(rename_all = "camelCase")]
pub struct ProjectLead {
    pub name: String,
    pub display_name: String,
    pub initials: String,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "TeamConnection")]
#[serde(transparent)]
pub struct ProjectTeams {
    pub nodes: Vec<TeamKey>,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Project")]
#[serde(rename_all = "camelCase")]
pub struct ProjectRef {
    pub id: cynic::Id,
    pub name: String,
}

/// A project offered in a picker: its slug ID tells apart projects that
/// share a name.
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Project")]
pub struct ProjectChoice {
    pub id: cynic::Id,
    pub name: String,
    pub slug_id: String,
}

/// A project's name, to name it in a confirmation.
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "DeleteProjectVariables"
)]
pub struct GetProjectName {
    #[arguments(id: $id)]
    pub project: ProjectRef,
}
