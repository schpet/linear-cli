//! Lifecycle is separate from workflow state: trashed issues still resolve by ID.
use crate::client::{LinearClient, RequestError};
use crate::ctx::Ctx;
use crate::error::{Error, Failure, Result};
use crate::graphql::operations::common::IdVariables;
use crate::graphql::operations::issue::{GetIssueLifecycle, IssueLifecycle};
use crate::graphql::scalars::DateTime;
use crate::platform::terminal_text::single_line;

#[derive(Clone, Copy)]
pub(super) enum Lifecycle {
    Active,
    Archived(DateTime),
    Deleted(Option<DateTime>),
}

impl Lifecycle {
    pub fn of(trashed: Option<bool>, archived_at: Option<DateTime>) -> Self {
        match (trashed, archived_at) {
            (Some(true), at) => Self::Deleted(at),
            (Some(false) | None, Some(at)) => Self::Archived(at),
            (Some(false) | None, None) => Self::Active,
        }
    }

    pub fn marker(self) -> &'static str {
        match self {
            Self::Active => "",
            Self::Archived(_) => " [archived]",
            Self::Deleted(_) => " [deleted]",
        }
    }

    fn description(self, identifier: &str) -> Option<String> {
        let identifier = single_line(identifier);
        match self {
            Self::Active => None,
            Self::Archived(at) => Some(format!(
                "{identifier} was archived on {}",
                at.0.date_naive()
            )),
            Self::Deleted(at) => Some(format!(
                "{identifier} was deleted{} (in trash)",
                at.map(|at| format!(" on {}", at.0.date_naive()))
                    .unwrap_or_default()
            )),
        }
    }

    pub fn warn(self, ctx: &Ctx, identifier: &str) -> Result<()> {
        if let Some(description) = self.description(identifier) {
            let remedy = match self {
                Self::Deleted(_) => "; restore it in Linear before working on it",
                Self::Active | Self::Archived(_) => "",
            };
            ctx.eprint(format!("Warning: {description}{remedy}.\n"))?;
        }
        Ok(())
    }

    pub fn ensure_not_deleted(self, identifier: &str) -> Result<()> {
        match self {
            Self::Deleted(_) => Err(self.deleted_error(identifier, Failure::General)),
            Self::Active | Self::Archived(_) => Ok(()),
        }
    }

    fn deleted_error(self, identifier: &str, failure: Failure) -> Error {
        Error::failed(
            failure,
            self.description(identifier)
                .expect("deleted lifecycle has a description"),
        )
        .with_hint("Restore it in Linear first.")
    }

    /// Only positive trash membership explains a rejected write. An archive is
    /// useful context, but does not establish that Linear forbids the operation.
    pub fn explain(self, issue: &str, original: Error) -> Error {
        match self {
            Self::Active => original,
            Self::Deleted(_) => self
                .deleted_error(
                    issue,
                    original
                        .failure()
                        .expect("a request failure has a runtime class"),
                )
                .with_source(original),
            Self::Archived(_) => original.with_hint(format!(
                "{}. Open it in Linear to inspect or unarchive it.",
                self.description(issue)
                    .expect("archived lifecycle has a description")
            )),
        }
    }
}

pub(super) fn of(issue: &IssueLifecycle) -> Lifecycle {
    Lifecycle::of(issue.trashed, issue.archived_at)
}

/// Diagnose only a definite missing Issue. Never retry a mutation or explain
/// missing comments, states, authentication failures or uncertain outcomes as deletion.
pub(crate) async fn write_error(
    client: &LinearClient,
    identifiers: &[&str],
    failure: RequestError,
) -> Error {
    if !failure.is_missing_entity("Issue") {
        return Error::from(failure);
    }
    let original = Error::from(failure);
    let mut resolved = false;
    let mut archived = None;
    for identifier in identifiers {
        let result: GetIssueLifecycle = match client
            .query(IdVariables {
                id: (*identifier).to_owned(),
            })
            .await
        {
            Ok(result) => result,
            Err(failure) if failure.is_not_found() => continue,
            Err(failure) => return diagnostic_failed(original, identifier, failure.into()),
        };
        if let Some(issue) = result.issue {
            resolved = true;
            match of(&issue) {
                Lifecycle::Deleted(_) => return of(&issue).explain(&issue.identifier, original),
                Lifecycle::Archived(_) => archived = Some(issue),
                Lifecycle::Active => {}
            }
        }
    }
    match archived {
        Some(issue) => of(&issue).explain(&issue.identifier, original),
        None => match (resolved, identifiers) {
            (false, [identifier]) => Error::not_found("Issue", identifier).with_source(original),
            _ => original,
        },
    }
}

pub(super) fn diagnostic_failed(original: Error, identifier: &str, failure: Error) -> Error {
    original
        .with_hint(format!(
            "Could not check whether {} is deleted or archived: {}",
            single_line(identifier),
            single_line(failure.message())
        ))
        .with_source(failure)
}
