//! Shared `comment add` steps: body flags, the body prompt check, the single
//! `AddComment` mutation and its output. Each target command resolves its own
//! target and then calls these in order.
use std::path::Path;

use crate::client::LinearClient;
use crate::commands::text_input;
use crate::ctx::Ctx;
use crate::error::{Error, Result};
use crate::graphql::operations::comment::{
    AddComment, AddCommentVariables, CommentCreateInput, CreatedComment,
    DocumentCommentTargetVariables, GetDocumentCommentTarget,
};
use crate::platform::prompt::Text;
use crate::refs::{reject_comment_url, reject_linear_url};

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
pub fn resolve_body(body: Option<&str>, body_file: Option<&Path>) -> Result<Option<String>, Error> {
    match (body, body_file) {
        (Some(_), Some(_)) => Err(Error::invalid("Cannot specify both --body and --body-file")),
        (None, Some(path)) => read_body_file(path).map(Some),
        (Some(text), None) if is_blank(text) => Err(Error::invalid("Comment body cannot be empty")
            .with_hint("Pass text with --body, or omit it to be prompted.")),
        (Some(text), None) => Ok(Some(text.to_owned())),
        (None, None) => Ok(None),
    }
}

/// Invalid UTF-8 is rejected rather than replaced, so a comment never silently changes.
fn read_body_file(path: &Path) -> Result<String, Error> {
    let shown = path.display();
    let content = text_input::read_file(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::InvalidData {
            Error::new("Body file must be valid UTF-8")
                .with_hint(format!("Re-save {shown} as UTF-8 text, or use --body."))
                .with_source(error)
        } else {
            Error::new(format!("Failed to read body file: {shown}"))
                .with_hint(format!("Error: {error}"))
        }
    })?;
    if is_blank(&content) {
        return Err(Error::new(format!("Body file is empty: {shown}"))
            .with_hint("Write the comment into the file, or use --body."));
    }
    Ok(content)
}

/// Asks for the body on the terminal; without one it fails, naming --body.
pub fn prompt(ctx: &Ctx) -> Result<String> {
    if !ctx.interactive() {
        return Err(ctx.missing_value("No comment body given", "--body or --body-file"));
    }
    ctx.prompter()?.text(Text::new("Comment body").required())
}

/// Checks `--parent` before anything is looked up: a pasted comment link
/// gets the specific explanation, any other Linear URL the general one.
pub fn check_parent(parent_id: Option<&str>) -> Result<()> {
    if let Some(parent) = parent_id {
        reject_comment_url(parent)?;
        reject_linear_url(parent, "the UUID of the comment to reply to")?;
    }
    Ok(())
}

/// Build the mutation input, with `parent_id` already checked by [`check_parent`].
pub fn build_input(
    target: CommentTarget,
    body: String,
    parent_id: Option<&str>,
    id: Option<&str>,
) -> CommentCreateInput {
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
    input
}

/// Send the mutation once. A failure after the request may have reached
/// Linear says the comment may already exist; nothing is retried.
pub async fn create(
    client: &LinearClient,
    input: CommentCreateInput,
) -> Result<CreatedComment, Error> {
    let result: AddComment = client
        .mutate(AddCommentVariables { input })
        .await
        .map_err(|failure| failure.into_create_error("comment"))?;
    if !result.comment_create.success {
        return Err(Error::new("Failed to create comment"));
    }
    Ok(result.comment_create.comment)
}

/// The success lines, naming the target as the user gave it.
pub fn output(noun: &str, original: &str, comment: &CreatedComment) -> Vec<u8> {
    super::outcome::done(
        "Added",
        "comment to",
        &format!("{noun} {original}"),
        Some(&comment.url),
    )
    .into_bytes()
}

/// `document(id:)` is non-null, so Linear reports a missing document as a
/// GraphQL error; only that becomes NotFound.
pub async fn document_content_id(client: &LinearClient, document: &str) -> Result<String, Error> {
    let data: GetDocumentCommentTarget = client
        .query(DocumentCommentTargetVariables {
            id: document.to_owned(),
        })
        .await
        .map_err(|failure| failure.or_not_found("Document", document))?;
    let target = data.document;
    target.document_content_id.ok_or_else(|| {
        Error::new(format!(
                "Document \"{}\" has no content record to comment on",
                target.title
            ),
        )
        .with_hint("Linear attaches document comments to the document's content; open the document in Linear once so it gets one, then retry.")
    })
}

#[cfg(test)]
mod tests;
