//! `linear document comment`: a document's comments.
mod add;
mod list;

use crate::cli::document::DocumentCommentCommand;
use crate::ctx::Ctx;
use crate::error::Result;

pub fn run(ctx: &Ctx, command: &DocumentCommentCommand) -> Result<()> {
    match command {
        DocumentCommentCommand::Add(args) => add::run(ctx, args),
        DocumentCommentCommand::List(args) => list::run(ctx, args),
    }
}
