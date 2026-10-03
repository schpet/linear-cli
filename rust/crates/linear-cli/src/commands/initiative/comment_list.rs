//! `initiative comment list`: every comment on an initiative, as threads or JSON.
use cynic::QueryBuilder;

use crate::cli::initiative::InitiativeCommentList;
use crate::commands::comments::{self, CommentSource};
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::comments::CommentConnection;
use crate::graphql::operations::initiative_comments::{
    GetInitiativeComments, GetInitiativeCommentsVariables,
};

pub fn run(ctx: &Ctx, args: &InitiativeCommentList) -> Result<()> {
    list(ctx, args).context("Failed to list comments")
}

fn list(ctx: &Ctx, args: &InitiativeCommentList) -> Result<()> {
    let original = &args.initiative;
    let reference = super::reference(ctx, original)?;
    let client = ctx.client()?;
    let nodes = ctx.spin(!args.json, async {
        let id = super::resolve(client, &reference, original, super::Archived::Exclude).await?;
        comments::fetch::<InitiativeComments>(client, original, &id, args.limit).await
    })?;
    comments::print(ctx, &nodes, args.json, "initiative")
}

struct InitiativeComments;

impl CommentSource for InitiativeComments {
    const ENTITY: &'static str = "Initiative";
    type Variables = GetInitiativeCommentsVariables;
    type Response = GetInitiativeComments;

    fn request(id: &str, after: Option<String>) -> GraphQlRequest<Self::Variables> {
        GraphQlRequest::with_variables(GetInitiativeComments::build(
            GetInitiativeCommentsVariables {
                id: id.to_owned(),
                filter_id: cynic::Id::new(id),
                after,
            },
        ))
    }

    fn comments(response: Self::Response) -> Option<CommentConnection> {
        response.initiative.map(|_| response.comments)
    }
}
