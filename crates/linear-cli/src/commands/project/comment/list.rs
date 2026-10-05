//! `project comment list`: every comment on a project, as threads or JSON.
use crate::cli::project::ProjectCommentList;
use crate::commands::comments::{self, CommentSource};
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};
use crate::graphql::operations::comment::CommentConnection;
use crate::graphql::operations::comment::{GetProjectComments, GetProjectCommentsVariables};
use crate::refs::{self, project::ProjectReference};

pub fn run(ctx: &Ctx, args: &ProjectCommentList) -> Result<()> {
    list(ctx, args).context("Failed to list comments")
}

fn list(ctx: &Ctx, args: &ProjectCommentList) -> Result<()> {
    let original = &args.project;
    let reference = ProjectReference::parse(original, &ctx.scope()?)?;
    let client = ctx.client()?;
    let nodes = ctx.spin(!args.json, async {
        let id = refs::project::resolve(client, &reference).await?;
        comments::fetch::<GetProjectComments>(client, original, &id, args.limit).await
    })?;
    comments::print(ctx, &nodes, args.json, "project", !args.no_pager)
}

impl CommentSource for GetProjectComments {
    const ENTITY: &'static str = "Project";
    type Variables = GetProjectCommentsVariables;

    fn variables(id: &str, after: Option<String>, first: i32) -> Self::Variables {
        GetProjectCommentsVariables {
            id: id.to_owned(),
            filter_id: cynic::Id::new(id),
            after,
            first,
        }
    }

    fn comments(self) -> Option<CommentConnection> {
        self.project.map(|_| self.comments)
    }
}
