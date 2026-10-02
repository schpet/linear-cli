//! The `UpdateProjectMilestone` mutation and its selection.
use super::milestone_create::CreatedMilestoneProject;
use crate::graphql::operations::number::Float;
use crate::graphql::scalars::TimelessDate;
use crate::graphql::schema;

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
    pub project: CreatedMilestoneProject,
}
