//! `document comment list`: every page of a document's comments, as threads or JSON.
use chrono::Utc;
use cynic::QueryBuilder;

use crate::cli::document::DocumentCommentList;
use crate::commands::comments;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::comments::CommentNode;
use crate::graphql::operations::document_comments::{
    GetDocumentComments, GetDocumentCommentsVariables,
};
use crate::graphql::operations::teams::PageInfo;
use crate::graphql::pagination::{self, Page};
use crate::graphql::transport::GraphQlTransport;

pub fn run(ctx: &Ctx, args: &DocumentCommentList) -> Result<()> {
    list(ctx, args).context("Failed to list comments")
}

fn list(ctx: &Ctx, args: &DocumentCommentList) -> Result<()> {
    let id = super::reference(ctx, &args.document)?;
    let client = ctx.client()?;
    let (nodes, page_info) = ctx.spin(!args.json, fetch(client, &id, &args.document))?;
    if args.json {
        ctx.print(comments::render_json(&nodes, &page_info))
    } else {
        ctx.print(comments::render_text(
            &nodes,
            Utc::now(),
            ctx.color(),
            "No comments found for this document",
        ))
    }
}

async fn fetch(
    client: &GraphQlTransport,
    id: &str,
    original: &str,
) -> Result<(Vec<CommentNode>, PageInfo)> {
    let result = pagination::paginate(|after| {
        let request = GraphQlRequest::with_variables(GetDocumentComments::build(
            GetDocumentCommentsVariables {
                id: id.to_owned(),
                after,
            },
        ));
        async move {
            let data: GetDocumentComments = client
                .execute(&request)
                .await
                .map_err(|failure| super::not_found(failure, original))?;
            let document = data
                .document
                .ok_or_else(|| Error::not_found("Document", original))?;
            Ok(Page {
                nodes: document.comments.nodes,
                page_info: document.comments.page_info.into(),
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
