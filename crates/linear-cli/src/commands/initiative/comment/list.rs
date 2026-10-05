//! `initiative comment list`: every comment on an initiative, as threads or JSON.
use crate::cli::initiative::InitiativeCommentList;
use crate::commands::comments::{self, CommentSource};
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};
use crate::graphql::operations::comment::CommentConnection;
use crate::graphql::operations::comment::{GetInitiativeComments, GetInitiativeCommentsVariables};
use crate::refs::{self, initiative::Archived};

pub fn run(ctx: &Ctx, args: &InitiativeCommentList) -> Result<()> {
    list(ctx, args).context("Failed to list comments")
}

fn list(ctx: &Ctx, args: &InitiativeCommentList) -> Result<()> {
    let original = &args.initiative;
    let reference = crate::commands::initiative::common::reference(ctx, original)?;
    let client = ctx.client()?;
    let nodes = ctx.spin(!args.json, async {
        let id = refs::initiative::resolve(client, &reference, Archived::Exclude).await?;
        comments::fetch::<GetInitiativeComments>(client, original, &id, args.limit).await
    })?;
    comments::print(ctx, &nodes, args.json, "initiative", !args.no_pager)
}

impl CommentSource for GetInitiativeComments {
    const ENTITY: &'static str = "Initiative";
    type Variables = GetInitiativeCommentsVariables;

    fn variables(id: &str, after: Option<String>, first: i32) -> Self::Variables {
        GetInitiativeCommentsVariables {
            id: id.to_owned(),
            filter_id: cynic::Id::new(id),
            after,
            first,
        }
    }

    fn comments(self) -> Option<CommentConnection> {
        self.initiative.map(|_| self.comments)
    }
}
