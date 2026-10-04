//! `document delete`: move one document, or many in bulk, to the trash.
use crate::cli::document::DocumentDelete;
use crate::client::LinearClient;
use crate::commands::bulk::{self, BulkInput, BulkResult, Found, Skipped, Verb};
use crate::commands::outcome;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::common::IdVariables;
use crate::graphql::operations::document::{DeleteDocument, DocumentDetails, GetDocumentForDelete};
use crate::refs;

pub fn run(ctx: &Ctx, args: &DocumentDelete) -> Result<()> {
    delete(ctx, args).context("Failed to delete document")
}

fn delete(ctx: &Ctx, args: &DocumentDelete) -> Result<()> {
    if !args.confirm.yes {
        ctx.require_tty("for confirmation", "--yes")?;
    }
    let input = BulkInput::from(&args.bulk);
    if input.requested() {
        return delete_bulk(ctx, args, &input);
    }
    let original = args.document_id.as_deref().ok_or_else(|| {
        Error::invalid("Document ID required").with_hint("Use --bulk for multiple documents.")
    })?;
    let id = super::common::reference(ctx, original)?;
    let client = ctx.client()?;
    let document = ctx.spin(true, async {
        details(client, &id)
            .await?
            .ok_or_else(|| Error::not_found("Document", original))
    })?;
    let question = format!("Are you sure you want to delete \"{}\"?", document.title);
    if !args.confirm.yes && !ctx.confirm(&question, "--yes")? {
        return outcome::canceled(ctx);
    }
    ctx.spin(true, submit(client, document.id.inner()))?;
    ctx.print(outcome::done("Deleted", "document", &document.title, None))
}

fn delete_bulk(ctx: &Ctx, args: &DocumentDelete, input: &BulkInput<'_>) -> Result<()> {
    let ids = bulk::collect_ids(input, &mut std::io::stdin().lock())?;
    if ids.is_empty() {
        return Err(Error::new("No document IDs provided for bulk delete"));
    }
    let scope = ctx.scope()?;
    let targets: Vec<_> = ids
        .into_iter()
        .map(|original| {
            let id = refs::document::parse(&original, &scope);
            (original, id)
        })
        .collect();
    let client = ctx.client()?;
    let (found, missing) = bulk::look_up(ctx, targets, |(original, id)| {
        look_up_item(client, original, id)
    });
    let verb = Verb {
        present: "delete",
        past: "deleted",
    };
    ctx.print(bulk::preview(&found, &missing, "document", verb))?;
    if found.is_empty() {
        return Err(Error::new("None of the listed documents could be found"));
    }
    let question = format!("Delete {}?", bulk::count(found.len(), "document"));
    if !args.confirm.yes && !ctx.confirm(&question, "--yes")? {
        return outcome::canceled(ctx);
    }
    let mut results = bulk::run(ctx, found, |found| async move {
        let outcome = submit(client, &found.item).await;
        found.result(outcome)
    })?;
    results.extend(missing.into_iter().map(BulkResult::from));
    bulk::report(ctx, &results, "document", verb)
}

/// Looks up one listed document; the found item is its UUID.
async fn look_up_item(
    client: &LinearClient,
    original: String,
    id: Result<String>,
) -> std::result::Result<Found<String>, Skipped> {
    let document = async { details(client, &id?).await }.await;
    match document {
        Ok(Some(document)) => Ok(Found {
            original,
            name: document.title,
            item: document.id.into_inner(),
        }),
        Ok(None) => Err(Skipped {
            original,
            reason: "Document not found".to_owned(),
        }),
        Err(error) => Err(Skipped {
            original,
            reason: error.message().to_owned(),
        }),
    }
}

async fn details(client: &LinearClient, id: &str) -> Result<Option<DocumentDetails>> {
    let data: GetDocumentForDelete = client
        .query(IdVariables { id: id.to_owned() })
        .await
        .map_err(|failure| super::common::not_found(failure, id))?;
    Ok(data.document)
}

async fn submit(client: &LinearClient, id: &str) -> Result<()> {
    let data: DeleteDocument = client.mutate(IdVariables { id: id.to_owned() }).await?;
    if !data.document_delete.success {
        return Err(Error::new("Linear did not delete the document"));
    }
    Ok(())
}
