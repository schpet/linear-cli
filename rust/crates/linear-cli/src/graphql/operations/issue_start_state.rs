//! Exact state-only mutation used after successful local start work.
use crate::graphql::schema;
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct Variables {
    pub issue_id: String,
    pub state_id: String,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Mutation", variables = "Variables")]
pub struct UpdateIssueState {
    #[arguments(id: $issue_id, input: { stateId: $state_id })]
    pub issue_update: StateResult,
}
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "IssuePayload")]
pub struct StateResult {
    pub success: bool,
}
