//! `issue comment delete`: delete a comment by its UUID.
use crate::cli::issue::IssueCommentDelete;
use crate::commands::outcome;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::comment::{DeleteComment, DeleteCommentVariables};
use crate::refs::{reject_comment_url, reject_linear_url};

pub fn run(ctx: &Ctx, args: &IssueCommentDelete) -> Result<()> {
    delete(ctx, args).context("Failed to delete comment")
}

fn delete(ctx: &Ctx, args: &IssueCommentDelete) -> Result<()> {
    let id = &args.comment_id;
    reject_comment_url(id)?;
    reject_linear_url(id, "a comment UUID")?;
    let client = ctx.client()?;
    let result: DeleteComment = ctx.spin(
        true,
        client.mutate(DeleteCommentVariables { id: id.clone() }),
    )?;
    if !result.comment_delete.success {
        return Err(Error::new("Linear did not delete the comment"));
    }
    ctx.print(outcome::done("Deleted", "comment", id, None))
}
