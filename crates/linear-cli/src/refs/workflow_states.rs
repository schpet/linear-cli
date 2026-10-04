//! A team's workflow states: fetching them and the orders they are shown in.

use std::cmp::Ordering;

use crate::client::LinearClient;
use crate::error::{Error, Result};
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

/// The state types `--state` accepts in place of a state name.
pub const STATE_TYPES: [&str; 6] = [
    "triage",
    "backlog",
    "unstarted",
    "started",
    "completed",
    "canceled",
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

/// A workflow state as the hint for an unknown `--state` lists it.
pub struct Candidate<'a> {
    pub name: &'a str,
    pub state_type: &'a str,
    pub position: f64,
    pub team_key: &'a str,
}

/// The error for a `--state` value that names no workflow state in the
/// teams with keys `teams` (every team when `None`). The hint lists the
/// states it could name in workflow order, grouped by team when there may
/// be several, and the state types that work in their place.
pub fn unknown_state(
    reference: &str,
    teams: Option<&[String]>,
    mut candidates: Vec<Candidate<'_>>,
) -> Error {
    let single = match teams {
        Some([key]) => Some(key.as_str()),
        Some(_) | None => None,
    };
    let scope = match teams {
        None => "any team".to_owned(),
        Some([key]) => format!("team {key}"),
        Some(keys) => format!("teams {}", keys.join(", ")),
    };
    candidates.sort_by(|left, right| {
        collation::compare(left.team_key, right.team_key)
            .then_with(|| compare_in(&WORKFLOW_ORDER, left.state_type, right.state_type))
            .then_with(|| left.position.total_cmp(&right.position))
    });
    let listed: Vec<String> = candidates
        .iter()
        .map(|state| {
            let name = crate::commands::json::quoted(state.name);
            match single {
                Some(_) => format!("{name} ({})", state.state_type),
                None => format!("{name} ({}, {})", state.state_type, state.team_key),
            }
        })
        .collect();
    let states = if listed.is_empty() {
        format!("{scope} has no workflow states. ")
    } else {
        format!("Valid states: {}. ", listed.join(", "))
    };
    let list = match single {
        Some(key) => format!("Run `linear team states {key}` to list them."),
        None => "Run `linear team states <team>` to list a team's states.".to_owned(),
    };
    Error::not_found("Workflow state", &format!("'{reference}' in {scope}")).with_hint(format!(
        "{}A state type also works: {}. {list}",
        capitalize(&states),
        STATE_TYPES.join(", ")
    ))
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state<'a>(
        name: &'a str,
        state_type: &'a str,
        position: f64,
        team_key: &'a str,
    ) -> Candidate<'a> {
        Candidate {
            name,
            state_type,
            position,
            team_key,
        }
    }

    #[test]
    fn unknown_states_list_the_workflow_in_order_with_one_wording() {
        let team = ["ENG".to_owned()];
        let error = unknown_state(
            "nope",
            Some(&team),
            vec![
                state("Done", "completed", 1.0, "ENG"),
                state("Todo", "unstarted", 2.0, "ENG"),
                state("Backlog", "backlog", 1.0, "ENG"),
                state("Ready", "unstarted", 1.0, "ENG"),
            ],
        );
        assert_eq!(
            error.message(),
            "Workflow state not found: 'nope' in team ENG"
        );
        assert_eq!(
            error.hint(),
            Some(
                "Valid states: \"Backlog\" (backlog), \"Ready\" (unstarted), \"Todo\" (unstarted), \"Done\" (completed). \
                 A state type also works: triage, backlog, unstarted, started, completed, canceled. \
                 Run `linear team states ENG` to list them."
            )
        );
    }

    #[test]
    fn unknown_states_across_teams_group_by_team() {
        let error = unknown_state(
            "nope",
            None,
            vec![
                state("Todo", "unstarted", 1.0, "OPS"),
                state("Done", "completed", 1.0, "ENG"),
                state("Todo", "unstarted", 1.0, "ENG"),
            ],
        );
        assert_eq!(
            error.message(),
            "Workflow state not found: 'nope' in any team"
        );
        assert!(
            error.hint().is_some_and(|hint| hint.starts_with(
                "Valid states: \"Todo\" (unstarted, ENG), \"Done\" (completed, ENG), \"Todo\" (unstarted, OPS). "
            )),
            "{error:?}"
        );
        let empty = unknown_state("nope", Some(&["ENG".to_owned()]), Vec::new());
        assert!(
            empty
                .hint()
                .is_some_and(|hint| hint.starts_with("Team ENG has no workflow states. ")),
            "{empty:?}"
        );
    }
}
