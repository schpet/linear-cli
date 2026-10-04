//! `document comment add`: a comment or reply on a document.
use crate::cli::document::DocumentCommentAdd;
use crate::commands::comment_add::{self, CommentTarget};
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};

pub fn run(ctx: &Ctx, args: &DocumentCommentAdd) -> Result<()> {
    add(ctx, args).context("Failed to add comment")
}

fn add(ctx: &Ctx, args: &DocumentCommentAdd) -> Result<()> {
    let document = crate::commands::document::common::reference(ctx, &args.document)?;
    let body = comment_add::resolve_body(args.body.as_deref(), args.body_file.as_ref())?;
    comment_add::check_parent(args.reply_to.as_deref())?;
    if body.is_none() {
        comment_add::require_editor(ctx)?;
    }
    let client = ctx.client()?;
    let target = ctx.spin(true, async {
        let target = comment_add::document_target(client, &document).await?;
        if body.is_none() {
            comment_add::check_parent_exists(client, args.reply_to.as_deref()).await?;
        }
        Ok::<_, Error>(target)
    })?;
    let body = match body {
        Some(body) => body,
        None => {
            let question = comment_add::question(
                &format!("document \"{}\"", target.title),
                args.reply_to.as_deref(),
            );
            match comment_add::write_in_editor(ctx, "", args.confirm.yes, &question)? {
                Some(body) => body,
                None => return Ok(()),
            }
        }
    };
    // Linear attaches document comments to the document's content record.
    let target = CommentTarget::Document {
        document_content_id: target.document_content_id,
    };
    let input = comment_add::build_input(target, body, args.reply_to.as_deref(), None);
    let comment = ctx.spin(true, comment_add::create(client, input))?;
    ctx.print(comment_add::output("document", &document, &comment))
}
