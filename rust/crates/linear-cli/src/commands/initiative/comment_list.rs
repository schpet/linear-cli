//! `initiative comment list`: every page of an initiative's comments, as threads or JSON.
use chrono::Utc;
use cynic::QueryBuilder;

use crate::cli::initiative::InitiativeCommentList;
use crate::commands::comments;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::{GraphQlRequest, is_not_found};
use crate::graphql::operations::comments::CommentNode;
use crate::graphql::operations::initiative_comments::{
    GetInitiativeComments, GetInitiativeCommentsVariables,
};
use crate::graphql::operations::teams::PageInfo;
use crate::graphql::pagination::{self, Page};
use crate::graphql::transport::{GraphQlTransport, TransportFailure};

pub fn run(ctx: &Ctx, args: &InitiativeCommentList) -> Result<()> {
    list(ctx, args).context("Failed to list comments")
}

fn list(ctx: &Ctx, args: &InitiativeCommentList) -> Result<()> {
    let original = &args.initiative;
    let reference = super::reference(ctx, original)?;
    let client = ctx.client()?;
    let (nodes, page_info) = ctx.spin(!args.json, async {
        let id = super::resolve(client, &reference, original, super::Archived::Exclude).await?;
        fetch(client, &id, original).await
    })?;
    if args.json {
        ctx.print(comments::render_json(&nodes, &page_info))
    } else {
        ctx.print(comments::render_text(
            &nodes,
            Utc::now(),
            ctx.color(),
            "No comments found for this initiative",
        ))
    }
}

async fn fetch(
    client: &GraphQlTransport,
    id: &str,
    original: &str,
) -> Result<(Vec<CommentNode>, PageInfo)> {
    let result = pagination::paginate(|after| {
        let request = GraphQlRequest::with_variables(GetInitiativeComments::build(
            GetInitiativeCommentsVariables {
                id: id.to_owned(),
                filter_id: cynic::Id::new(id),
                after,
            },
        ));
        async move {
            let data: GetInitiativeComments = client
                .execute(&request)
                .await
                .map_err(|failure| not_found(failure, original))?;
            if data.initiative.is_none() {
                return Err(Error::not_found("Initiative", original));
            }
            Ok(Page {
                nodes: data.comments.nodes,
                page_info: data.comments.page_info.into(),
            })
        }
    })
    .await
    .map_err(comments::pagination_error)?;
    let page_info = PageInfo {
        has_next_page: result.page_info.has_next_page,
        end_cursor: result.page_info.end_cursor,
    };
    Ok((result.nodes, page_info))
}

/// `initiative(id:)` is non-null, so Linear reports a missing initiative as a
/// GraphQL error.
fn not_found(failure: TransportFailure, original: &str) -> Error {
    if let TransportFailure::GraphQl { errors, .. } = &failure
        && is_not_found(errors)
    {
        return Error::not_found("Initiative", original);
    }
    Error::from(failure)
}
