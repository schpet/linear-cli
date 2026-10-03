//! A team's workflow states: fetching them and the shared display ordering.

use std::cmp::Ordering;

use crate::client::LinearClient;
use crate::error::Result;
use crate::graphql::operations::team::{
    GetWorkflowStates, GetWorkflowStatesVariables, WorkflowState,
};
use crate::graphql::pagination::{self, Page};
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

/// Every workflow state of the team with key `team_key`, in display order.
pub async fn fetch(client: &LinearClient, team_key: String) -> Result<Vec<WorkflowState>> {
    let mut states = pagination::collect(None, |after, first| {
        let variables = GetWorkflowStatesVariables {
            team_key: team_key.clone(),
            first,
            after,
        };
        async move {
            let data: GetWorkflowStates = client.query(variables).await?;
            Ok(Page {
                nodes: data.team.states.nodes,
                page_info: data.team.states.page_info,
            })
        }
    })
    .await?;
    sort(&mut states);
    Ok(states)
}

/// Known workflow types first, followed by unknown types in display order.
pub fn compare_types(left: &str, right: &str) -> Ordering {
    let left_rank = KNOWN_TYPES.iter().position(|kind| *kind == left);
    let right_rank = KNOWN_TYPES.iter().position(|kind| *kind == right);
    match (left_rank, right_rank) {
        (Some(left), Some(right)) => left.cmp(&right),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => collation::compare(left, right),
    }
}

fn sort(states: &mut [WorkflowState]) {
    states.sort_by(|left, right| {
        compare_types(&left.state_type, &right.state_type)
            .then_with(|| right.position.get().total_cmp(&left.position.get()))
    });
}
