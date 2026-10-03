//! `linear initiative`: initiatives, their projects and comments.
mod add_project;
mod archive;
mod archive_or_delete;
mod comment;
mod common;
mod create;
mod delete;
mod list;
mod remove_project;
mod unarchive;
mod update;
mod view;

use crate::cli::initiative::InitiativeCommand;
use crate::ctx::Ctx;
use crate::error::Result;

pub fn run(ctx: &Ctx, command: &InitiativeCommand) -> Result<()> {
    match command {
        InitiativeCommand::List(args) => list::run(ctx, args),
        InitiativeCommand::View(args) => view::run(ctx, args),
        InitiativeCommand::Create(args) => create::run(ctx, args),
        InitiativeCommand::Archive(args) => archive::run(ctx, args),
        InitiativeCommand::Update(args) => update::run(ctx, args),
        InitiativeCommand::Unarchive(args) => unarchive::run(ctx, args),
        InitiativeCommand::Delete(args) => delete::run(ctx, args),
        InitiativeCommand::AddProject(args) => add_project::run(ctx, args),
        InitiativeCommand::RemoveProject(args) => remove_project::run(ctx, args),
        InitiativeCommand::Comment(args) => comment::run(ctx, &args.command),
    }
}
