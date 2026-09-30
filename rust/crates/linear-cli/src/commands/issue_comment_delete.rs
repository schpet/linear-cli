//! `issue comment delete`: one typed mutation for a directly supplied comment id.
use cynic::MutationBuilder;

use crate::error::{AppError, AppErrorKind};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::comment_delete::{DeleteComment, DeleteCommentVariables};
use crate::graphql::transport::GraphQlTransport;

/// The source's single `handleError` prefix for every action failure.
pub const CONTEXT: &str = "Failed to delete comment";

/// The source's `console.log` line after a successful delete.
pub const DELETED: &[u8] = "✓ Comment deleted\n".as_bytes();

/// The raw positional is sent unchanged: no UUID check or comment lookup.
pub fn request(id: &str) -> GraphQlRequest<DeleteCommentVariables> {
    GraphQlRequest::with_variables(DeleteComment::build(DeleteCommentVariables {
        id: id.to_owned(),
    }))
}

/// Sends the mutation exactly once and keeps the source's diagnostics; a
/// failed delete is visible on a retry, so no uncertainty text is added.
pub async fn submit(transport: &GraphQlTransport, id: &str) -> Result<Vec<u8>, AppError> {
    let result: DeleteComment = transport.execute(&request(id)).await?;
    if !result.comment_delete.success {
        return Err(AppError::new(AppErrorKind::GraphQl, CONTEXT));
    }
    Ok(DELETED.to_vec())
}
