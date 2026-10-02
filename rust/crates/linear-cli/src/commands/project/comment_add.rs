//! `project comment add`: a comment or reply on a project's discussion.
use crate::cli::project::ProjectCommentAdd;
use crate::commands::comment_add::{self, CommentTarget};
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};
use crate::refs::{prepare_project_lookup, resolve_project_with_transport};

pub fn run(ctx: &Ctx, args: &ProjectCommentAdd) -> Result<()> {
    add(ctx, args).context("Failed to add comment")
}

fn add(ctx: &Ctx, args: &ProjectCommentAdd) -> Result<()> {
    let original = &args.project;
    let body = comment_add::resolve_body(args.body.as_deref(), args.body_file.as_deref())?;
    let reference = prepare_project_lookup(original, &ctx.scope()?)?;
    let body = match body {
        Some(body) => body,
        None => comment_add::prompt(ctx)?,
    };
    let client = ctx.client()?;
    let comment = ctx.spin(true, async {
        let project_id = resolve_project_with_transport(&reference, original, client).await?;
        let input = comment_add::build_input(
            CommentTarget::Project { project_id },
            body,
            args.parent.as_deref(),
            None,
        )?;
        comment_add::create(client, input).await
    })?;
    ctx.print(comment_add::output("project", original, &comment))
}
