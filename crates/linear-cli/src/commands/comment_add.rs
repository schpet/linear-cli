//! Shared `comment add` steps: body flags, writing the body in the editor,
//! the single `AddComment` mutation and its output. Each target command
//! checks its flags, resolves its target, and only then opens the editor, so
//! nothing typed is lost to a target that does not exist.
use crate::cli::values::TextSource;
use crate::client::LinearClient;
use crate::commands::{confirm, outcome, text_input};
use crate::ctx::Ctx;
use crate::error::{Error, Result};
use crate::graphql::operations::comment::{
    AddComment, AddCommentVariables, CommentCreateInput, CreatedComment,
    DocumentCommentTargetVariables, GetCommentVariables, GetDocumentCommentTarget, GetReplyParent,
    ReplyParent,
};
use crate::refs::{is_linear_uuid, reject_comment_url, reject_linear_url};

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
    body_file: Option<&TextSource>,
) -> Result<Option<String>, Error> {
    match (body, body_file) {
        (Some(_), Some(_)) => Err(Error::invalid("Cannot specify both --body and --body-file")),
        (None, Some(path)) => read_body_file(path).map(Some),
        (Some(text), None) if is_blank(text) => Err(Error::invalid("Comment body cannot be empty")
            .with_hint("Pass text with --body, or omit it to write the comment in your editor.")),
        (Some(text), None) => Ok(Some(text.to_owned())),
        (None, None) => Ok(None),
    }
}

/// Invalid UTF-8 is rejected rather than replaced, so a comment never silently changes.
fn read_body_file(source: &TextSource) -> Result<String, Error> {
    let shown = source;
    let content = text_input::read_source(source).map_err(|error| {
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
        return Err(Error::invalid(format!("Body file is empty: {shown}"))
            .with_hint("Write the comment into the file, or use --body."));
    }
    Ok(content)
}

/// Fails before any lookup when no body was given and none can be written
/// in the editor, naming the flags to pass instead.
pub fn require_editor(ctx: &Ctx) -> Result<()> {
    if ctx.interactive() {
        Ok(())
    } else {
        Err(ctx.missing_value("No comment body given", "--body or --body-file"))
    }
}

/// The body written in the editor, starting from `initial`, once the user
/// confirms `question` (or passed `yes`). `None` when the editor is left
/// empty or unchanged, or the user declines; that is already reported.
pub fn write_in_editor(
    ctx: &Ctx,
    initial: &str,
    yes: bool,
    question: &str,
) -> Result<Option<String>> {
    let Some(body) = text_input::edited_body(&ctx.edit_text(initial)?) else {
        ctx.eprint("No content entered.\n")?;
        outcome::canceled(ctx)?;
        return Ok(None);
    };
    if body == initial.trim() {
        ctx.print(outcome::NO_CHANGES)?;
        return Ok(None);
    }
    if !confirm::proceed(ctx, yes, question)? {
        return Ok(None);
    }
    Ok(Some(body))
}

/// Checks `--reply-to` before anything is looked up: a pasted comment link
/// gets the specific explanation, any other Linear URL the general one, and
/// anything else that is not a UUID is refused.
pub fn check_parent(parent_id: Option<&str>) -> Result<()> {
    if let Some(parent) = parent_id {
        reject_comment_url(parent)?;
        reject_linear_url(parent, "the UUID of the comment to reply to")?;
        if !is_linear_uuid(parent) {
            return Err(Error::invalid(format!("Not a comment UUID: {parent}"))
                .with_hint("--reply-to takes the UUID of the top-level comment to reply to, as `comment list --json` shows it."));
        }
    }
    Ok(())
}

/// The comment `--reply-to` names, looked up before anything is typed so
/// a reply that cannot be posted fails first.
pub struct Parent<'a> {
    id: &'a str,
    comment: ReplyParent,
}

/// Looks up the comment `--reply-to` names, failing when it does not exist.
pub async fn fetch_parent<'a>(
    client: &LinearClient,
    parent_id: Option<&'a str>,
) -> Result<Option<Parent<'a>>> {
    let Some(id) = parent_id else {
        return Ok(None);
    };
    let data: GetReplyParent = client
        .query(GetCommentVariables { id: id.to_owned() })
        .await
        .map_err(|failure| failure.or_not_found("Comment", id))?;
    let comment = data
        .comment
        .ok_or_else(|| Error::not_found("Comment", id))?;
    Ok(Some(Parent { id, comment }))
}

impl Parent<'_> {
    /// Fails unless this is a top-level comment on `target` (described as
    /// `described`, like `issue ENG-1`), so a reply never lands in another
    /// entity's thread. An issue target must hold the issue's UUID.
    pub fn check(&self, target: &CommentTarget, described: &str) -> Result<()> {
        let Self { id, comment } = self;
        if let Some(top) = &comment.parent_id {
            return Err(Error::new(format!(
                "Comment {id} is a reply; only a top-level comment can be replied to"
            ))
            .with_hint(format!(
                "Reply in its thread with --reply-to {top}, the thread's top-level comment."
            )));
        }
        let on_target = match target {
            CommentTarget::Issue { issue_id } => comment.issue_id.as_ref() == Some(issue_id),
            CommentTarget::Document {
                document_content_id,
            } => comment.document_content_id.as_ref() == Some(document_content_id),
            CommentTarget::Project { project_id } => {
                comment.project_id.as_ref() == Some(project_id)
            }
            CommentTarget::Initiative { initiative_id } => {
                comment.initiative_id.as_ref() == Some(initiative_id)
            }
        };
        if on_target {
            Ok(())
        } else {
            Err(
                Error::new(format!("Comment {id} is not on {described}")).with_hint(
                    "--reply-to takes a top-level comment on the same entity, as its `comment list --json` shows.",
                ),
            )
        }
    }
}

/// The confirmation after the editor: a comment on `target`, or a reply.
pub fn question(target: &str, parent_id: Option<&str>) -> String {
    let what = if parent_id.is_some() {
        "reply"
    } else {
        "comment"
    };
    format!("Post this {what} on {target}?")
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
pub fn output(
    noun: &str,
    original: &str,
    parent_id: Option<&str>,
    comment: &CreatedComment,
) -> Vec<u8> {
    let what = if parent_id.is_some() {
        "reply to"
    } else {
        "comment to"
    };
    super::outcome::done(
        "Added",
        what,
        &format!("{noun} {original}"),
        Some(&comment.url),
    )
    .into_bytes()
}

/// A document as a comment target: the content record comments attach to,
/// and the title to name it by.
pub struct DocumentTarget {
    pub document_content_id: String,
    pub title: String,
}

/// `document(id:)` is non-null, so Linear reports a missing document as a
/// GraphQL error; only that becomes NotFound.
pub async fn document_target(
    client: &LinearClient,
    document: &str,
) -> Result<DocumentTarget, Error> {
    let data: GetDocumentCommentTarget = client
        .query(DocumentCommentTargetVariables {
            id: document.to_owned(),
        })
        .await
        .map_err(|failure| failure.or_not_found("Document", document))?;
    let target = data.document;
    let Some(document_content_id) = target.document_content_id else {
        return Err(Error::new(format!(
            "Document \"{}\" has no content record to comment on",
            target.title
        ))
        .with_hint("Linear attaches document comments to the document's content; open the document in Linear once so it gets one, then retry."));
    };
    Ok(DocumentTarget {
        document_content_id,
        title: target.title,
    })
}

#[cfg(test)]
mod tests;
