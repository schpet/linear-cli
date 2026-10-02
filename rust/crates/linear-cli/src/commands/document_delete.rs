//! Concurrent app integration and raw-error gate remain explicit.
use crate::{
    commands::initiative_bulk::{BulkOutcome, BulkResult},
    graphql::{envelope::GraphQlRequest, operations::document_delete::*},
};
use cynic::{MutationBuilder, QueryBuilder};
pub const CONTEXT: &str = "Failed to delete document";
pub fn single_details_request(id: &str) -> GraphQlRequest<IdVariables> {
    GraphQlRequest::with_variables(GetDocumentForDelete::build(IdVariables {
        id: id.to_owned(),
    }))
}
pub fn bulk_details_request(id: &str) -> GraphQlRequest<IdVariables> {
    GraphQlRequest::with_variables(GetDocumentForBulkDelete::build(IdVariables {
        id: id.to_owned(),
    }))
}
pub fn single_delete_request(id: &str) -> GraphQlRequest<IdVariables> {
    GraphQlRequest::with_variables(DeleteDocument::build(IdVariables { id: id.to_owned() }))
}
pub fn bulk_delete_request(id: &str) -> GraphQlRequest<IdVariables> {
    GraphQlRequest::with_variables(BulkDeleteDocument::build(IdVariables { id: id.to_owned() }))
}
pub fn deleted(title: &str) -> Vec<u8> {
    format!("✓ Deleted document: {title}\n").into_bytes()
}
pub fn summary(results: &[BulkResult]) -> (Vec<u8>, bool) {
    let total = results.len();
    let succeeded = results.iter().filter(|r| r.succeeded()).count();
    let failed = total - succeeded;
    let plural = if total == 1 { "" } else { "s" };
    let mut out = String::from("\n");
    if failed == 0 {
        out.push_str(&format!(
            "✓ Successfully deleted {succeeded} document{}\n",
            if succeeded == 1 { "" } else { "s" }
        ));
    } else if succeeded == 0 {
        out.push_str(&format!(
            "✗ Failed to delete all {total} document{plural}\n"
        ));
    } else {
        out.push_str(&format!("Completed: {succeeded}/{total} document{plural} deleted\n  ✓ Succeeded: {succeeded}\n  ✗ Failed: {failed}\n"));
    }
    if failed > 0 {
        out.push_str("\nFailed operations:\n");
        for row in results {
            if let BulkOutcome::Failed(error) = &row.outcome {
                let name = row
                    .name
                    .as_ref()
                    .filter(|x| !x.is_empty())
                    .map_or_else(String::new, |x| format!(" ({x})"));
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

use crate::commands::initiative_bulk::Progress;
use crate::{
    error::Error,
    graphql::{bulk_error, transport::GraphQlTransport},
    refs::{WorkspaceScope, resolve_document_reference},
};
use std::cell::{Cell, RefCell};
pub struct Target {
    pub original: String,
    pub id: Result<String, Error>,
}
impl Target {
    pub fn prepare(original: String, scope: &WorkspaceScope<'_>) -> Self {
        let id = resolve_document_reference(&original, scope);
        Self { original, id }
    }
}
pub async fn single_details(
    transport: &GraphQlTransport,
    original: &str,
    id: &str,
) -> Result<DocumentDetails, Error> {
    let data: GetDocumentForDelete = transport.execute(&single_details_request(id)).await?;
    data.document
        .ok_or_else(|| Error::not_found("Document", original))
}
pub async fn submit_single(
    transport: &GraphQlTransport,
    document: &DocumentDetails,
) -> Result<Vec<u8>, Error> {
    let result: DeleteDocument = transport
        .execute(&single_delete_request(document.id.inner()))
        .await?;
    if !result.document_delete.success {
        return Err(Error::new("Delete operation failed"));
    }
    Ok(deleted(&document.title))
}
async fn run_resolved(
    transport: &GraphQlTransport,
    original: &str,
    id: &str,
) -> Result<BulkResult, Error> {
    let data: Result<GetDocumentForBulkDelete, bulk_error::BulkExchangeFailure> =
        bulk_error::execute(transport, &bulk_details_request(id)).await;
    let document = match data {
        Ok(data) => data.document,
        Err(bulk_error::BulkExchangeFailure::Strict(error)) => return Err(error),
        Err(bulk_error::BulkExchangeFailure::Ordinary(_)) => {
            return Ok(BulkResult {
                id: original.to_owned(),
                name: None,
                outcome: BulkOutcome::Failed("Document not found".to_owned()),
            });
        }
    };
    let (uuid, title) = document.map_or_else(
        || (id.to_owned(), original.to_owned()),
        |doc| (doc.id.into_inner(), doc.title),
    );
    let result: BulkDeleteDocument = bulk_error::execute(transport, &bulk_delete_request(&uuid))
        .await
        .map_err(bulk_error::BulkExchangeFailure::into_error)?;
    Ok(BulkResult {
        id: uuid,
        name: Some(title),
        outcome: if result.document_delete.success {
            BulkOutcome::Succeeded
        } else {
            BulkOutcome::Failed("Delete operation failed".to_owned())
        },
    })
}
pub async fn run_item(transport: &GraphQlTransport, target: Target) -> BulkResult {
    let result = match target.id {
        Ok(id) => run_resolved(transport, &target.original, &id).await,
        Err(error) => Err(error),
    };
    result.unwrap_or_else(|error| BulkResult {
        id: target.original,
        name: None,
        outcome: BulkOutcome::Failed(error.to_string()),
    })
}
async fn slot<F>(
    transport: &GraphQlTransport,
    target: Option<Target>,
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
    let result = run_item(transport, target).await;
    completed.set(completed.get() + 1);
    progress.borrow_mut()(Progress {
        completed: completed.get(),
        total,
        succeeded,
    })?;
    Ok(Some(result))
}
/// Exactly five borrowed in-flight operations and a barrier before the next batch.
pub async fn execute<F>(
    transport: &GraphQlTransport,
    targets: Vec<Target>,
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
        let Some(first) = targets.next() else { break };
        let succeeded = results
            .iter()
            .filter(|r: &&BulkResult| r.succeeded())
            .count();
        let (a, b, c, d, e) = tokio::join!(
            slot(
                transport,
                Some(first),
                &completed,
                total,
                succeeded,
                &progress
            ),
            slot(
                transport,
                targets.next(),
                &completed,
                total,
                succeeded,
                &progress
            ),
            slot(
                transport,
                targets.next(),
                &completed,
                total,
                succeeded,
                &progress
            ),
            slot(
                transport,
                targets.next(),
                &completed,
                total,
                succeeded,
                &progress
            ),
            slot(
                transport,
                targets.next(),
                &completed,
                total,
                succeeded,
                &progress
            )
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
