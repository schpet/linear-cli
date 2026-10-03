//! `initiative comment add`: a comment or reply on an initiative.
use crate::cli::initiative::InitiativeCommentAdd;
use crate::commands::comment_add::{self, CommentTarget};
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};

pub fn run(ctx: &Ctx, args: &InitiativeCommentAdd) -> Result<()> {
    add(ctx, args).context("Failed to add comment")
}

fn add(ctx: &Ctx, args: &InitiativeCommentAdd) -> Result<()> {
    let original = &args.initiative;
    let body = comment_add::resolve_body(args.body.as_deref(), args.body_file.as_deref())?;
    let reference = super::reference(ctx, original)?;
    comment_add::check_parent(args.parent.as_deref())?;
    let body = match body {
        Some(body) => body,
        None => comment_add::prompt(ctx)?,
    };
    let client = ctx.client()?;
    let comment = ctx.spin(true, async {
        let initiative_id =
            super::resolve(client, &reference, original, super::Archived::Exclude).await?;
        let target = CommentTarget::Initiative { initiative_id };
        let input = comment_add::build_input(target, body, args.parent.as_deref(), None);
        comment_add::create(client, input).await
    })?;
    ctx.print(comment_add::output("initiative", original, &comment))
}
