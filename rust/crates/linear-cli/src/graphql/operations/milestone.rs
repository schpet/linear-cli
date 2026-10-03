//! Project milestone operations: list, details, create, update and delete.

use serde::Serialize;

use super::common::DeletePayload;
use super::project::ProjectRef;
use crate::graphql::pagination::PageInfo;
use crate::graphql::scalars::DateTime;
use crate::graphql::scalars::Float;
use crate::graphql::scalars::TimelessDate;
use crate::graphql::schema;

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct CreateProjectMilestoneVariables {
    pub input: ProjectMilestoneCreateInput,
}

/// Absent optionals are omitted; supplied strings, including empty ones, are
/// sent verbatim.
#[derive(cynic::InputObject, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "ProjectMilestoneCreateInput")]
pub struct ProjectMilestoneCreateInput {
    pub project_id: String,
    pub name: String,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub target_date: Option<TimelessDate>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "CreateProjectMilestoneVariables"
)]
pub struct CreateProjectMilestone {
    #[arguments(input: $input)]
    pub project_milestone_create: CreateProjectMilestonePayload,
}

/// `projectMilestone` is non-null in the schema; a null or missing entity is
/// a decode failure.
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "ProjectMilestonePayload")]
pub struct CreateProjectMilestonePayload {
    pub success: bool,
    pub project_milestone: CreatedMilestone,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "ProjectMilestone")]
pub struct CreatedMilestone {
    pub id: cynic::Id,
    pub name: String,
    pub target_date: Option<TimelessDate>,
    pub project: ProjectRef,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct DeleteProjectMilestoneVariables {
    pub id: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "DeleteProjectMilestoneVariables"
)]
pub struct DeleteProjectMilestone {
    #[arguments(id: $id)]
    pub project_milestone_delete: DeletePayload,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq)]
pub struct UpdateProjectMilestoneVariables {
    pub id: String,
    pub input: ProjectMilestoneUpdateInput,
}

/// Absent values are omitted; update never clears an existing field.
#[derive(cynic::InputObject, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "ProjectMilestoneUpdateInput")]
pub struct ProjectMilestoneUpdateInput {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub target_date: Option<TimelessDate>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub sort_order: Option<Float>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "UpdateProjectMilestoneVariables"
)]
pub struct UpdateProjectMilestone {
    #[arguments(id: $id, input: $input)]
    pub project_milestone_update: UpdateProjectMilestonePayload,
}

/// The schema's milestone and project are non-null: malformed success fails.
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "ProjectMilestonePayload")]
pub struct UpdateProjectMilestonePayload {
    pub success: bool,
    pub project_milestone: UpdatedMilestone,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "ProjectMilestone")]
pub struct UpdatedMilestone {
    pub id: cynic::Id,
    pub name: String,
    pub target_date: Option<TimelessDate>,
    pub sort_order: Float,
    pub project: ProjectRef,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct DetailVariables {
    pub id: String,
    pub first: i32,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "DetailVariables"
)]
pub struct GetMilestoneDetails {
    #[arguments(id: $id)]
    pub project_milestone: Option<DetailMilestone>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "ProjectMilestone",
    variables = "DetailVariables"
)]
pub struct DetailMilestone {
    pub id: cynic::Id,
    pub name: String,
    pub description: Option<String>,
    pub target_date: Option<TimelessDate>,
    pub sort_order: crate::graphql::scalars::Float,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub project: DetailProject,
    #[arguments(first: $first, after: $after)]
    pub issues: DetailIssues,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Project")]
pub struct DetailProject {
    pub id: cynic::Id,
    pub name: String,
    pub slug_id: String,
    pub url: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "IssueConnection")]
pub struct DetailIssues {
    pub nodes: Vec<DetailIssue>,
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "Issue")]
pub struct DetailIssue {
    pub id: cynic::Id,
    pub identifier: String,
    pub title: String,
    pub state: DetailState,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "WorkflowState")]
pub struct DetailState {
    pub name: String,
    #[cynic(rename = "type")]
    pub state_type: String,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct LookupVariables {
    pub project_id: String,
    pub name: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "LookupVariables"
)]
pub struct GetProjectMilestonesForLookup {
    #[arguments(id: $project_id)]
    pub project: Option<LookupProject>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Project",
    variables = "LookupVariables"
)]
pub struct LookupProject {
    #[arguments(filter: { name: { eqIgnoreCase: $name } })]
    pub project_milestones: Option<LookupConnection>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear", graphql_type = "ProjectMilestoneConnection")]
pub struct LookupConnection {
    pub nodes: Vec<LookupMilestone>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "ProjectMilestone")]
pub struct LookupMilestone {
    pub id: cynic::Id,
    pub name: String,
}

/// `$first` and `$after` are nullable; both are skipped from the variables
/// object when `None`, including `after` on the first page.
#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct GetProjectMilestonesVariables {
    pub project_id: String,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub first: Option<i32>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

/// `Query.project` is non-null in the schema, but a null root is reported as
/// "project not found", so only the root is optional here. Every nested non-null field stays strict.
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetProjectMilestonesVariables"
)]
pub struct GetProjectMilestones {
    #[arguments(id: $project_id)]
    pub project: Option<MilestonesProject>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(
    schema = "linear",
    graphql_type = "Project",
    variables = "GetProjectMilestonesVariables"
)]
pub struct MilestonesProject {
    pub id: cynic::Id,
    pub name: String,
    #[arguments(first: $first, after: $after)]
    pub project_milestones: ProjectMilestoneConnection,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq)]
#[cynic(schema = "linear")]
pub struct ProjectMilestoneConnection {
    pub nodes: Vec<ProjectMilestone>,
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq)]
#[cynic(schema = "linear")]
#[serde(rename_all = "camelCase")]
pub struct ProjectMilestone {
    pub id: cynic::Id,
    pub name: String,
    pub target_date: Option<TimelessDate>,
    pub sort_order: crate::graphql::scalars::Float,
    pub project: ProjectRef,
}
