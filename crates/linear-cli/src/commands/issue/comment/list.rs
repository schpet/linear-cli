//! `issue comment list`: every comment on an issue, as threads or JSON,
//! optionally only the resolved or the open threads.
use crate::cli::Limit;
use crate::cli::issue::IssueCommentList;
use crate::client::LinearClient;
use crate::commands::comments;
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};
use crate::graphql::operations::comment::CommentNode;
use crate::graphql::operations::comment::{GetIssueComments, GetIssueCommentsVariables};
use crate::graphql::pagination::{self, Page};

pub fn run(ctx: &Ctx, args: &IssueCommentList) -> Result<()> {
    list(ctx, args).context("Failed to list comments")
}

/// Which threads `--resolved` or `--unresolved` keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ThreadFilter {
    Resolved,
    Open,
}

impl ThreadFilter {
    fn from_flags(resolved: bool, unresolved: bool) -> Option<Self> {
        match (resolved, unresolved) {
            (false, false) => None,
            (true, false) => Some(Self::Resolved),
            (false, true) => Some(Self::Open),
            (true, true) => unreachable!("clap rejects --resolved with --unresolved"),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Resolved => "resolved",
            Self::Open => "open",
        }
    }
}

fn list(ctx: &Ctx, args: &IssueCommentList) -> Result<()> {
    let filter = ThreadFilter::from_flags(args.resolved, args.unresolved);
    let identifier = crate::commands::issue::require(ctx, &args.issue_id)?;
    let client = ctx.client()?;
    // Linear cannot filter comments by resolution, so a filtered list reads
    // every comment and then applies --limit to what is left.
    let fetch_limit = if filter.is_some() {
        Limit::All
    } else {
        args.limit
    };
    let mut nodes = ctx.spin(!args.json, fetch(ctx, client, &identifier, fetch_limit))?;
    let Some(filter) = filter else {
        return comments::print(ctx, &nodes, args.json, "issue", !args.no_pager);
    };
    nodes.retain(|node| comments::in_resolved_thread(node) == (filter == ThreadFilter::Resolved));
    args.limit.apply(&mut nodes);
    if nodes.is_empty() && !args.json {
        return ctx.print(format!(
            "No {} threads found for this issue\n",
            filter.label()
        ));
    }
    comments::print(ctx, &nodes, args.json, "issue", !args.no_pager)
}

async fn fetch(
    ctx: &Ctx,
    client: &LinearClient,
    identifier: &str,
    limit: Limit,
) -> Result<Vec<CommentNode>> {
    pagination::collect(limit.max(), |after, first| {
        let first_page = after.is_none();
        let variables = GetIssueCommentsVariables {
            id: identifier.to_owned(),
            after,
            first,
        };
        async move {
            let response: GetIssueComments = client
                .query(variables)
                .await
                .map_err(|failure| failure.or_not_found("Issue", identifier))?;
            let issue = response
                .issue
                .ok_or_else(|| crate::error::Error::not_found("Issue", identifier))?;
            if first_page {
                super::super::lifecycle::Lifecycle::of(issue.trashed, issue.archived_at)
                    .warn(ctx, identifier)?;
            }
            Ok(Page {
                nodes: issue.comments.nodes,
                page_info: issue.comments.page_info,
            })
        }
    })
    .await
}
