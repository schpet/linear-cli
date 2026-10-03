//! `linear issue comment`: an issue's comments.
mod add;
mod delete;
mod list;
mod update;

use crate::cli::issue::IssueCommentCommand;
use crate::ctx::Ctx;
use crate::error::Result;

pub fn run(ctx: &Ctx, command: &IssueCommentCommand) -> Result<()> {
    match command {
        IssueCommentCommand::Add(args) => add::run(ctx, args),
        IssueCommentCommand::Delete(args) => delete::run(ctx, args),
        IssueCommentCommand::Update(args) => update::run(ctx, args),
        IssueCommentCommand::List(args) => list::run(ctx, args),
    }
}
