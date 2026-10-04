//! `initiative comment add`: a comment or reply on an initiative.
use crate::cli::initiative::InitiativeCommentAdd;
use crate::commands::comment_add::{self, CommentTarget};
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::refs::{self, initiative::Archived};

pub fn run(ctx: &Ctx, args: &InitiativeCommentAdd) -> Result<()> {
    add(ctx, args).context("Failed to add comment")
}

fn add(ctx: &Ctx, args: &InitiativeCommentAdd) -> Result<()> {
    let original = &args.initiative;
    let body = comment_add::resolve_body(args.body.as_deref(), args.body_file.as_ref())?;
    let reference = crate::commands::initiative::common::reference(ctx, original)?;
    comment_add::check_parent(args.reply_to.as_deref())?;
    if body.is_none() {
        comment_add::require_editor(ctx)?;
    }
    let client = ctx.client()?;
    let (initiative_id, name) = ctx.spin(true, async {
        let (id, parent) = tokio::try_join!(
            refs::initiative::resolve(client, &reference, Archived::Exclude),
            comment_add::fetch_parent(client, args.reply_to.as_deref()),
        )?;
        if let Some(parent) = parent {
            let target = CommentTarget::Initiative {
                initiative_id: id.clone(),
            };
            parent.check(&target, &format!("initiative {original}"))?;
        }
        let name = match body {
            Some(_) => None,
            None => Some(refs::initiative::name(client, &id).await?),
        };
        Ok::<_, Error>((id, name))
    })?;
    let body = match body {
        Some(body) => body,
        None => {
            let name = name.expect("the name is looked up for the editor");
            let question =
                comment_add::question(&format!("initiative \"{name}\""), args.reply_to.as_deref());
            match comment_add::write_in_editor(ctx, "", args.confirm.yes, &question)? {
                Some(body) => body,
                None => return Ok(()),
            }
        }
    };
    let target = CommentTarget::Initiative { initiative_id };
    let input = comment_add::build_input(target, body, args.reply_to.as_deref(), None);
    let comment = ctx.spin(true, comment_add::create(client, input))?;
    ctx.print(comment_add::output(
        "initiative",
        original,
        args.reply_to.as_deref(),
        &comment,
    ))
}
