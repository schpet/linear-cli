//! `linear issue comment`: an issue's comments.
mod add;
mod delete;
mod list;
mod resolve_or_unresolve;
mod update;

use crate::cli::issue::IssueCommentCommand;
use crate::ctx::Ctx;
use crate::error::Result;
use resolve_or_unresolve::{Mode, Request};

pub fn run(ctx: &Ctx, command: &IssueCommentCommand) -> Result<()> {
    match command {
        IssueCommentCommand::Add(args) => add::run(ctx, args),
        IssueCommentCommand::Delete(args) => delete::run(ctx, args),
        IssueCommentCommand::Update(args) => update::run(ctx, args),
        IssueCommentCommand::List(args) => list::run(ctx, args),
        IssueCommentCommand::Resolve(args) => resolve_or_unresolve::run(
            ctx,
            Mode::Resolve,
            &Request {
                comment_ids: &args.comment_ids,
                with: args.with.as_deref(),
                bulk: &args.bulk,
            },
        ),
        IssueCommentCommand::Unresolve(args) => resolve_or_unresolve::run(
            ctx,
            Mode::Unresolve,
            &Request {
                comment_ids: &args.comment_ids,
                with: None,
                bulk: &args.bulk,
            },
        ),
    }
}
