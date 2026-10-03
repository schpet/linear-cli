//! A team's workflow states: the request and the shared display ordering.

use std::cmp::Ordering;

use cynic::QueryBuilder;

use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::workflow_states::{
    GetWorkflowStates, GetWorkflowStatesVariables, WorkflowState,
};
use crate::platform::collation;

const KNOWN_TYPES: [&str; 7] = [
    "triage",
    "started",
    "unstarted",
    "backlog",
    "completed",
    "canceled",
    "duplicate",
];

/// The workflow states of the team with key `team_key`.
pub fn request(team_key: String) -> GraphQlRequest<GetWorkflowStatesVariables> {
    GraphQlRequest::with_variables(GetWorkflowStates::build(GetWorkflowStatesVariables {
        team_key,
    }))
}

pub fn sort(states: &mut [WorkflowState]) {
    states.sort_by(|left, right| {
        let left_rank = KNOWN_TYPES
            .iter()
            .position(|kind| *kind == left.state_type.as_str());
        let right_rank = KNOWN_TYPES
            .iter()
            .position(|kind| *kind == right.state_type.as_str());
        let by_type = match (left_rank, right_rank) {
            (Some(left), Some(right)) => left.cmp(&right),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => collation::compare(&left.state_type, &right.state_type),
        };
        by_type.then_with(|| right.position.get().total_cmp(&left.position.get()))
    });
}
