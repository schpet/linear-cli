//! `initiative comment add`: a comment or reply on an initiative.
use crate::cli::initiative::InitiativeCommentAdd;
use crate::commands::comment_add::{self, CommentTarget};
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};
use crate::refs::{self, initiative::Archived};

pub fn run(ctx: &Ctx, args: &InitiativeCommentAdd) -> Result<()> {
    add(ctx, args).context("Failed to add comment")
}

fn add(ctx: &Ctx, args: &InitiativeCommentAdd) -> Result<()> {
    let original = &args.initiative;
    let body = comment_add::resolve_body(args.body.as_deref(), args.body_file.as_deref())?;
    let reference = crate::commands::initiative::common::reference(ctx, original)?;
    comment_add::check_parent(args.reply_to.as_deref())?;
    if body.is_none() {
        comment_add::require_editor(ctx)?;
    }
    let client = ctx.client()?;
    let initiative_id = ctx.spin(
        true,
        refs::initiative::resolve(client, &reference, Archived::Exclude),
    )?;
    let body = match body {
        Some(body) => body,
        None => {
            let question = format!("Post this comment on initiative {original}?");
            match comment_add::write_in_editor(ctx, "", args.confirm.yes, &question)? {
                Some(body) => body,
                None => return Ok(()),
            }
        }
    };
    let target = CommentTarget::Initiative { initiative_id };
    let input = comment_add::build_input(target, body, args.reply_to.as_deref(), None);
    let comment = ctx.spin(true, comment_add::create(client, input))?;
    ctx.print(comment_add::output("initiative", original, &comment))
}
