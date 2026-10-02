//! `initiative comment list`: typed root connection, complete pagination, and threads.

use std::future::Future;

use chrono::{DateTime, Utc};
use cynic::QueryBuilder;

use crate::error::Error;
use crate::graphql::envelope::{GraphQlRequest, is_not_found};
use crate::graphql::operations::comments::CommentNode;
use crate::graphql::operations::initiative_comments::{
    GetInitiativeComments, GetInitiativeCommentsVariables,
};
use crate::graphql::operations::teams::PageInfo;
use crate::graphql::pagination::{self, EmptyCursorPolicy, Page};
use crate::graphql::transport::{GraphQlTransport, TransportFailure};

pub const CONTEXT: &str = "Failed to list comments";

pub fn request(id: &str, after: Option<String>) -> GraphQlRequest<GetInitiativeCommentsVariables> {
    GraphQlRequest::with_variables(GetInitiativeComments::build(
        GetInitiativeCommentsVariables {
            id: id.to_owned(),
            filter_id: cynic::Id::new(id),
            after,
        },
    ))
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
    F: FnMut(GraphQlRequest<GetInitiativeCommentsVariables>) -> Fut,
    Fut: Future<Output = Result<GetInitiativeComments, Error>>,
{
    let result = pagination::paginate_with_policy(EmptyCursorPolicy::Allow, |after| {
        let query = request(id, after);
        let pending = fetch(query);
        async move {
            let data = pending.await?;
            if data.initiative.is_none() {
                return Err(Error::not_found("Initiative", original));
            }
            Ok::<Page<CommentNode>, Error>(Page {
                nodes: data.comments.nodes,
                page_info: data.comments.page_info.into(),
            })
        }
    })
    .await
    .map_err(|error| super::comments::pagination_error(error, CONTEXT))?;
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
        return Error::not_found("Initiative", original);
    }
    Error::from(failure)
}

pub fn render_json(nodes: &[CommentNode], page_info: &PageInfo) -> Result<Vec<u8>, Error> {
    super::comments::render_json(nodes, page_info, "initiative", CONTEXT)
}
pub fn render_text(nodes: &[CommentNode], now: DateTime<Utc>, color: bool) -> String {
    super::comments::render_text(nodes, now, color, "No comments found for this initiative")
}
