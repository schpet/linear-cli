//! `linear document`: documents and their comments.
mod comment_add;
mod comment_list;
mod delete;
mod list;
mod target;
mod view;
pub mod write;

use crate::cli::document::{DocumentCommand, DocumentCommentCommand};
use crate::client::RequestError;
use crate::ctx::Ctx;
use crate::error::{Error, Result};
use crate::refs;

pub fn run(ctx: &Ctx, command: &DocumentCommand) -> Result<()> {
    match command {
        DocumentCommand::List(args) => list::run(ctx, args),
        DocumentCommand::View(args) => view::run(ctx, args),
        DocumentCommand::Create(args) => write::create(ctx, args),
        DocumentCommand::Update(args) => write::update(ctx, args),
        DocumentCommand::Delete(args) => delete::run(ctx, args),
        DocumentCommand::Comment(args) => match &args.command {
            DocumentCommentCommand::Add(args) => comment_add::run(ctx, args),
            DocumentCommentCommand::List(args) => comment_list::run(ctx, args),
        },
    }
}

/// The document's slug ID or UUID from a document argument, which may be a URL.
fn reference(ctx: &Ctx, input: &str) -> Result<String> {
    refs::document::parse(input, &ctx.scope()?)
}

/// `document(id:)` is non-null, so Linear reports a missing document as a
/// GraphQL error.
fn not_found(failure: RequestError, original: &str) -> Error {
    failure.or_not_found("Document", original)
}
