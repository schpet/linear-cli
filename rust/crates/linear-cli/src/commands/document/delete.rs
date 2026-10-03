//! `document delete`: move one document, or many in bulk, to the trash.
use crate::cli::document::DocumentDelete;
use crate::client::LinearClient;
use crate::commands::bulk::{self, BulkInput, BulkOutcome, BulkResult, Verb};
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::document_delete::{
    DeleteDocument, DocumentDetails, GetDocumentForDelete, IdVariables,
};

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
    let results = bulk::run(ctx, targets, |(original, id)| {
        delete_item(client, original, id)
    })?;
    bulk::report(
        ctx,
        &results,
        "document",
        Verb {
            present: "delete",
            past: "deleted",
        },
    )
}

/// One bulk row. Failures, including an unparseable reference, become the
/// row's message rather than stopping the other items.
async fn delete_item(client: &LinearClient, original: String, id: Result<String>) -> BulkResult {
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

async fn details(client: &LinearClient, id: &str) -> Result<Option<DocumentDetails>> {
    let data: GetDocumentForDelete = client
        .query(IdVariables { id: id.to_owned() })
        .await
        .map_err(|failure| super::not_found(failure, id))?;
    Ok(data.document)
}

async fn submit(client: &LinearClient, id: &str) -> Result<()> {
    let data: DeleteDocument = client.mutate(IdVariables { id: id.to_owned() }).await?;
    if !data.document_delete.success {
        return Err(Error::new("Linear did not delete the document"));
    }
    Ok(())
}
