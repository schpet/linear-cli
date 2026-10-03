//! `document comment list`: every comment on a document, as threads or JSON.
use crate::cli::document::DocumentCommentList;
use crate::commands::comments::{self, CommentSource};
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};
use crate::graphql::operations::comment::CommentConnection;
use crate::graphql::operations::comment::{GetDocumentComments, GetDocumentCommentsVariables};

pub fn run(ctx: &Ctx, args: &DocumentCommentList) -> Result<()> {
    list(ctx, args).context("Failed to list comments")
}

fn list(ctx: &Ctx, args: &DocumentCommentList) -> Result<()> {
    let id = crate::commands::document::common::reference(ctx, &args.document)?;
    let client = ctx.client()?;
    let nodes = ctx.spin(
        !args.json,
        comments::fetch::<GetDocumentComments>(client, &args.document, &id, args.limit),
    )?;
    comments::print(ctx, &nodes, args.json, "document")
}

impl CommentSource for GetDocumentComments {
    const ENTITY: &'static str = "Document";
    type Variables = GetDocumentCommentsVariables;

    fn variables(id: &str, after: Option<String>, first: i32) -> Self::Variables {
        GetDocumentCommentsVariables {
            id: id.to_owned(),
            after,
            first,
        }
    }

    fn comments(self) -> Option<CommentConnection> {
        self.document.map(|document| document.comments)
    }
}
