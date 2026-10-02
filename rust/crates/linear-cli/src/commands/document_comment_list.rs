//! `document comment list`: typed nested connection, complete pagination, and threads.

use std::future::Future;

use chrono::{DateTime, Utc};
use cynic::QueryBuilder;

use crate::error::Error;
use crate::graphql::envelope::{GraphQlRequest, is_not_found};
use crate::graphql::operations::comments::CommentNode;
use crate::graphql::operations::document_comments::{
    GetDocumentComments, GetDocumentCommentsVariables,
};
use crate::graphql::operations::teams::PageInfo;
use crate::graphql::pagination::{self, EmptyCursorPolicy, Page};
use crate::graphql::transport::{GraphQlTransport, TransportFailure};

pub const CONTEXT: &str = "Failed to list comments";

pub fn request(id: &str, after: Option<String>) -> GraphQlRequest<GetDocumentCommentsVariables> {
    GraphQlRequest::with_variables(GetDocumentComments::build(GetDocumentCommentsVariables {
        id: id.to_owned(),
        after,
    }))
}

/// Complete every request before returning a byte of output to the caller.
pub async fn run(
    transport: &GraphQlTransport,
    original: &str,
    id: &str,
    json: bool,
    color: bool,
) -> Result<Vec<u8>, Error> {
    run_with(
        original,
        id,
        |query| async move {
            transport
                .execute(&query)
                .await
                .map_err(|failure| translate_failure(failure, original))
        },
        json,
        color,
        Utc::now(),
    )
    .await
}

pub async fn run_with<F, Fut>(
    original: &str,
    id: &str,
    mut fetch: F,
    json: bool,
    color: bool,
    now: DateTime<Utc>,
) -> Result<Vec<u8>, Error>
where
    F: FnMut(GraphQlRequest<GetDocumentCommentsVariables>) -> Fut,
    Fut: Future<Output = Result<GetDocumentComments, Error>>,
{
    let result = pagination::paginate_with_policy(EmptyCursorPolicy::Allow, |after| {
        let query = request(id, after);
        let pending = fetch(query);
        async move {
            let data = pending.await?;
            let document = data
                .document
                .ok_or_else(|| Error::not_found("Document", original))?;
            Ok::<Page<CommentNode>, Error>(Page {
                nodes: document.comments.nodes,
                page_info: document.comments.page_info.into(),
            })
        }
    })
    .await
    .map_err(|error| super::comments::pagination_error(error).context(CONTEXT))?;
    let page_info = PageInfo {
        has_next_page: result.page_info.has_next_page,
        end_cursor: result.page_info.end_cursor,
    };
    if json {
        render_json(&result.nodes, &page_info)
    } else {
        Ok(render_text(&result.nodes, now, color).into_bytes())
    }
}

fn translate_failure(failure: TransportFailure, original: &str) -> Error {
    if let TransportFailure::GraphQl { errors, .. } = &failure
        && is_not_found(errors)
    {
        return Error::not_found("Document", original);
    }
    Error::from(failure)
}

pub fn render_json(nodes: &[CommentNode], page_info: &PageInfo) -> Result<Vec<u8>, Error> {
    Ok(super::comments::render_json(nodes, page_info))
}
pub fn render_text(nodes: &[CommentNode], now: DateTime<Utc>, color: bool) -> String {
    super::comments::render_text(nodes, now, color, "No comments found for this document")
}
