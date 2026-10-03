//! `document comment add`: a comment or reply on a document.
use crate::cli::document::DocumentCommentAdd;
use crate::commands::comment_add::{self, CommentTarget};
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};

pub fn run(ctx: &Ctx, args: &DocumentCommentAdd) -> Result<()> {
    add(ctx, args).context("Failed to add comment")
}

fn add(ctx: &Ctx, args: &DocumentCommentAdd) -> Result<()> {
    let document = super::reference(ctx, &args.document)?;
    let body = comment_add::resolve_body(args.body.as_deref(), args.body_file.as_deref())?;
    comment_add::check_parent(args.parent.as_deref())?;
    let body = match body {
        Some(body) => body,
        None => comment_add::prompt(ctx)?,
    };
    let client = ctx.client()?;
    let comment = ctx.spin(true, async {
        // Linear attaches document comments to the document's content record.
        let document_content_id = comment_add::document_content_id(client, &document).await?;
        let target = CommentTarget::Document {
            document_content_id,
        };
        let input = comment_add::build_input(target, body, args.parent.as_deref(), None);
        comment_add::create(client, input).await
    })?;
    ctx.print(comment_add::output("document", &document, &comment))
}
