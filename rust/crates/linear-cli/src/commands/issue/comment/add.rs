//! `issue comment add`: a comment or reply, optionally with uploaded files.
use crate::cli::issue::IssueCommentAdd;
use crate::commands::comment_add::{self, CommentTarget};
use crate::commands::issue::attach;
use crate::commands::upload;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};

pub fn run(ctx: &Ctx, args: &IssueCommentAdd) -> Result<()> {
    add(ctx, args).context("Failed to add comment")
}

fn add(ctx: &Ctx, args: &IssueCommentAdd) -> Result<()> {
    attach::validate_comment_id(args.id.as_deref())?;
    let text = comment_add::resolve_body(args.body.as_deref(), args.body_file.as_deref())?;
    let identifier = crate::commands::issue::require(ctx, args.issue_id.as_deref())?;
    if args.public && args.attach.is_empty() {
        return Err(Error::new("--public requires at least one --attach")
            .with_hint("Add --attach <file> to upload, or remove --public."));
    }
    upload::prevalidate(&args.attach, args.public)?;
    let text = match text {
        None if args.attach.is_empty() => Some(comment_add::prompt(ctx)?),
        text => text,
    };
    let files = args
        .attach
        .iter()
        .map(|path| attach::upload_file(ctx, path, args.public))
        .collect::<Result<Vec<_>>>()?;
    let body = attach::compose_body(text.as_deref(), &files);
    let input = comment_add::build_input(
        CommentTarget::Issue {
            issue_id: identifier.clone(),
        },
        body,
        args.parent.as_deref(),
        args.id.as_deref(),
    )?;
    let client = ctx.client()?;
    let comment = ctx.spin(true, comment_add::create(client, input))?;
    ctx.print(attach::comment_output(&identifier, &comment.url))
}
