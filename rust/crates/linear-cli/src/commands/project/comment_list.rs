//! `project comment list`: every comment on a project, as threads or JSON.
use chrono::Utc;
use cynic::QueryBuilder;

use crate::cli::project::ProjectCommentList;
use crate::commands::comments;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::{GraphQlRequest, is_not_found};
use crate::graphql::operations::comments::CommentNode;
use crate::graphql::operations::project_comments::{
    GetProjectComments, GetProjectCommentsVariables,
};
use crate::graphql::operations::teams::PageInfo;
use crate::graphql::pagination::{self, EmptyCursorPolicy, Page};
use crate::graphql::transport::{GraphQlTransport, TransportFailure};
use crate::refs::{prepare_project_lookup, resolve_project_with_transport};

pub fn run(ctx: &Ctx, args: &ProjectCommentList) -> Result<()> {
    list(ctx, args).context("Failed to list comments")
}

fn list(ctx: &Ctx, args: &ProjectCommentList) -> Result<()> {
    let original = &args.project;
    let reference = prepare_project_lookup(original, &ctx.scope()?)?;
    let client = ctx.client()?;
    let (nodes, page_info) = ctx.spin(!args.json, async {
        let id = resolve_project_with_transport(&reference, original, client).await?;
        fetch(client, original, &id).await
    })?;
    if args.json {
        ctx.print(comments::render_json(&nodes, &page_info))
    } else {
        ctx.print(comments::render_text(
            &nodes,
            Utc::now(),
            ctx.color(),
            "No comments found for this project",
        ))
    }
}

/// Every comment on the project, across all pages.
async fn fetch(
    client: &GraphQlTransport,
    original: &str,
    id: &str,
) -> Result<(Vec<CommentNode>, PageInfo)> {
    let result = pagination::paginate_with_policy(EmptyCursorPolicy::Allow, |after| {
        let request = GraphQlRequest::with_variables(GetProjectComments::build(
            GetProjectCommentsVariables {
                id: id.to_owned(),
                filter_id: cynic::Id::new(id),
                after,
            },
        ));
        async move {
            let data: GetProjectComments = client.execute(&request).await.map_err(|failure| {
                if let TransportFailure::GraphQl { errors, .. } = &failure
                    && is_not_found(errors)
                {
                    return Error::not_found("Project", original);
                }
                Error::from(failure)
            })?;
            if data.project.is_none() {
                return Err(Error::not_found("Project", original));
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
