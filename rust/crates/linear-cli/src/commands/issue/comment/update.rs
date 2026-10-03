//! `issue comment update`: body from a flag, a file or a prompt, then one mutation.
use crate::client::LinearClient;
use crate::{
    cli::issue::IssueCommentUpdate,
    ctx::Ctx,
    error::{Error, Result, ResultExt},
    graphql::operations::comment::*,
    platform::prompt::Text,
    refs::{reject_comment_url, reject_linear_url},
};

pub fn run(ctx: &Ctx, args: &IssueCommentUpdate) -> Result<()> {
    update(ctx, args).context("Failed to update comment")
}

fn update(ctx: &Ctx, args: &IssueCommentUpdate) -> Result<()> {
    let id = &args.comment_id;
    let body = prepare_body(id, args.body.as_deref(), args.body_file.as_deref())?;
    let body = body.filter(|body| !needs_prompt(Some(body)));
    if body.is_none() && !ctx.interactive() {
        return Err(Error::new("No comment body given")
            .with_hint("Pass --body or --body-file, or run in a terminal to be prompted."));
    }
    let client = ctx.client()?;
    let body = match body {
        Some(body) => body,
        None => {
            let existing = ctx.spin(true, existing_body(client, id))?;
            ctx.prompter()?.text(
                Text::new("New comment body")
                    .required()
                    .with_default(&existing),
            )?
        }
    };
    ctx.print(ctx.spin(true, submit(client, id, body))?)
}
/// URL guards and local body/file work happen before the client is built.
pub fn prepare_body(
    id: &str,
    body: Option<&str>,
    file: Option<&str>,
) -> Result<Option<String>, Error> {
    reject_comment_url(id)?;
    reject_linear_url(id, "a comment UUID")?;
    let file = file.filter(|value| !value.is_empty());
    if body.is_some_and(|value| !value.is_empty()) && file.is_some() {
        return Err(Error::new("Cannot specify both --body and --body-file"));
    }
    match file {
        Some(path) => crate::commands::text_input::read_file(path)
            .map(Some)
            .map_err(|error| {
                Error::new(format!("Failed to read body file: {path}"))
                    .with_hint(format!("Error: {error}"))
                    .with_source(error)
            }),
        None => Ok(body.map(str::to_owned)),
    }
}
pub fn needs_prompt(body: Option<&str>) -> bool {
    body.is_none_or(str::is_empty)
}
pub async fn existing_body(client: &LinearClient, id: &str) -> Result<String, Error> {
    let result: GetComment = client
        .query(GetCommentVariables { id: id.to_owned() })
        .await?;
    Ok(result
        .comment
        .and_then(|comment| comment.body)
        .unwrap_or_default())
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
    Ok(format!("✓ Comment updated\n{}\n", comment.url).into_bytes())
}
