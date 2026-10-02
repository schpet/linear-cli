//! `issue comment list`: typed nested connection, complete pagination, and threads.

use std::future::Future;

use chrono::{DateTime, Utc};
use cynic::QueryBuilder;

use crate::cli::issue::IssueCommentList;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::{GraphQlRequest, is_not_found};
use crate::graphql::operations::comments::CommentNode;
use crate::graphql::operations::issue_comments::{GetIssueComments, GetIssueCommentsVariables};
use crate::graphql::operations::teams::PageInfo;
use crate::graphql::pagination::{self, EmptyCursorPolicy, Page};
use crate::graphql::transport::{GraphQlTransport, TransportFailure};

pub fn run(ctx: &Ctx, args: &IssueCommentList) -> Result<()> {
    list(ctx, args).context("Failed to list comments")
}

fn list(ctx: &Ctx, args: &IssueCommentList) -> Result<()> {
    let identifier = crate::commands::issue::require(ctx, args.issue_id.as_deref())?;
    let client = ctx.client()?;
    let output = ctx.spin(
        !args.json,
        fetch_output(client, &identifier, args.json, ctx.color()),
    )?;
    ctx.print(output)
}

pub fn request(id: &str, after: Option<String>) -> GraphQlRequest<GetIssueCommentsVariables> {
    GraphQlRequest::with_variables(GetIssueComments::build(GetIssueCommentsVariables {
        id: id.to_owned(),
        after,
    }))
}

/// Every page of the issue's comments, rendered.
async fn fetch_output(
    transport: &GraphQlTransport,
    identifier: &str,
    json: bool,
    color: bool,
) -> Result<Vec<u8>> {
    run_with(
        identifier,
        identifier,
        |query| async move {
            transport
                .execute(&query)
                .await
                .map_err(|failure| translate_failure(failure, identifier))
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
    F: FnMut(GraphQlRequest<GetIssueCommentsVariables>) -> Fut,
    Fut: Future<Output = Result<GetIssueComments, Error>>,
{
    let result = pagination::paginate_with_policy(EmptyCursorPolicy::Allow, |after| {
        let query = request(id, after);
        let pending = fetch(query);
        async move {
            let data = pending.await?;
            let issue = data
                .issue
                .ok_or_else(|| Error::not_found("Issue", original))?;
            Ok::<Page<CommentNode>, Error>(Page {
                nodes: issue.comments.nodes,
                page_info: issue.comments.page_info.into(),
            })
        }
    })
    .await
    .map_err(crate::commands::comments::pagination_error)?;
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
        return Error::not_found("Issue", original);
    }
    Error::from(failure)
}

pub fn render_json(nodes: &[CommentNode], page_info: &PageInfo) -> Result<Vec<u8>, Error> {
    Ok(crate::commands::comments::render_json(nodes, page_info))
}
pub fn render_text(nodes: &[CommentNode], now: DateTime<Utc>, color: bool) -> String {
    crate::commands::comments::render_text(nodes, now, color, "No comments found for this issue")
}
