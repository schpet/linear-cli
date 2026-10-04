//! `project comment add`: a comment or reply on a project's discussion.
use crate::cli::project::ProjectCommentAdd;
use crate::commands::comment_add::{self, CommentTarget};
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::refs::{self, project::ProjectReference};

pub fn run(ctx: &Ctx, args: &ProjectCommentAdd) -> Result<()> {
    add(ctx, args).context("Failed to add comment")
}

fn add(ctx: &Ctx, args: &ProjectCommentAdd) -> Result<()> {
    let original = &args.project;
    let body = comment_add::resolve_body(args.body.as_deref(), args.body_file.as_ref())?;
    comment_add::check_parent(args.reply_to.as_deref())?;
    let reference = ProjectReference::parse(original, &ctx.scope()?)?;
    if body.is_none() {
        comment_add::require_editor(ctx)?;
    }
    let client = ctx.client()?;
    let (project_id, name) = ctx.spin(true, async {
        let id = refs::project::resolve(client, &reference).await?;
        if body.is_some() {
            return Ok::<_, Error>((id, None));
        }
        let (name, ()) = tokio::try_join!(
            refs::project::name(client, &id),
            comment_add::check_parent_exists(client, args.reply_to.as_deref()),
        )?;
        Ok((id, Some(name)))
    })?;
    let body = match body {
        Some(body) => body,
        None => {
            let name = name.expect("the name is looked up for the editor");
            let question =
                comment_add::question(&format!("project \"{name}\""), args.reply_to.as_deref());
            match comment_add::write_in_editor(ctx, "", args.confirm.yes, &question)? {
                Some(body) => body,
                None => return Ok(()),
            }
        }
    };
    let input = comment_add::build_input(
        CommentTarget::Project { project_id },
        body,
        args.reply_to.as_deref(),
        None,
    );
    let comment = ctx.spin(true, comment_add::create(client, input))?;
    ctx.print(comment_add::output("project", original, &comment))
}
