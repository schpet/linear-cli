//! `project comment add`: a comment or reply on a project's discussion.
use crate::cli::project::ProjectCommentAdd;
use crate::commands::comment_add::{self, CommentTarget};
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};
use crate::refs::{self, project::ProjectReference};

pub fn run(ctx: &Ctx, args: &ProjectCommentAdd) -> Result<()> {
    add(ctx, args).context("Failed to add comment")
}

fn add(ctx: &Ctx, args: &ProjectCommentAdd) -> Result<()> {
    let original = &args.project;
    let body = comment_add::resolve_body(args.body.as_deref(), args.body_file.as_deref())?;
    comment_add::check_parent(args.parent.as_deref())?;
    let reference = ProjectReference::parse(original, &ctx.scope()?)?;
    let body = match body {
        Some(body) => body,
        None => comment_add::prompt(ctx)?,
    };
    let client = ctx.client()?;
    let comment = ctx.spin(true, async {
        let project_id = refs::project::resolve(client, &reference).await?;
        let input = comment_add::build_input(
            CommentTarget::Project { project_id },
            body,
            args.parent.as_deref(),
            None,
        );
        comment_add::create(client, input).await
    })?;
    ctx.print(comment_add::output("project", original, &comment))
}
