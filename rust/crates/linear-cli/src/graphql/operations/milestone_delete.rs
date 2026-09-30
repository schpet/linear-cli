//! Milestone deletion sends the original positional directly, without lookup.
use crate::graphql::schema;

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
    pub project_milestone_delete: MilestoneDeletePayload,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "DeletePayload")]
pub struct MilestoneDeletePayload {
    pub success: bool,
}
