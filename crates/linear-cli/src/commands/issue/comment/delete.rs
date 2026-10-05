//! `issue comment delete`: delete a comment by its UUID after confirmation.
use crate::cli::issue::IssueCommentDelete;
use crate::commands::table::truncate;
use crate::commands::{confirm, outcome};
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::comment::{
    CommentForDelete, DeleteComment, DeleteCommentVariables, GetCommentForDelete,
    GetCommentVariables,
};
use crate::platform::terminal_text::single_line;
use crate::refs::{reject_comment_url, reject_linear_url};

pub fn run(ctx: &Ctx, args: &IssueCommentDelete) -> Result<()> {
    delete(ctx, args).context("Failed to delete comment")
}

fn delete(ctx: &Ctx, args: &IssueCommentDelete) -> Result<()> {
    let id = &args.comment_id;
    reject_comment_url(id)?;
    reject_linear_url(id, "a comment UUID")?;
    let client = ctx.client()?;
    let data: GetCommentForDelete = ctx
        .spin(true, client.query(GetCommentVariables { id: id.clone() }))
        .map_err(|failure| failure.or_not_found("Comment", id))?;
    let comment = data
        .comment
        .ok_or_else(|| Error::not_found("Comment", id))?;
    if !confirm::proceed(ctx, args.confirm.yes, &question(&comment))? {
        return Ok(());
    }
    let result: DeleteComment = ctx.spin(
        true,
        client.mutate(DeleteCommentVariables { id: id.clone() }),
    )?;
    if !result.comment_delete.success {
        return Err(Error::new("Linear did not delete the comment"));
    }
    ctx.print(outcome::done("Deleted", "comment", id, None))
}

/// Names the comment by its issue and how it starts.
fn question(comment: &CommentForDelete) -> String {
    let start = truncate(&single_line(comment.body.trim()), 40);
    match &comment.issue {
        Some(issue) => format!("Delete comment \"{start}\" on {}?", issue.identifier),
        None => format!("Delete comment \"{start}\"?"),
    }
}
