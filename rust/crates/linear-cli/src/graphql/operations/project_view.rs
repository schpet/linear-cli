//! Project view and reference queries.

use serde::Serialize;

use crate::graphql::operations::number::Float;
use crate::graphql::operations::projects::{ProjectFilter, ProjectUpdateHealthType};
use crate::graphql::pagination::PageInfo;
use crate::graphql::scalars::{DateTime, TimelessDate};
use crate::graphql::schema;

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
    pub creator: Option<ViewUser>,
    pub lead: Option<ViewUser>,
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
    pub status_type: crate::graphql::operations::projects::ProjectStatusType,
    pub position: Float,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "User")]
#[serde(rename_all = "camelCase")]
pub struct ViewUser {
    pub id: cynic::Id,
    pub name: String,
    pub display_name: String,
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

connection!(ViewTeams, "TeamConnection", ViewTeam);
connection!(ViewLabels, "ProjectLabelConnection", ViewLabel);
connection!(ViewMembers, "UserConnection", ViewUser);
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
#[cynic(schema = "linear", graphql_type = "Team")]
pub struct ViewTeam {
    pub id: cynic::Id,
    pub key: String,
    pub name: String,
}
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
    pub user: Option<ViewUser>,
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
    pub nodes: Vec<PickerTeam>,
}
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Team")]
pub struct PickerTeam {
    pub key: String,
}
