//! `linear initiative`: initiatives, their projects and comments.
mod archive;
mod comment_add;
mod comment_list;
mod create;
pub mod list;
mod projects;
mod unarchive;
mod update;
pub mod view;

use crate::cli::initiative::{InitiativeCommand, InitiativeCommentCommand};
use crate::ctx::Ctx;
use crate::error::Result;
use crate::refs::{self, initiative::InitiativeReference};

pub fn run(ctx: &Ctx, command: &InitiativeCommand) -> Result<()> {
    match command {
        InitiativeCommand::List(args) => list::run(ctx, args),
        InitiativeCommand::View(args) => view::run(ctx, args),
        InitiativeCommand::Create(args) => create::run(ctx, args),
        InitiativeCommand::Archive(args) => archive::archive(ctx, args),
        InitiativeCommand::Update(args) => update::run(ctx, args),
        InitiativeCommand::Unarchive(args) => unarchive::run(ctx, args),
        InitiativeCommand::Delete(args) => archive::delete(ctx, args),
        InitiativeCommand::AddProject(args) => projects::add(ctx, args),
        InitiativeCommand::RemoveProject(args) => projects::remove(ctx, args),
        InitiativeCommand::Comment(args) => match &args.command {
            InitiativeCommentCommand::Add(args) => comment_add::run(ctx, args),
            InitiativeCommentCommand::List(args) => comment_list::run(ctx, args),
        },
    }
}

/// Parses an initiative argument (URL, UUID, slug ID or name) without a request.
fn reference(ctx: &Ctx, input: &str) -> Result<InitiativeReference> {
    InitiativeReference::parse(input, &ctx.scope()?)
}

/// Rejects a Linear URL where an owner is expected, before any request.
fn check_owner(owner: Option<&str>) -> Result<()> {
    match owner {
        Some(owner) => refs::reject_linear_url(owner, "an email, username, display name, or @me"),
        None => Ok(()),
    }
}
