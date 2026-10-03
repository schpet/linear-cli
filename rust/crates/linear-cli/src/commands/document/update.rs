//! `document update`: new fields from flags, stdin or an editor, then one
//! mutation.
use crate::cli::document::DocumentUpdate;
use crate::client::LinearClient;
use crate::commands::outcome;
use crate::commands::text_input;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::document::*;
use crate::graphql::pagination::{self, Page};

use super::common::{self, attach, read_file};
use super::target::{self, TargetOptions};

pub fn run(ctx: &Ctx, args: &DocumentUpdate) -> Result<()> {
    update(ctx, args).context("Failed to update document")
}

fn update(ctx: &Ctx, args: &DocumentUpdate) -> Result<()> {
    let id = common::reference(ctx, &args.document_id)?;
    let target = target::prepare(
        ctx,
        TargetOptions {
            project: args.project.as_deref(),
            issue: args.issue.as_deref(),
            initiative: args.initiative.as_deref(),
            team: args.team.as_deref(),
            cycle: args.cycle.as_deref(),
            release: args.release.as_deref(),
        },
    )?;
    let metadata = args.title.is_some() || args.icon.is_some() || target.is_some();
    let content = match (&args.content, &args.content_file) {
        (Some(content), _) => Some(content.clone()),
        (None, Some(path)) => Some(read_file(path)?),
        // Piped stdin is the new content when nothing else is being changed.
        (None, None) if !args.edit && !metadata && !ctx.stdin_tty() => {
            text_input::read_stdin(std::io::stdin().lock())?
        }
        (None, None) => None,
    };
    let edit = args.edit && content.is_none();
    if content.is_none() && !edit && !metadata {
        return Err(Error::new("No update fields provided").with_hint(
            "Use --title, --content, --content-file, --icon, --edit, or re-point the attachment with --project, --issue, --initiative, --team, --cycle, or --release.",
        ));
    }
    let client = ctx.client()?;
    let mut input = DocumentUpdateInput {
        title: args.title.clone(),
        icon: args.icon.clone(),
        content,
        ..Default::default()
    };
    if target.is_some() {
        ctx.spin(true, attach(client, &mut input, target.as_ref()))?;
    }
    if edit {
        let document = ctx.spin(true, for_edit(client, &id))?;
        let seed = document.content.unwrap_or_default();
        ctx.print(format!("Opening {} in editor...\n", document.title))?;
        let edited = ctx.edit_text(&seed)?;
        if edited == seed {
            return ctx.print("No changes made; the document is unchanged.\n");
        }
        let Some(content) = text_input::edited_body(&edited) else {
            return ctx.print("No changes made; the document is unchanged.\n");
        };
        input.content = Some(content);
    }
    let updated = ctx.spin(true, async {
        if input.content.is_some() && !args.force {
            refuse_inline_comments(client, &id).await?;
        }
        let data: UpdateDocument = client
            .mutate(UpdateDocumentVariables {
                id: id.clone(),
                input,
            })
            .await?;
        if !data.document_update.success {
            return Err(Error::new("Linear did not update the document"));
        }
        Ok(data.document_update.document)
    })?;
    ctx.print(outcome::done(
        "Updated",
        "document",
        &updated.title,
        Some(&updated.url),
    ))
}

async fn for_edit(client: &LinearClient, id: &str) -> Result<DocumentForEdit> {
    let data: GetDocumentForEdit = client
        .query(DocumentEditVariables { id: id.to_owned() })
        .await
        .map_err(|failure| common::not_found(failure, id))?;
    data.document
        .ok_or_else(|| Error::not_found("Document", id))
}

/// Replacing the Markdown can detach or hide inline comments, so content
/// updates stop while any open comment quotes the document.
async fn refuse_inline_comments(client: &LinearClient, id: &str) -> Result<()> {
    let comments = pagination::collect(None, |after, _first| {
        let variables = DocumentGuardVariables {
            id: id.to_owned(),
            after,
        };
        async move {
            let data: DocumentInlineCommentGuard = client
                .query(variables)
                .await
                .map_err(|failure| common::not_found(failure, id))?;
            let document = data
                .document
                .ok_or_else(|| Error::not_found("Document", id))?;
            Ok::<_, Error>(Page {
                nodes: document.comments.nodes,
                page_info: document.comments.page_info,
            })
        }
    })
    .await?;
    let open_quote = comments.into_iter().find_map(|comment| {
        let open = comment.resolved_at.is_none() && comment.archived_at.is_none();
        comment
            .quoted_text
            .filter(|_| open)
            .map(|quoted| (comment.id, quoted))
    });
    match open_quote {
        None => Ok(()),
        Some((comment, quoted)) => Err(Error::new(
            "Refusing to update document content because this document has inline comments.",
        )
        .with_hint(format!(
            "Updating Markdown content can detach or hide Linear document comments. First review comment {} quoting \"{quoted}\", then rerun with --force if you accept that risk.",
            comment.inner()
        ))),
    }
}
