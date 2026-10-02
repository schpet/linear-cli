//! `document delete`: move one document, or many in bulk, to the trash.
use cynic::{MutationBuilder, QueryBuilder};
use futures_util::{StreamExt, stream};

use crate::cli::document::DocumentDelete;
use crate::commands::bulk::{self, BulkInput, BulkOutcome, BulkResult, Progress};
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::document_delete::{
    DeleteDocument, DocumentDetails, GetDocumentForDelete, IdVariables,
};
use crate::graphql::transport::GraphQlTransport;

pub fn run(ctx: &Ctx, args: &DocumentDelete) -> Result<()> {
    delete(ctx, args).context("Failed to delete document")
}

fn delete(ctx: &Ctx, args: &DocumentDelete) -> Result<()> {
    if !args.yes {
        ctx.require_tty("--yes")?;
    }
    let input = BulkInput {
        argv: args.bulk.as_deref(),
        file: args.bulk_file.as_deref().map(std::path::Path::new),
        stdin: args.bulk_stdin,
    };
    if input.requested() {
        return delete_bulk(ctx, args, &input);
    }
    let original = args.document_id.as_deref().ok_or_else(|| {
        Error::new("Document ID required").with_hint("Use --bulk for multiple documents.")
    })?;
    let id = super::reference(ctx, original)?;
    let client = ctx.client()?;
    let document = ctx.spin(true, async {
        details(client, &id)
            .await?
            .ok_or_else(|| Error::not_found("Document", original))
    })?;
    let question = format!("Are you sure you want to delete \"{}\"?", document.title);
    if !args.yes && !ctx.confirm(&question, "--yes")? {
        return ctx.print("Delete cancelled.\n");
    }
    ctx.spin(true, submit(client, document.id.inner()))?;
    ctx.print(format!("✓ Deleted document: {}\n", document.title))
}

fn delete_bulk(ctx: &Ctx, args: &DocumentDelete, input: &BulkInput<'_>) -> Result<()> {
    let ids = bulk::collect_ids(input, &mut std::io::stdin().lock())?;
    if ids.is_empty() {
        return Err(Error::new("No document IDs provided for bulk delete"));
    }
    ctx.print(format!("Found {} document(s) to delete.\n", ids.len()))?;
    let question = format!("Delete {} document(s)?", ids.len());
    if !args.yes && !ctx.confirm(&question, "--yes")? {
        return ctx.print("Bulk delete cancelled.\n");
    }
    let scope = ctx.scope()?;
    let targets: Vec<_> = ids
        .into_iter()
        .map(|original| {
            let id = crate::refs::resolve_document_reference(&original, &scope);
            (original, id)
        })
        .collect();
    let client = ctx.client()?;
    let show_progress = ctx.terminal().stderr_tty;
    let total = targets.len();
    let results = ctx.block_on(async {
        let mut rows = stream::iter(targets)
            .map(|(original, id)| delete_item(client, original, id))
            .buffered(5);
        let mut results: Vec<BulkResult> = Vec::with_capacity(total);
        while let Some(row) = rows.next().await {
            results.push(row);
            if show_progress {
                let progress = Progress {
                    completed: results.len(),
                    total,
                    succeeded: results.iter().filter(|row| row.succeeded()).count(),
                };
                ctx.eprint(progress.render())?;
            }
        }
        Ok::<_, Error>(results)
    })?;
    if show_progress {
        ctx.eprint(bulk::PROGRESS_CLEAR)?;
    }
    let (output, failed) = summary(&results);
    ctx.print(output)?;
    if failed {
        return Err(Error::reported());
    }
    Ok(())
}

/// One bulk row. Failures, including an unparseable reference, become the
/// row's message rather than stopping the other items.
async fn delete_item(
    client: &GraphQlTransport,
    original: String,
    id: Result<String>,
) -> BulkResult {
    let row = async {
        let Some(document) = details(client, &id?).await? else {
            return Ok(BulkResult {
                id: original.clone(),
                name: None,
                outcome: BulkOutcome::Failed("Document not found".to_owned()),
            });
        };
        submit(client, document.id.inner()).await?;
        Ok::<_, Error>(BulkResult {
            id: document.id.into_inner(),
            name: Some(document.title),
            outcome: BulkOutcome::Succeeded,
        })
    };
    row.await.unwrap_or_else(|error| BulkResult {
        id: original.clone(),
        name: None,
        outcome: BulkOutcome::Failed(error.message().to_owned()),
    })
}

async fn details(client: &GraphQlTransport, id: &str) -> Result<Option<DocumentDetails>> {
    let request = GraphQlRequest::with_variables(GetDocumentForDelete::build(IdVariables {
        id: id.to_owned(),
    }));
    let data: GetDocumentForDelete = client
        .execute(&request)
        .await
        .map_err(|failure| super::not_found(failure, id))?;
    Ok(data.document)
}

async fn submit(client: &GraphQlTransport, id: &str) -> Result<()> {
    let request =
        GraphQlRequest::with_variables(DeleteDocument::build(IdVariables { id: id.to_owned() }));
    let data: DeleteDocument = client.execute(&request).await?;
    if !data.document_delete.success {
        return Err(Error::new("Linear did not delete the document"));
    }
    Ok(())
}

fn summary(results: &[BulkResult]) -> (String, bool) {
    let total = results.len();
    let succeeded = results.iter().filter(|row| row.succeeded()).count();
    let failed = total - succeeded;
    let plural = |count: usize| if count == 1 { "" } else { "s" };
    let mut output = String::from("\n");
    if failed == 0 {
        output.push_str(&format!(
            "✓ Successfully deleted {succeeded} document{}\n",
            plural(succeeded)
        ));
        return (output, false);
    }
    if succeeded == 0 {
        output.push_str(&format!(
            "✗ Failed to delete all {total} document{}\n",
            plural(total)
        ));
    } else {
        output.push_str(&format!(
            "Completed: {succeeded}/{total} document{} deleted\n  ✓ Succeeded: {succeeded}\n  ✗ Failed: {failed}\n",
            plural(total)
        ));
    }
    output.push_str("\nFailed operations:\n");
    for row in results {
        if let BulkOutcome::Failed(error) = &row.outcome {
            let name = row
                .name
                .as_deref()
                .filter(|name| !name.is_empty())
                .map_or_else(String::new, |name| format!(" ({name})"));
            output.push_str(&format!("  - {}{name}: {error}\n", row.id));
        }
    }
    (output, true)
}
