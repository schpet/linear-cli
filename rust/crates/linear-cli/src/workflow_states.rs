//! Shared display ordering for a team's workflow states.

use std::cmp::Ordering;

use crate::error::{AppError, AppErrorKind};
use crate::graphql::operations::workflow_states::WorkflowState;
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

pub fn sort(states: &mut [WorkflowState]) -> Result<(), AppError> {
    for state in states.iter() {
        if !state.position.is_finite() {
            return Err(AppError::new(
                AppErrorKind::Invariant,
                format!("Workflow state \"{}\" has no usable position", state.name),
            )
            .with_suggestion("This indicates a malformed Linear API response."));
        }
    }
    let collator = collation::root()?;
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
            (None, None) => collator.compare(&left.state_type, &right.state_type),
        };
        by_type.then_with(|| {
            if right.position < left.position {
                Ordering::Less
            } else if right.position > left.position {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        })
    });
    Ok(())
}
