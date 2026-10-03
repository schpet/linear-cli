//! `issue comment list`: every comment on an issue, as threads or JSON.
use crate::cli::issue::IssueCommentList;
use crate::commands::comments::{self, CommentSource};
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};
use crate::graphql::operations::comments::CommentConnection;
use crate::graphql::operations::issue_comments::{GetIssueComments, GetIssueCommentsVariables};

pub fn run(ctx: &Ctx, args: &IssueCommentList) -> Result<()> {
    list(ctx, args).context("Failed to list comments")
}

fn list(ctx: &Ctx, args: &IssueCommentList) -> Result<()> {
    let identifier = crate::commands::issue::require(ctx, args.issue_id.as_deref())?;
    let client = ctx.client()?;
    let nodes = ctx.spin(
        !args.json,
        comments::fetch::<GetIssueComments>(client, &identifier, &identifier, args.limit),
    )?;
    comments::print(ctx, &nodes, args.json, "issue")
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
