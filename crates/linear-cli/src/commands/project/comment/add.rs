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
    comment_add::check_parent(args.reply_to.as_deref())?;
    let reference = ProjectReference::parse(original, &ctx.scope()?)?;
    if body.is_none() {
        comment_add::require_editor(ctx)?;
    }
    let client = ctx.client()?;
    let project_id = ctx.spin(true, refs::project::resolve(client, &reference))?;
    let body = match body {
        Some(body) => body,
        None => {
            let question = format!("Post this comment on project {original}?");
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
