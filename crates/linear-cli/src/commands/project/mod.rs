//! `linear project`: projects and their comments.
mod collections;
mod comment;
mod common;
mod create;
mod delete;
mod list;
mod update;
pub(crate) mod view;

use crate::cli::project::ProjectCommand;
use crate::ctx::Ctx;
use crate::error::Result;

pub fn run(ctx: &Ctx, command: &ProjectCommand) -> Result<()> {
    match command {
        ProjectCommand::List(args) => list::run(ctx, args),
        ProjectCommand::View(args) => view::run(ctx, args),
        ProjectCommand::Create(args) => create::run(ctx, args),
        ProjectCommand::Update(args) => update::run(ctx, args),
        ProjectCommand::Delete(args) => delete::run(ctx, args),
        ProjectCommand::Comment(args) => comment::run(ctx, &args.command),
    }
}
