//! `document comment list`: every comment on a document, as threads or JSON.
use cynic::QueryBuilder;

use crate::cli::document::DocumentCommentList;
use crate::commands::comments::{self, CommentSource};
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::comments::CommentConnection;
use crate::graphql::operations::document_comments::{
    GetDocumentComments, GetDocumentCommentsVariables,
};

pub fn run(ctx: &Ctx, args: &DocumentCommentList) -> Result<()> {
    list(ctx, args).context("Failed to list comments")
}

fn list(ctx: &Ctx, args: &DocumentCommentList) -> Result<()> {
    let id = super::reference(ctx, &args.document)?;
    let client = ctx.client()?;
    let nodes = ctx.spin(
        !args.json,
        comments::fetch::<DocumentComments>(client, &args.document, &id, args.limit),
    )?;
    comments::print(ctx, &nodes, args.json, "document")
}

struct DocumentComments;

impl CommentSource for DocumentComments {
    const ENTITY: &'static str = "Document";
    type Variables = GetDocumentCommentsVariables;
    type Response = GetDocumentComments;

    fn request(id: &str, after: Option<String>, first: i32) -> GraphQlRequest<Self::Variables> {
        GraphQlRequest::with_variables(GetDocumentComments::build(GetDocumentCommentsVariables {
            id: id.to_owned(),
            after,
            first,
        }))
    }

    fn comments(response: Self::Response) -> Option<CommentConnection> {
        response.document.map(|document| document.comments)
    }
}
