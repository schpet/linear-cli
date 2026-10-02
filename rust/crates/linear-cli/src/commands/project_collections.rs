//! Ordered collection edits and partial initiative-write diagnostics.

use crate::error::{AppError, AppErrorKind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedRef {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissingMember(pub ResolvedRef);

/// Preserve existing duplicates and order; callers dedupe fetched rows first.
pub fn apply_collection_edit(
    current: &[String],
    add: &[ResolvedRef],
    remove: &[ResolvedRef],
) -> Result<Vec<String>, MissingMember> {
    for reference in remove {
        if !current.contains(&reference.id) {
            return Err(MissingMember(reference.clone()));
        }
    }
    let mut result: Vec<String> = current
        .iter()
        .filter(|id| !remove.iter().any(|reference| &reference.id == *id))
        .cloned()
        .collect();
    for reference in add {
        if !result.contains(&reference.id) {
            result.push(reference.id.clone());
        }
    }
    Ok(result)
}

pub fn has_add_remove_overlap(add: &[ResolvedRef], remove: &[ResolvedRef]) -> bool {
    add.iter()
        .any(|added| remove.iter().any(|removed| removed.id == added.id))
}

/// Applied after sequential reference resolution, preserving the first label.
pub fn dedupe_refs(references: Vec<ResolvedRef>) -> Vec<ResolvedRef> {
    let mut result: Vec<ResolvedRef> = Vec::new();
    for reference in references {
        if !result.iter().any(|kept| kept.id == reference.id) {
            result.push(reference);
        }
    }
    result
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InitiativeLink {
    pub id: String,
    pub initiative_id: String,
    pub initiative_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InitiativeChange {
    Add {
        initiative_id: String,
        label: String,
    },
    Remove {
        link_id: String,
        initiative_id: String,
        label: String,
    },
}

/// Input links have been deduped by ROW id, never by initiative id.
/// Desired IDs come from a replacement list or apply_collection_edit.
pub fn plan_initiative_changes(
    links: &[InitiativeLink],
    desired_ids: &[String],
    desired_labels: &[ResolvedRef],
) -> Vec<InitiativeChange> {
    let mut changes = Vec::new();
    for link in links {
        if !desired_ids.contains(&link.initiative_id) {
            changes.push(InitiativeChange::Remove {
                link_id: link.id.clone(),
                initiative_id: link.initiative_id.clone(),
                label: link.initiative_name.clone(),
            });
        }
    }
    for id in desired_ids {
        if !links.iter().any(|link| &link.initiative_id == id) {
            let label = desired_labels
                .iter()
                .find(|reference| &reference.id == id)
                .map_or_else(|| id.clone(), |reference| reference.label.clone());
            changes.push(InitiativeChange::Add {
                initiative_id: id.clone(),
                label,
            });
        }
    }
    changes
}

impl InitiativeChange {
    pub fn description(&self) -> String {
        match self {
            Self::Add { label, .. } => format!("added \"{label}\""),
            Self::Remove { label, .. } => format!("removed \"{label}\""),
        }
    }

    pub fn recovery_flag(&self) -> String {
        match self {
            Self::Add { initiative_id, .. } => format!("--add-initiative {initiative_id}"),
            Self::Remove { initiative_id, .. } => format!("--remove-initiative {initiative_id}"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailedWrite {
    Rejected,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartialDiagnostic {
    pub message: String,
    pub suggestion: String,
}

/// Only invoked for an actual failed sequential write, never for a no-op.
pub fn partial_diagnostic(
    changes: &[InitiativeChange],
    applied: usize,
    outcome: FailedWrite,
    prior_fields_applied: bool,
) -> Result<PartialDiagnostic, AppError> {
    let (completed, pending) = changes.split_at_checked(applied).ok_or_else(|| {
        AppError::new(
            AppErrorKind::Invariant,
            "failed write index exceeds initiative plan",
        )
    })?;
    let (current, after_current) = pending.split_first().ok_or_else(|| {
        AppError::new(
            AppErrorKind::Invariant,
            "failed write is missing from initiative plan",
        )
    })?;
    let mut done = Vec::new();
    if prior_fields_applied {
        done.push("updated the project's other fields".to_owned());
    }
    done.extend(completed.iter().map(InitiativeChange::description));
    let done_text = if done.is_empty() {
        "none".to_owned()
    } else {
        done.join(", ")
    };
    let (unknown_text, not_applied) = match outcome {
        FailedWrite::Rejected => (String::new(), pending),
        FailedWrite::Unknown => (
            format!(
                " Unknown (the request failed before Linear answered): {}.",
                current.description()
            ),
            after_current,
        ),
    };
    let not_applied_text = if not_applied.is_empty() {
        String::new()
    } else {
        format!(
            " Not applied: {}.",
            not_applied
                .iter()
                .map(InitiativeChange::description)
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let remaining = pending
        .iter()
        .map(InitiativeChange::recovery_flag)
        .collect::<Vec<_>>()
        .join(" ");
    let advice = format!(
        "with only the remaining changes ({remaining}), or use --initiative to set the exact set."
    );
    let suggestion = match outcome {
        FailedWrite::Rejected => format!("Re-run {advice}"),
        FailedWrite::Unknown => format!("Check the project's initiatives, then re-run {advice}"),
    };
    Ok(PartialDiagnostic {
        message: format!(
            "Failed to update project initiatives after {applied} of {} changes; earlier changes were not rolled back. Applied: {done_text}.{unknown_text}{not_applied_text}",
            changes.len()
        ),
        suggestion,
    })
}
