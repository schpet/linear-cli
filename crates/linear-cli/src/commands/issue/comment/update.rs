//! `issue comment update`: body from a flag, a file or the editor, then one mutation.
use crate::cli::values::TextSource;
use crate::client::LinearClient;
use crate::{
    cli::issue::IssueCommentUpdate,
    commands::{comment_add, outcome},
    ctx::Ctx,
    error::{Error, Result, ResultExt},
    graphql::operations::comment::*,
    refs::{reject_comment_url, reject_linear_url},
};

pub fn run(ctx: &Ctx, args: &IssueCommentUpdate) -> Result<()> {
    update(ctx, args).context("Failed to update comment")
}

fn update(ctx: &Ctx, args: &IssueCommentUpdate) -> Result<()> {
    let id = &args.comment_id;
    let body = prepare_body(id, args.body.as_deref(), args.body_file.as_ref())?;
    if body.is_none() {
        comment_add::require_editor(ctx)?;
    }
    let client = ctx.client()?;
    let body = match body {
        Some(body) => body,
        None => {
            let existing = ctx.spin(true, existing_body(client, id))?;
            match comment_add::write_in_editor(
                ctx,
                &existing,
                args.confirm.yes,
                "Save the edited comment?",
            )? {
                Some(body) => body,
                None => return Ok(()),
            }
        }
    };
    ctx.print(ctx.spin(true, submit(client, id, body))?)
}
/// URL guards and local body/file work happen before the client is built.
pub fn prepare_body(
    id: &str,
    body: Option<&str>,
    file: Option<&TextSource>,
) -> Result<Option<String>, Error> {
    reject_comment_url(id)?;
    reject_linear_url(id, "a comment UUID")?;
    comment_add::resolve_body(body, file)
}
pub async fn existing_body(client: &LinearClient, id: &str) -> Result<String, Error> {
    let result: GetComment = client
        .query(GetCommentVariables { id: id.to_owned() })
        .await
        .map_err(|failure| failure.or_not_found("Comment", id))?;
    let comment = result
        .comment
        .ok_or_else(|| Error::not_found("Comment", id))?;
    Ok(comment.body.unwrap_or_default())
}
pub async fn submit(client: &LinearClient, id: &str, body: String) -> Result<Vec<u8>, Error> {
    let result: UpdateComment = client
        .mutate(UpdateCommentVariables {
            id: id.to_owned(),
            input: CommentUpdateInput { body },
        })
        .await
        .map_err(|failure| {
            let uncertain = failure.outcome_unknown();
            let mut error = Error::from(failure);
            if uncertain {
                error.push_message("; the comment may already be updated");
            }
            error
        })?;
    if !result.comment_update.success {
        return Err(Error::new("Linear did not update the comment"));
    }
    // `success: true` with a null comment means the update happened but its
    // result is unavailable; report that without retrying.
    let comment = result
        .comment_update
        .comment
        .ok_or_else(|| Error::new("Comment update failed - no comment returned"))?;
    Ok(outcome::done("Updated", "comment", id, Some(&comment.url)).into_bytes())
}

#[cfg(test)]
mod tests;
