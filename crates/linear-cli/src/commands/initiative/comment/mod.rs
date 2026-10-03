//! `linear initiative comment`: an initiative's comments.
mod add;
mod list;

use crate::cli::initiative::InitiativeCommentCommand;
use crate::ctx::Ctx;
use crate::error::Result;

pub fn run(ctx: &Ctx, command: &InitiativeCommentCommand) -> Result<()> {
    match command {
        InitiativeCommentCommand::Add(args) => add::run(ctx, args),
        InitiativeCommentCommand::List(args) => list::run(ctx, args),
    }
}
