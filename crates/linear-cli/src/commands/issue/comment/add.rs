//! `issue comment add`: a comment or reply, optionally with uploaded files.
use crate::cli::issue::IssueCommentAdd;
use crate::commands::comment_add::{self, CommentTarget};
use crate::commands::issue::attach;
use crate::commands::upload::{self, UploadedFile};
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};

pub fn run(ctx: &Ctx, args: &IssueCommentAdd) -> Result<()> {
    add(ctx, args).context("Failed to add comment")
}

fn add(ctx: &Ctx, args: &IssueCommentAdd) -> Result<()> {
    validate_comment_id(args.id.as_deref())?;
    let text = comment_add::resolve_body(args.body.as_deref(), args.body_file.as_ref())?;
    comment_add::check_parent(args.reply_to.as_deref())?;
    let identifier = crate::commands::issue::require(ctx, args.issue_id.as_deref())?;
    if args.public && args.attach.is_empty() {
        return Err(Error::new("--public requires at least one --attach")
            .with_hint("Add --attach <file> to upload, or remove --public."));
    }
    upload::prevalidate(&args.attach, args.public)?;
    let editor = text.is_none() && args.attach.is_empty();
    if editor {
        comment_add::require_editor(ctx)?;
    }
    let client = ctx.client()?;
    // The issue and the comment replied to must exist before anything is
    // typed for them, and a reply must answer a thread on this issue.
    if editor || args.reply_to.is_some() {
        let (issue_id, parent) = ctx.spin(true, async {
            tokio::try_join!(
                crate::commands::issue::id::fetch(client, &identifier),
                comment_add::fetch_parent(client, args.reply_to.as_deref()),
            )
        })?;
        if let Some(parent) = parent {
            parent.check(
                &CommentTarget::Issue { issue_id },
                &format!("issue {identifier}"),
            )?;
        }
    }
    let text = if editor {
        let question = comment_add::question(&identifier, args.reply_to.as_deref());
        match comment_add::write_in_editor(ctx, "", args.confirm.yes, &question)? {
            Some(body) => Some(body),
            None => return Ok(()),
        }
    } else {
        text
    };
    let files = args
        .attach
        .iter()
        .map(|path| attach::upload_file(ctx, path, args.public))
        .collect::<Result<Vec<_>>>()?;
    let body = compose_body(text.as_deref(), &files);
    let input = comment_add::build_input(
        CommentTarget::Issue {
            issue_id: identifier.clone(),
        },
        body,
        args.reply_to.as_deref(),
        args.id.as_deref(),
    );
    let comment = ctx.spin(true, comment_add::create(client, input))?;
    ctx.print(comment_add::output(
        "issue",
        &identifier,
        args.reply_to.as_deref(),
        &comment,
    ))
}

fn validate_comment_id(id: Option<&str>) -> Result<(), Error> {
    if let Some(id) = id {
        let bytes = id.as_bytes();
        if !(crate::refs::is_linear_uuid(id)
            && bytes.get(14) == Some(&b'4')
            && matches!(bytes.get(19), Some(b'8' | b'9' | b'a' | b'A' | b'b' | b'B')))
        {
            return Err(Error::new(format!("Invalid comment ID: {id}"))
                .with_hint("--id must be a v4 UUID, like 123e4567-e89b-42d3-a456-426614174000."));
        }
    }
    Ok(())
}

fn compose_body(text: Option<&str>, files: &[UploadedFile]) -> String {
    let links = files
        .iter()
        .map(upload::markdown)
        .collect::<Vec<_>>()
        .join("\n");
    [text.unwrap_or(""), &links]
        .into_iter()
        .filter(|x| !x.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}
