//! `issue comment list`: every comment on an issue, as threads or JSON,
//! optionally only the resolved or the open threads.
use crate::cli::Limit;
use crate::cli::issue::IssueCommentList;
use crate::commands::comments::{self, CommentSource};
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};
use crate::graphql::operations::comment::CommentConnection;
use crate::graphql::operations::comment::{GetIssueComments, GetIssueCommentsVariables};

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
    let identifier = crate::commands::issue::require(ctx, args.issue_id.as_deref())?;
    let client = ctx.client()?;
    // Linear cannot filter comments by resolution, so a filtered list reads
    // every comment and then applies --limit to what is left.
    let fetch_limit = if filter.is_some() {
        Limit::All
    } else {
        args.limit
    };
    let mut nodes = ctx.spin(
        !args.json,
        comments::fetch::<GetIssueComments>(client, &identifier, &identifier, fetch_limit),
    )?;
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

impl CommentSource for GetIssueComments {
    const ENTITY: &'static str = "Issue";
    type Variables = GetIssueCommentsVariables;

    fn variables(id: &str, after: Option<String>, first: i32) -> Self::Variables {
        GetIssueCommentsVariables {
            id: id.to_owned(),
            after,
            first,
        }
    }

    fn comments(self) -> Option<CommentConnection> {
        self.issue.map(|issue| issue.comments)
    }
}
