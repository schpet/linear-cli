//! `issue comment update`: body from a flag, a file or a prompt, then one mutation.
use crate::{
    error::{AppError, AppErrorKind},
    graphql::{
        bulk_error,
        envelope::{GraphQlRequest, ResponseError},
        operations::comment_update::*,
        transport::{GraphQlTransport, TransportFailure, classify_typed},
    },
    platform::{
        prompt::{PromptOutcome, PromptSession},
        prompt_text::TextOptions,
    },
    refs::{reject_comment_url, reject_linear_url},
};
use cynic::{MutationBuilder, QueryBuilder};
use serde::{Serialize, de::DeserializeOwned};
use std::io::{Read, Write};

pub const CONTEXT: &str = "Failed to update comment";
/// URL guards and local body/file work happen before transport construction.
pub fn prepare_body(
    id: &str,
    body: Option<&str>,
    file: Option<&str>,
) -> Result<Option<String>, AppError> {
    reject_comment_url(id)?;
    reject_linear_url(id, "a comment UUID")?;
    let file = file.filter(|value| !value.is_empty());
    if body.is_some_and(|value| !value.is_empty()) && file.is_some() {
        return Err(AppError::new(
            AppErrorKind::Validation,
            "Cannot specify both --body and --body-file",
        ));
    }
    match file {
        Some(path) => crate::commands::text_input::read_file(path)
            .map(Some)
            .map_err(|error| {
                AppError::new(
                    AppErrorKind::Validation,
                    format!("Failed to read body file: {path}"),
                )
                .with_suggestion(format!("Error: {error}"))
                .with_source(error)
            }),
        None => Ok(body.map(str::to_owned)),
    }
}
pub fn needs_prompt(body: Option<&str>) -> bool {
    body.is_none_or(str::is_empty)
}
/// Refuses prompting when stdout is a FIFO; checked after the fetch and before raw mode.
/// Pipe stdin and regular redirected files remain eligible for their native prompts.
pub fn check_prompt_topology(stdin_tty: bool, stdout_fifo: bool) -> Result<(), AppError> {
    if stdin_tty && stdout_fifo {
        return Err(AppError::new(
            AppErrorKind::Validation,
            "Comment text prompt requires terminal or regular-file stdout when stdin is a terminal",
        ).with_suggestion("Keep stdout on the terminal, redirect it to a regular file, or supply --body/--body-file."));
    }
    Ok(())
}
#[cfg(unix)]
pub fn stdout_is_fifo() -> Result<bool, AppError> {
    let stat = rustix::fs::fstat(std::io::stdout()).map_err(|source| {
        AppError::new(
            AppErrorKind::IoProcess,
            "Failed to inspect comment text prompt stdout",
        )
        .with_source(source)
    })?;
    Ok(rustix::fs::FileType::from_raw_mode(stat.st_mode) == rustix::fs::FileType::Fifo)
}
#[cfg(not(unix))]
pub fn stdout_is_fifo() -> Result<bool, AppError> {
    Ok(false)
}
pub fn get_request(id: &str) -> GraphQlRequest<GetCommentVariables> {
    GraphQlRequest::with_variables(GetComment::build(GetCommentVariables { id: id.to_owned() }))
}
pub fn update_request(id: &str, body: String) -> GraphQlRequest<UpdateCommentVariables> {
    GraphQlRequest::with_variables(UpdateComment::build(UpdateCommentVariables {
        id: id.to_owned(),
        input: CommentUpdateInput { body },
    }))
}
/// Report a GraphQL error from the exchange first, then decode the whole result. Never decode partial JSON to confirm a mutation.
async fn exchange<T: DeserializeOwned, V: Serialize>(
    transport: &GraphQlTransport,
    request: &GraphQlRequest<V>,
    mutation: bool,
) -> Result<T, AppError> {
    let response = transport.send_request(request).await?;
    if let Some(error) =
        bulk_error::observe_source_error(&response, request).map_err(|error| error.into_error())?
    {
        return Err(AppError::new(
            AppErrorKind::GraphQl,
            error.preferred_message.unwrap_or(error.message),
        ));
    }
    classify_typed(response).map_err(|error| match error {
        TransportFailure::Response(ResponseError::UnexpectedShape(source)) => AppError::new(
            AppErrorKind::Invariant,
            format!(
                "Linear returned an unexpected response: {source}{}",
                if mutation {
                    "; update outcome unknown; do not retry automatically"
                } else {
                    "; no update attempted"
                }
            ),
        )
        .with_source(source),
        error => AppError::from(error),
    })
}
pub async fn existing_body(transport: &GraphQlTransport, id: &str) -> Result<String, AppError> {
    let result: GetComment = exchange(transport, &get_request(id), false).await?;
    Ok(result
        .comment
        .and_then(|comment| comment.body)
        .unwrap_or_default())
}
pub fn prompt_body<R: Read, W: Write>(
    session: &mut PromptSession<R, W>,
    existing: &str,
) -> Result<PromptOutcome<String>, AppError> {
    match session.text_with_display_default(
        "New comment body",
        TextOptions {
            required: false,
            default: Some(existing),
        },
    )? {
        PromptOutcome::Submitted(body) => {
            if body.trim().is_empty() {
                Err(AppError::new(
                    AppErrorKind::Validation,
                    "Comment body cannot be empty",
                ))
            } else {
                Ok(PromptOutcome::Submitted(body))
            }
        }
        PromptOutcome::Interrupted => Ok(PromptOutcome::Interrupted),
        PromptOutcome::EndOfInput => Err(AppError::new(
            AppErrorKind::Validation,
            "unexpected EOF while prompting for comment body",
        )),
    }
}
pub async fn submit(
    transport: &GraphQlTransport,
    id: &str,
    body: String,
) -> Result<Vec<u8>, AppError> {
    let result: UpdateComment = exchange(transport, &update_request(id, body), true).await?;
    if !result.comment_update.success {
        return Err(AppError::new(AppErrorKind::GraphQl, CONTEXT));
    }
    // `success: true` with a null comment means the update happened but its
    // result is unavailable; report that without retrying.
    let comment = result.comment_update.comment.ok_or_else(|| {
        AppError::new(
            AppErrorKind::GraphQl,
            "Comment update failed - no comment returned",
        )
    })?;
    Ok(format!("✓ Comment updated\n{}\n", comment.url).into_bytes())
}
