//! The source `CreateProjectMilestone` mutation and its create-only selection.
use crate::graphql::scalars::TimelessDate;
use crate::graphql::schema;

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct CreateProjectMilestoneVariables {
    pub input: ProjectMilestoneCreateInput,
}

/// Deno builds `{ projectId, name, description, targetDate }` and drops
/// undefined keys, so absent optionals are omitted while supplied strings,
/// including empty ones, are sent verbatim.
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
/// a decode failure rather than the source's silent success.
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
    pub project: CreatedMilestoneProject,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Project")]
pub struct CreatedMilestoneProject {
    pub id: cynic::Id,
    pub name: String,
}
