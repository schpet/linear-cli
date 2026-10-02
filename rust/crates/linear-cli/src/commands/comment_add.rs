//! Shared `comment add` steps: body flags, the body prompt check, the single
//! `AddComment` mutation and its output. Each leaf owns its target resolution
//! and calls these in the source order.
use cynic::{MutationBuilder, QueryBuilder};

use crate::commands::text_input;
use crate::error::{AppError, AppErrorKind};
use crate::graphql::envelope::{GraphQlRequest, is_not_found};
use crate::graphql::operations::comment_create::{
    AddComment, AddCommentVariables, CommentCreateInput, CreatedComment,
    DocumentCommentTargetVariables, GetDocumentCommentTarget,
};
use crate::graphql::transport::{GraphQlTransport, TransportFailure};
use crate::platform::prompt::{PromptOutcome, PromptSession};
use crate::refs::{reject_comment_url, reject_linear_url};

/// The source's single `handleError` prefix for every action failure.
pub const CONTEXT: &str = "Failed to add comment";
pub const PROMPT_MESSAGE: &str = "Comment body";

/// Linear requires exactly one target even for replies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommentTarget {
    Issue { issue_id: String },
    Document { document_content_id: String },
    Project { project_id: String },
    Initiative { initiative_id: String },
}

fn is_blank(value: &str) -> bool {
    value.trim().is_empty()
}

/// Turn `--body` / `--body-file` into a body, or `None` so the caller prompts.
/// Supplied text is returned unchanged; blank supplied input is an error.
pub fn resolve_body(
    body: Option<&str>,
    body_file: Option<&str>,
) -> Result<Option<String>, AppError> {
    match (body, body_file) {
        (Some(_), Some(_)) => Err(AppError::new(
            AppErrorKind::Validation,
            "Cannot specify both --body and --body-file",
        )),
        (None, Some(path)) => read_body_file(path).map(Some),
        (Some(text), None) if is_blank(text) => Err(AppError::new(
            AppErrorKind::Validation,
            "Comment body cannot be empty",
        )
        .with_suggestion("Pass text with --body, or omit it to be prompted.")),
        (Some(text), None) => Ok(Some(text.to_owned())),
        (None, None) => Ok(None),
    }
}

/// Invalid UTF-8 is rejected rather than replaced, so a comment never silently changes.
fn read_body_file(path: &str) -> Result<String, AppError> {
    let content = text_input::read_file(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::InvalidData {
            AppError::new(AppErrorKind::Validation, "Body file must be valid UTF-8")
                .with_suggestion(format!("Re-save {path} as UTF-8 text, or use --body."))
                .with_source(error)
        } else {
            AppError::new(
                AppErrorKind::Validation,
                format!("Failed to read body file: {path}"),
            )
            .with_suggestion(format!("Error: {error}"))
        }
    })?;
    if is_blank(&content) {
        return Err(AppError::new(
            AppErrorKind::Validation,
            format!("Body file is empty: {path}"),
        )
        .with_suggestion("Write the comment into the file, or use --body."));
    }
    Ok(content)
}

/// Ask for the body once. The answer is trimmed by the text prompt; a blank
/// answer is checked by the caller after the session is closed.
pub fn prompt_body<R: std::io::Read, W: std::io::Write>(
    session: &mut PromptSession<R, W>,
) -> Result<PromptOutcome<String>, AppError> {
    session.text(PROMPT_MESSAGE, 0, |_| Ok(()))
}

/// Reject a blank submitted prompt answer, without the flag suggestion.
pub fn require_prompted(body: String) -> Result<String, AppError> {
    if is_blank(&body) {
        return Err(AppError::new(
            AppErrorKind::Validation,
            "Comment body cannot be empty",
        ));
    }
    Ok(body)
}

/// Build the mutation input. A pasted comment link gets the specific
/// explanation; any other Linear URL gets the general one.
pub fn build_input(
    target: CommentTarget,
    body: String,
    parent_id: Option<&str>,
    id: Option<&str>,
) -> Result<CommentCreateInput, AppError> {
    if let Some(parent) = parent_id {
        reject_comment_url(parent)?;
        reject_linear_url(parent, "the UUID of the comment to reply to")?;
    }
    let mut input = CommentCreateInput {
        body,
        parent_id: parent_id.map(str::to_owned),
        id: id.map(str::to_owned),
        issue_id: None,
        document_content_id: None,
        project_id: None,
        initiative_id: None,
    };
    match target {
        CommentTarget::Issue { issue_id } => input.issue_id = Some(issue_id),
        CommentTarget::Document {
            document_content_id,
        } => input.document_content_id = Some(document_content_id),
        CommentTarget::Project { project_id } => input.project_id = Some(project_id),
        CommentTarget::Initiative { initiative_id } => input.initiative_id = Some(initiative_id),
    }
    Ok(input)
}

pub fn request(input: CommentCreateInput) -> GraphQlRequest<AddCommentVariables> {
    GraphQlRequest::with_variables(AddComment::build(AddCommentVariables { input }))
}

/// Send the mutation once. A failure after the request may have reached
/// Linear says the comment may already exist; nothing is retried.
pub async fn create(
    transport: &GraphQlTransport,
    input: CommentCreateInput,
) -> Result<CreatedComment, AppError> {
    let result: AddComment = transport
        .execute(&request(input))
        .await
        .map_err(|failure| {
            let uncertain = super::milestone_create::outcome_unknown(&failure);
            let mut error = AppError::from(failure);
            if uncertain {
                error.message.push_str("; comment may already exist");
            }
            error
        })?;
    if !result.comment_create.success {
        return Err(AppError::new(
            AppErrorKind::GraphQl,
            "Failed to create comment",
        ));
    }
    Ok(result.comment_create.comment)
}

/// The source's two `console.log` lines, naming the target as the user gave it.
pub fn output(noun: &str, original: &str, comment: &CreatedComment) -> Vec<u8> {
    format!("✓ Comment added to {noun} {original}\n{}\n", comment.url).into_bytes()
}

/// `document(id:)` is non-null, so Linear reports a missing document as a
/// GraphQL error; only that becomes NotFound.
pub async fn document_content_id(
    transport: &GraphQlTransport,
    document: &str,
) -> Result<String, AppError> {
    let request = GraphQlRequest::with_variables(GetDocumentCommentTarget::build(
        DocumentCommentTargetVariables {
            id: document.to_owned(),
        },
    ));
    let data: GetDocumentCommentTarget = transport.execute(&request).await.map_err(|failure| {
        if let TransportFailure::GraphQl { errors, .. } = &failure
            && is_not_found(errors)
        {
            return AppError::not_found("Document", document);
        }
        AppError::from(failure)
    })?;
    let target = data.document;
    target.document_content_id.ok_or_else(|| {
        AppError::new(
            AppErrorKind::Validation,
            format!(
                "Document \"{}\" has no content record to comment on",
                target.title
            ),
        )
        .with_suggestion("Linear attaches document comments to the document's content; open the document in Linear once so it gets one, then retry.")
    })
}
