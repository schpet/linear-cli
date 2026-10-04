//! A team's workflow states: fetching them and the orders they are shown in.

use std::cmp::Ordering;

use crate::client::LinearClient;
use crate::error::Result;
use crate::graphql::operations::team::{
    GetWorkflowStates, GetWorkflowStatesVariables, WorkflowState,
};
use crate::graphql::pagination::{self, Page};
use crate::platform::collation;

/// The order of a team's workflow in Linear's settings, which state lists
/// and pickers follow.
const WORKFLOW_ORDER: [&str; 7] = [
    "triage",
    "backlog",
    "unstarted",
    "started",
    "completed",
    "canceled",
    "duplicate",
];

/// Issue lists put work in progress first.
const ISSUE_LIST_ORDER: [&str; 7] = [
    "triage",
    "started",
    "unstarted",
    "backlog",
    "completed",
    "canceled",
    "duplicate",
];

/// Every workflow state of the team with key `team_key`, in workflow order:
/// by type, then by position within a type.
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
    states.sort_by(|left, right| {
        compare_in(&WORKFLOW_ORDER, &left.state_type, &right.state_type)
            .then_with(|| left.position.get().total_cmp(&right.position.get()))
    });
    Ok(states)
}

/// The order of state types in issue lists: known types first, followed by
/// unknown types alphabetically.
pub fn compare_types(left: &str, right: &str) -> Ordering {
    compare_in(&ISSUE_LIST_ORDER, left, right)
}

fn compare_in(order: &[&str], left: &str, right: &str) -> Ordering {
    let left_rank = order.iter().position(|kind| *kind == left);
    let right_rank = order.iter().position(|kind| *kind == right);
    match (left_rank, right_rank) {
        (Some(left), Some(right)) => left.cmp(&right),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => collation::compare(left, right),
    }
}
