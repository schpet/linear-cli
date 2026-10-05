//! `linear document`: documents and their comments.
mod comment;
mod common;
mod create;
mod delete;
mod list;
mod target;
mod update;
mod view;

use crate::cli::document::DocumentCommand;
use crate::ctx::Ctx;
use crate::error::Result;

pub fn run(ctx: &Ctx, command: &DocumentCommand) -> Result<()> {
    match command {
        DocumentCommand::List(args) => list::run(ctx, args),
        DocumentCommand::View(args) => view::run(ctx, args),
        DocumentCommand::Create(args) => create::run(ctx, args),
        DocumentCommand::Update(args) => update::run(ctx, args),
        DocumentCommand::Delete(args) => delete::run(ctx, args),
        DocumentCommand::Comment(args) => comment::run(ctx, &args.command),
    }
}
