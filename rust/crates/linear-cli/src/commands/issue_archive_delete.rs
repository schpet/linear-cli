//! `issue archive`/`delete`, single or bulk.
use crate::{
    commands::initiative_bulk::{BulkOutcome, BulkResult, Progress},
    error::Error,
    graphql::{
        bulk_error::{self, ObservedExchangeFailure},
        envelope::GraphQlRequest,
        operations::issue_archive_delete::*,
        transport::{GraphQlTransport, classify_typed},
    },
    refs::{self, IssueReference, WorkspaceScope},
};
use cynic::{MutationBuilder, QueryBuilder};
use std::cell::{Cell, RefCell};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Archive,
    Delete,
}
impl Mode {
    pub const fn verb(self) -> &'static str {
        match self {
            Self::Archive => "archive",
            Self::Delete => "delete",
        }
    }
    pub const fn past(self) -> &'static str {
        match self {
            Self::Archive => "archived",
            Self::Delete => "deleted",
        }
    }
    pub const fn context(self) -> &'static str {
        match self {
            Self::Archive => "Failed to archive issue",
            Self::Delete => "Failed to delete issue",
        }
    }
}
pub enum ReferenceOutcome {
    Resolved(String),
    Unresolved,
    Failed(Error),
}
pub struct Target {
    pub original: String,
    pub reference: ReferenceOutcome,
}
impl Target {
    pub fn prepare(original: String, team: Option<&str>, scope: &WorkspaceScope<'_>) -> Self {
        let reference = match refs::prepare_issue_reference(Some(&original), team, scope) {
            Ok(IssueReference::Identifier(id)) => ReferenceOutcome::Resolved(id),
            Ok(IssueReference::Unresolved) => ReferenceOutcome::Unresolved,
            Ok(IssueReference::Inferred) => ReferenceOutcome::Failed(Error::new(
                "explicit bulk issue reference requested inference",
            )),
            Err(error) => ReferenceOutcome::Failed(error),
        };
        Self {
            original,
            reference,
        }
    }
}
pub struct Details {
    pub identifier: String,
    pub title: String,
    pub already_archived: bool,
}
impl Details {
    pub fn name(&self) -> String {
        format!("{}: {}", self.identifier, self.title)
    }
}
pub fn details_request(id: &str, mode: Mode, bulk: bool) -> GraphQlRequest<IdVariables> {
    let variables = IdVariables { id: id.to_owned() };
    match (mode, bulk) {
        (Mode::Archive, false) => {
            GraphQlRequest::with_variables(GetIssueArchiveDetails::build(variables))
        }
        (Mode::Archive, true) => {
            GraphQlRequest::with_variables(GetIssueDetailsForBulkArchive::build(variables))
        }
        (Mode::Delete, false) => {
            GraphQlRequest::with_variables(GetIssueDeleteDetails::build(variables))
        }
        (Mode::Delete, true) => {
            GraphQlRequest::with_variables(GetIssueDetailsForBulkDelete::build(variables))
        }
    }
}
pub fn mutation_request(id: &str, mode: Mode, bulk: bool) -> GraphQlRequest<IdVariables> {
    let variables = IdVariables { id: id.to_owned() };
    match (mode, bulk) {
        (Mode::Archive, false) => GraphQlRequest::with_variables(ArchiveIssue::build(variables)),
        (Mode::Archive, true) => GraphQlRequest::with_variables(BulkArchiveIssue::build(variables)),
        (Mode::Delete, false) => GraphQlRequest::with_variables(DeleteIssue::build(variables)),
        (Mode::Delete, true) => GraphQlRequest::with_variables(BulkDeleteIssue::build(variables)),
    }
}
/// One captured exchange retains ordinary friendly errors, with archive's Client-only translation.
async fn single_exchange<T: serde::de::DeserializeOwned>(
    transport: &GraphQlTransport,
    request: &GraphQlRequest<IdVariables>,
    archive_not_found: Option<&str>,
) -> Result<T, Error> {
    let response = transport.send_request(request).await.map_err(Error::from)?;
    let observed = bulk_error::observe_source_error(&response, request)
        .map_err(bulk_error::BulkExchangeFailure::into_error)?;
    if let Some(error) = observed {
        if let Some(id) = archive_not_found
            && error.is_not_found()
        {
            return Err(Error::not_found("Issue", id));
        }
        // Prefer Linear's user-facing message; otherwise keep the full error
        // message, including its metadata.
        return Err(Error::new(error.preferred_message.unwrap_or(error.message)));
    }
    classify_typed(response).map_err(Error::from)
}
pub async fn single_details(
    transport: &GraphQlTransport,
    id: &str,
    mode: Mode,
) -> Result<Details, Error> {
    let request = details_request(id, mode, false);
    let details = match mode {
        Mode::Archive => {
            let data: GetIssueArchiveDetails =
                single_exchange(transport, &request, Some(id)).await?;
            data.issue.map(|issue| Details {
                identifier: issue.identifier,
                title: issue.title,
                already_archived: issue.archived_at.is_some(),
            })
        }
        Mode::Delete => {
            let data: GetIssueDeleteDetails = single_exchange(transport, &request, None).await?;
            data.issue.map(|issue| Details {
                identifier: issue.identifier,
                title: issue.title,
                already_archived: false,
            })
        }
    };
    details.ok_or_else(|| Error::not_found("Issue", id))
}
pub async fn submit_single(
    transport: &GraphQlTransport,
    id: &str,
    details: &Details,
    mode: Mode,
) -> Result<Vec<u8>, Error> {
    let request = mutation_request(id, mode, false);
    let success = match mode {
        Mode::Archive => {
            let data: ArchiveIssue = single_exchange(transport, &request, None).await?;
            data.issue_archive.success
        }
        Mode::Delete => {
            let data: DeleteIssue = single_exchange(transport, &request, None).await?;
            data.issue_delete.success
        }
    };
    if !success {
        return Err(Error::new(match mode {
            Mode::Archive => "Linear reported the archive as unsuccessful",
            Mode::Delete => "Failed to delete issue",
        }));
    }
    Ok(format!("✓ Successfully {} issue: {}\n", mode.past(), details.name()).into_bytes())
}
async fn bulk_resolved(
    transport: &GraphQlTransport,
    id: &str,
    mode: Mode,
) -> Result<BulkResult, Error> {
    let request = details_request(id, mode, true);
    let not_found = || BulkResult {
        id: id.to_owned(),
        name: None,
        outcome: BulkOutcome::Failed("Issue not found".to_owned()),
    };
    let (name, already_archived) = match mode {
        Mode::Archive => {
            let data: GetIssueDetailsForBulkArchive =
                match bulk_error::execute_observed(transport, &request).await {
                    Ok(data) => data,
                    Err(ObservedExchangeFailure::Ordinary(error)) if error.is_not_found() => {
                        return Ok(not_found());
                    }
                    Err(error) => return Err(error.into_error()),
                };
            let Some(issue) = data.issue else {
                return Ok(not_found());
            };
            (
                format!("{}: {}", issue.identifier, issue.title),
                issue.archived_at.is_some(),
            )
        }
        Mode::Delete => {
            // A failed details lookup only loses the title in the summary; the
            // delete still runs.
            let data: Result<GetIssueDetailsForBulkDelete, _> =
                bulk_error::execute_observed(transport, &request).await;
            let issue = match data {
                Ok(data) => data.issue,
                Err(_) => None,
            };
            let name = issue.map_or_else(
                || id.to_owned(),
                |issue| {
                    if issue.title.is_empty() {
                        issue.identifier
                    } else {
                        format!("{}: {}", issue.identifier, issue.title)
                    }
                },
            );
            (name, false)
        }
    };
    let success = if already_archived {
        true
    } else {
        let request = mutation_request(id, mode, true);
        match mode {
            Mode::Archive => {
                let data: BulkArchiveIssue = bulk_error::execute_observed(transport, &request)
                    .await
                    .map_err(ObservedExchangeFailure::into_error)?;
                data.issue_archive.success
            }
            Mode::Delete => {
                let data: BulkDeleteIssue = bulk_error::execute_observed(transport, &request)
                    .await
                    .map_err(ObservedExchangeFailure::into_error)?;
                data.issue_delete.success
            }
        }
    };
    Ok(BulkResult {
        id: id.to_owned(),
        name: Some(name),
        outcome: if success {
            BulkOutcome::Succeeded
        } else {
            BulkOutcome::Failed(format!(
                "{} operation failed",
                match mode {
                    Mode::Archive => "Archive",
                    Mode::Delete => "Delete",
                }
            ))
        },
    })
}
pub async fn run_item(transport: &GraphQlTransport, target: Target, mode: Mode) -> BulkResult {
    let result = match target.reference {
        ReferenceOutcome::Resolved(id) => bulk_resolved(transport, &id, mode).await,
        ReferenceOutcome::Unresolved => Ok(BulkResult {
            id: target.original.clone(),
            name: None,
            outcome: BulkOutcome::Failed("Issue not found".to_owned()),
        }),
        // Bulk rows show only the error message, not its suggestion or context.
        ReferenceOutcome::Failed(error) => Err(error),
    };
    result.unwrap_or_else(|error| BulkResult {
        id: target.original,
        name: None,
        outcome: BulkOutcome::Failed(error.message().to_owned()),
    })
}
async fn slot<F>(
    transport: &GraphQlTransport,
    target: Option<Target>,
    mode: Mode,
    completed: &Cell<usize>,
    total: usize,
    succeeded: usize,
    progress: &RefCell<F>,
) -> Result<Option<BulkResult>, Error>
where
    F: FnMut(Progress) -> Result<(), Error>,
{
    let Some(target) = target else {
        return Ok(None);
    };
    let result = run_item(transport, target, mode).await;
    completed.set(completed.get() + 1);
    progress.borrow_mut()(Progress {
        completed: completed.get(),
        total,
        succeeded,
    })?;
    Ok(Some(result))
}
/// Five borrowed futures, ordered rows, and a reply barrier before a sixth operation.
pub async fn execute<F>(
    transport: &GraphQlTransport,
    targets: Vec<Target>,
    mode: Mode,
    progress: F,
) -> Result<Vec<BulkResult>, Error>
where
    F: FnMut(Progress) -> Result<(), Error>,
{
    let total = targets.len();
    let mut targets = targets.into_iter();
    let completed = Cell::new(0);
    let progress = RefCell::new(progress);
    let mut results = Vec::with_capacity(total);
    loop {
        let Some(first) = targets.next() else {
            break;
        };
        let succeeded = results
            .iter()
            .filter(|row: &&BulkResult| row.succeeded())
            .count();
        let (a, b, c, d, e) = tokio::join!(
            slot(
                transport,
                Some(first),
                mode,
                &completed,
                total,
                succeeded,
                &progress
            ),
            slot(
                transport,
                targets.next(),
                mode,
                &completed,
                total,
                succeeded,
                &progress
            ),
            slot(
                transport,
                targets.next(),
                mode,
                &completed,
                total,
                succeeded,
                &progress
            ),
            slot(
                transport,
                targets.next(),
                mode,
                &completed,
                total,
                succeeded,
                &progress
            ),
            slot(
                transport,
                targets.next(),
                mode,
                &completed,
                total,
                succeeded,
                &progress
            )
        );
        for result in [a, b, c, d, e] {
            if let Some(row) = result? {
                results.push(row);
            }
        }
    }
    assert_eq!(
        completed.get(),
        total,
        "issue bulk completion count must match input"
    );
    assert_eq!(results.len(), total, "issue bulk rows must match input");
    Ok(results)
}
pub fn summary(results: &[BulkResult], mode: Mode) -> (Vec<u8>, bool) {
    let total = results.len();
    let succeeded = results.iter().filter(|row| row.succeeded()).count();
    let failed = total - succeeded;
    let plural = if total == 1 { "" } else { "s" };
    let mut out = String::from("\n");
    if failed == 0 {
        out.push_str(&format!(
            "✓ Successfully {} {succeeded} issue{}\n",
            mode.past(),
            if succeeded == 1 { "" } else { "s" }
        ));
    } else if succeeded == 0 {
        out.push_str(&format!(
            "✗ Failed to {} all {total} issue{plural}\n",
            match mode {
                Mode::Archive => "archive",
                Mode::Delete => "delete",
            }
        ));
    } else {
        out.push_str(&format!("Completed: {succeeded}/{total} issue{plural} {}\n  ✓ Succeeded: {succeeded}\n  ✗ Failed: {failed}\n",mode.past()));
    }
    if failed > 0 {
        out.push_str("\nFailed operations:\n");
        for row in results {
            if let BulkOutcome::Failed(error) = &row.outcome {
                let name = row
                    .name
                    .as_ref()
                    .filter(|name| !name.is_empty())
                    .map_or_else(String::new, |name| format!(" ({name})"));
                out.push_str(&format!(
                    "  - {}{name}: {}\n",
                    row.id,
                    if error.is_empty() {
                        "Unknown error"
                    } else {
                        error
                    }
                ));
            }
        }
    }
    (out.into_bytes(), failed > 0)
}
/// Descriptor type, not CI or a broad !stdoutTTY gate, defines the approved boundary.
#[cfg(unix)]
pub fn stdout_is_pipe() -> Result<bool, Error> {
    let stat = rustix::fs::fstat(std::io::stdout()).map_err(|error| {
        Error::new("Failed to inspect issue confirmation stdout").with_source(error)
    })?;
    Ok(rustix::fs::FileType::from_raw_mode(stat.st_mode) == rustix::fs::FileType::Fifo)
}
#[cfg(not(unix))]
pub fn stdout_is_pipe() -> Result<bool, Error> {
    // The qualified FIFO refusal is Unix-only; other targets retain native prompts.
    Ok(false)
}
