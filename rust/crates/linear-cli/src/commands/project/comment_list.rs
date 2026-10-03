//! `project comment list`: every comment on a project, as threads or JSON.
use cynic::QueryBuilder;

use crate::cli::project::ProjectCommentList;
use crate::commands::comments::{self, CommentSource};
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::comments::CommentConnection;
use crate::graphql::operations::project_comments::{
    GetProjectComments, GetProjectCommentsVariables,
};
use crate::refs::{prepare_project_lookup, resolve_project_with_transport};

pub fn run(ctx: &Ctx, args: &ProjectCommentList) -> Result<()> {
    list(ctx, args).context("Failed to list comments")
}

fn list(ctx: &Ctx, args: &ProjectCommentList) -> Result<()> {
    let original = &args.project;
    let reference = prepare_project_lookup(original, &ctx.scope()?)?;
    let client = ctx.client()?;
    let nodes = ctx.spin(!args.json, async {
        let id = resolve_project_with_transport(&reference, original, client).await?;
        comments::fetch::<ProjectComments>(client, original, &id, args.limit).await
    })?;
    comments::print(ctx, &nodes, args.json, "project")
}

struct ProjectComments;

impl CommentSource for ProjectComments {
    const ENTITY: &'static str = "Project";
    type Variables = GetProjectCommentsVariables;
    type Response = GetProjectComments;

    fn request(id: &str, after: Option<String>, first: i32) -> GraphQlRequest<Self::Variables> {
        GraphQlRequest::with_variables(GetProjectComments::build(GetProjectCommentsVariables {
            id: id.to_owned(),
            filter_id: cynic::Id::new(id),
            after,
            first,
        }))
    }

    fn comments(response: Self::Response) -> Option<CommentConnection> {
        response.project.map(|_| response.comments)
    }
}
