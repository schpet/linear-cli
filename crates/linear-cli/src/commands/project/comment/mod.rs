//! `linear project comment`: a project's comments.
mod add;
mod list;

use crate::cli::project::ProjectCommentCommand;
use crate::ctx::Ctx;
use crate::error::Result;

pub fn run(ctx: &Ctx, command: &ProjectCommentCommand) -> Result<()> {
    match command {
        ProjectCommentCommand::Add(args) => add::run(ctx, args),
        ProjectCommentCommand::List(args) => list::run(ctx, args),
    }
}
