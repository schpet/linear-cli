//! `initiative archive`/`delete`, including bulk mode run in batches of five.
use crate::{
    commands::bulk::{BulkOutcome, BulkResult, Progress},
    commands::initiative::view::{Reference, prepare_reference},
    error::{Error, ResultExt},
    graphql::{
        bulk_error,
        envelope::GraphQlRequest,
        operations::{
            initiative_bulk::*,
            initiative_view::{ResolveInitiativeBySlug, UrlSlugVariables},
        },
        transport::GraphQlTransport,
    },
    refs::{WorkspaceScope, is_linear_uuid},
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
            Self::Archive => "Failed to archive initiative",
            Self::Delete => "Failed to delete initiative",
        }
    }
    pub const fn bulk_cancelled(self) -> &'static [u8] {
        match self {
            Self::Archive => b"Bulk archive cancelled.\n",
            Self::Delete => b"Bulk delete cancelled.\n",
        }
    }
    pub const fn single_cancelled(self) -> &'static [u8] {
        match self {
            Self::Archive => b"Archive cancelled.\n",
            Self::Delete => b"Delete cancelled.\n",
        }
    }
}

/// Local URL/workspace refusal is kept per item, so it becomes a failure row in bulk mode.
/// Preparing these owned targets borrows no app state during concurrent requests/output.
pub struct Target {
    pub original: String,
    pub reference: Result<Reference, Error>,
}
impl Target {
    pub fn prepare(original: String, scope: &WorkspaceScope<'_>) -> Self {
        let reference = prepare_reference(&original, scope);
        Self {
            original,
            reference,
        }
    }
}
async fn resolve_text(transport: &GraphQlTransport, token: &str, mode: Mode) -> Option<String> {
    let variables = SlugVariables {
        slug_id: token.to_owned(),
    };
    let nodes = match mode {
        Mode::Archive => transport
            .execute::<GetInitiativeBySlugForArchive, _>(&GraphQlRequest::with_variables(
                GetInitiativeBySlugForArchive::build(variables),
            ))
            .await
            .ok()
            .map(|data| data.initiatives.nodes),
        Mode::Delete => transport
            .execute::<GetInitiativeBySlugForDelete, _>(&GraphQlRequest::with_variables(
                GetInitiativeBySlugForDelete::build(variables),
            ))
            .await
            .ok()
            .map(|data| data.initiatives.nodes),
    };
    if let Some(node) = nodes.and_then(|nodes| nodes.into_iter().next()) {
        return Some(node.id.into_inner()).filter(|id| !id.is_empty());
    }
    let variables = NameVariables {
        name: token.to_owned(),
    };
    let nodes = match mode {
        Mode::Archive => transport
            .execute::<GetInitiativeByNameForArchive, _>(&GraphQlRequest::with_variables(
                GetInitiativeByNameForArchive::build(variables),
            ))
            .await
            .ok()
            .map(|data| data.initiatives.nodes),
        Mode::Delete => transport
            .execute::<GetInitiativeByNameForDelete, _>(&GraphQlRequest::with_variables(
                GetInitiativeByNameForDelete::build(variables),
            ))
            .await
            .ok()
            .map(|data| data.initiatives.nodes),
    };
    nodes
        .and_then(|nodes| nodes.into_iter().next())
        .map(|node| node.id.into_inner())
        .filter(|id| !id.is_empty())
}
pub async fn resolve(
    transport: &GraphQlTransport,
    reference: &Reference,
    mode: Mode,
) -> Result<Option<String>, Error> {
    resolve_with_errors(transport, reference, mode, ResolutionErrors::SingleFriendly).await
}
#[derive(Clone, Copy)]
enum ResolutionErrors {
    SingleFriendly,
    BulkSourceMessage,
}
async fn resolve_with_errors(
    transport: &GraphQlTransport,
    reference: &Reference,
    mode: Mode,
    errors: ResolutionErrors,
) -> Result<Option<String>, Error> {
    match reference {
        Reference::Id(id) => Ok(Some(id.clone())),
        Reference::NameOrSlug(token) => Ok(resolve_text(transport, token, mode).await),
        Reference::UrlSlug(slug) => {
            let request =
                GraphQlRequest::with_variables(ResolveInitiativeBySlug::build(UrlSlugVariables {
                    slug_id: slug.clone(),
                    include_archived: Some(mode == Mode::Delete),
                }));
            let data: ResolveInitiativeBySlug = match errors {
                ResolutionErrors::SingleFriendly => {
                    transport.execute(&request).await.map_err(Error::from)?
                }
                ResolutionErrors::BulkSourceMessage => bulk_error::execute(transport, &request)
                    .await
                    .map_err(bulk_error::BulkExchangeFailure::into_error)?,
            };
            let Some(id) = data
                .initiatives
                .nodes
                .into_iter()
                .next()
                .map(|node| node.id.into_inner())
            else {
                return Ok(None);
            };
            if is_linear_uuid(&id) {
                Ok(Some(id))
            } else {
                Ok(resolve_text(transport, &id, mode).await)
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SingleDetails {
    Archive(ArchiveDetail),
    Delete(DeleteDetail),
}
impl SingleDetails {
    pub fn name(&self) -> &str {
        match self {
            Self::Archive(detail) => &detail.name,
            Self::Delete(detail) => &detail.name,
        }
    }
    pub fn already_archived(&self) -> bool {
        match self {
            Self::Archive(detail) => detail
                .archived_at
                .as_ref()
                .is_some_and(|date| !date.0.is_empty()),
            Self::Delete(_) => false,
        }
    }
    pub fn linked_warning(&self) -> Option<Vec<u8>> {
        match self {
            Self::Delete(detail) => {
                let count = detail
                    .projects
                    .as_ref()
                    .map_or(0, |projects| projects.nodes.len());
                (count>0).then(||format!("\n⚠️  Initiative \"{}\" has {count} linked project(s).\nDeleting the initiative will unlink these projects.\n\n",detail.name).into_bytes())
            }
            Self::Archive(_) => None,
        }
    }
}
pub async fn fetch_single(
    transport: &GraphQlTransport,
    id: &str,
    mode: Mode,
) -> Result<Option<SingleDetails>, Error> {
    let variables = IdVariables { id: id.to_owned() };
    let result = match mode {
        Mode::Archive => transport
            .execute::<GetInitiativeForArchive, _>(&GraphQlRequest::with_variables(
                GetInitiativeForArchive::build(variables),
            ))
            .await
            .map(|data| data.initiative.map(SingleDetails::Archive)),
        Mode::Delete => transport
            .execute::<GetInitiativeForDelete, _>(&GraphQlRequest::with_variables(
                GetInitiativeForDelete::build(variables),
            ))
            .await
            .map(|data| data.initiative.map(SingleDetails::Delete)),
    };
    result
        .map_err(Error::from)
        .context("Failed to fetch initiative details")
}
pub async fn submit_single(
    transport: &GraphQlTransport,
    id: &str,
    name: &str,
    mode: Mode,
) -> Result<Vec<u8>, Error> {
    let variables = IdVariables { id: id.to_owned() };
    let result = match mode {
        Mode::Archive => transport
            .execute::<ArchiveInitiative, _>(&GraphQlRequest::with_variables(
                ArchiveInitiative::build(variables),
            ))
            .await
            .map(|data| data.initiative_archive.success),
        Mode::Delete => transport
            .execute::<DeleteInitiative, _>(&GraphQlRequest::with_variables(
                DeleteInitiative::build(variables),
            ))
            .await
            .map(|data| data.initiative_delete.success),
    };
    let success = result.map_err(Error::from).context(mode.context())?;
    if !success {
        return Err(Error::new(mode.context()).context(mode.context()));
    }
    Ok(match mode {
        Mode::Archive => format!("✓ Archived initiative: {name}\n"),
        Mode::Delete => format!("✓ Permanently deleted initiative: {name}\n"),
    }
    .into_bytes())
}

async fn run_resolved(
    transport: &GraphQlTransport,
    original: &str,
    reference: &Reference,
    mode: Mode,
) -> Result<BulkResult, Error> {
    let Some(id) = resolve_with_errors(
        transport,
        reference,
        mode,
        ResolutionErrors::BulkSourceMessage,
    )
    .await?
    else {
        return Ok(BulkResult {
            id: original.to_owned(),
            name: Some(original.to_owned()),
            outcome: BulkOutcome::Failed("Initiative not found".to_owned()),
        });
    };
    let variables = IdVariables { id: id.clone() };
    let (name, already_archived) = match mode {
        Mode::Archive => transport
            .execute::<GetInitiativeNameForBulkArchive, _>(&GraphQlRequest::with_variables(
                GetInitiativeNameForBulkArchive::build(variables),
            ))
            .await
            .ok()
            .and_then(|data| data.initiative)
            .map_or_else(
                || (original.to_owned(), false),
                |node| {
                    (
                        node.name,
                        node.archived_at.is_some_and(|date| !date.0.is_empty()),
                    )
                },
            ),
        Mode::Delete => transport
            .execute::<GetInitiativeNameForBulkDelete, _>(&GraphQlRequest::with_variables(
                GetInitiativeNameForBulkDelete::build(variables),
            ))
            .await
            .ok()
            .and_then(|data| data.initiative)
            .map_or_else(|| (original.to_owned(), false), |node| (node.name, false)),
    };
    if already_archived {
        return Ok(BulkResult {
            id,
            name: Some(name),
            outcome: BulkOutcome::Succeeded,
        });
    }
    let variables = IdVariables { id: id.clone() };
    let success = match mode {
        Mode::Archive => bulk_error::execute::<BulkArchiveInitiative, _>(
            transport,
            &GraphQlRequest::with_variables(BulkArchiveInitiative::build(variables)),
        )
        .await
        .map(|data| data.initiative_archive.success),
        Mode::Delete => bulk_error::execute::<BulkDeleteInitiative, _>(
            transport,
            &GraphQlRequest::with_variables(BulkDeleteInitiative::build(variables)),
        )
        .await
        .map(|data| data.initiative_delete.success),
    }
    .map_err(bulk_error::BulkExchangeFailure::into_error)?;
    Ok(BulkResult {
        id,
        name: Some(name),
        outcome: if success {
            BulkOutcome::Succeeded
        } else {
            BulkOutcome::Failed(
                match mode {
                    Mode::Archive => "Archive operation failed",
                    Mode::Delete => "Delete operation failed",
                }
                .to_owned(),
            )
        },
    })
}
pub async fn run_item(transport: &GraphQlTransport, target: Target, mode: Mode) -> BulkResult {
    let result = match target.reference {
        Ok(reference) => run_resolved(transport, &target.original, &reference, mode).await,
        Err(error) => Err(error),
    };
    result.unwrap_or_else(|error| BulkResult {
        id: target.original,
        name: None,
        outcome: BulkOutcome::Failed(error.to_string()),
    })
}
struct BatchContext<'a, F> {
    transport: &'a GraphQlTransport,
    mode: Mode,
    completed: &'a Cell<usize>,
    total: usize,
    succeeded: usize,
    progress: &'a RefCell<F>,
}
async fn run_slot<F>(
    context: &BatchContext<'_, F>,
    target: Option<Target>,
) -> Result<Option<BulkResult>, Error>
where
    F: FnMut(Progress) -> Result<(), Error>,
{
    let Some(target) = target else {
        return Ok(None);
    };
    let result = run_item(context.transport, target, context.mode).await;
    let completed = context.completed.get() + 1;
    context.completed.set(completed);
    (context.progress.borrow_mut())(Progress {
        completed,
        total: context.total,
        succeeded: context.succeeded,
    })?;
    Ok(Some(result))
}
/// Five borrowed futures run concurrently. The next chunk starts only after all five finish.
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
    let mut results = Vec::with_capacity(total);
    let completed = Cell::new(0);
    let progress = RefCell::new(progress);
    loop {
        let Some(first) = targets.next() else { break };
        let context = BatchContext {
            transport,
            mode,
            completed: &completed,
            total,
            succeeded: results
                .iter()
                .filter(|result: &&BulkResult| result.succeeded())
                .count(),
            progress: &progress,
        };
        let (a, b, c, d, e) = tokio::join!(
            run_slot(&context, Some(first)),
            run_slot(&context, targets.next()),
            run_slot(&context, targets.next()),
            run_slot(&context, targets.next()),
            run_slot(&context, targets.next())
        );
        for result in [a, b, c, d, e] {
            if let Some(result) = result? {
                results.push(result)
            }
        }
    }
    assert_eq!(
        completed.get(),
        total,
        "bulk completion count must equal input count"
    );
    assert_eq!(results.len(), total, "bulk rows must equal input count");
    Ok(results)
}
pub fn summary(results: &[BulkResult], mode: Mode) -> (Vec<u8>, bool) {
    let total = results.len();
    let succeeded = results.iter().filter(|result| result.succeeded()).count();
    let failed = total - succeeded;
    let plural = if total == 1 { "" } else { "s" };
    let mut output = String::from("\n");
    if failed == 0 {
        output.push_str(&format!(
            "✓ Successfully {} {succeeded} initiative{}\n",
            mode.past(),
            if succeeded == 1 { "" } else { "s" }
        ));
    } else if succeeded == 0 {
        output.push_str(&format!(
            "✗ Failed to {} all {total} initiative{plural}\n",
            match mode {
                Mode::Archive => "archive",
                Mode::Delete => "delete",
            }
        ));
    } else {
        output.push_str(&format!("Completed: {succeeded}/{total} initiative{plural} {}\n  ✓ Succeeded: {succeeded}\n  ✗ Failed: {failed}\n",mode.past()));
    }
    if failed > 0 {
        output.push_str("\nFailed operations:\n");
        for result in results {
            if let BulkOutcome::Failed(error) = &result.outcome {
                let name = result
                    .name
                    .as_ref()
                    .filter(|name| !name.is_empty())
                    .map_or_else(String::new, |name| format!(" ({name})"));
                output.push_str(&format!(
                    "  - {}{name}: {}\n",
                    result.id,
                    if error.is_empty() {
                        "Unknown error"
                    } else {
                        error
                    }
                ));
            }
        }
    }
    (output.into_bytes(), failed > 0)
}
