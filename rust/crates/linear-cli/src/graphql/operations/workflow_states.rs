//! The single, unpaginated workflow-state connection used by `team states`.
//! State types remain strings because Linear can add new lifecycle groups.

use serde::Serialize;

use crate::graphql::schema;

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct GetWorkflowStatesVariables {
    pub team_key: String,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetWorkflowStatesVariables"
)]
pub struct GetWorkflowStates {
    #[arguments(id: $team_key)]
    pub team: WorkflowTeam,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Team")]
pub struct WorkflowTeam {
    pub states: WorkflowStateConnection,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear")]
pub struct WorkflowStateConnection {
    pub nodes: Vec<WorkflowState>,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq)]
#[cynic(schema = "linear")]
pub struct WorkflowState {
    pub id: cynic::Id,
    pub name: String,
    #[cynic(rename = "type")]
    #[serde(rename = "type")]
    pub state_type: String,
    pub position: f64,
}
