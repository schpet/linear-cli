//! `linear project`.
pub mod collections;
pub mod comment_add;
pub mod comment_list;
pub mod create;
pub mod delete;
pub mod list;
pub mod update;
pub mod view;
pub mod write;

use crate::cli::project::{ProjectCommand, ProjectCommentCommand};
use crate::ctx::Ctx;
use crate::error::{Error, Result};
use crate::graphql::pagination::PaginationError;

pub fn run(ctx: &Ctx, command: &ProjectCommand) -> Result<()> {
    match command {
        ProjectCommand::List(args) => list::run(ctx, args),
        ProjectCommand::View(args) => view::run(ctx, args),
        ProjectCommand::Create(args) => create::run(ctx, args),
        ProjectCommand::Update(args) => update::run(ctx, args),
        ProjectCommand::Delete(args) => delete::run(ctx, args),
        ProjectCommand::Comment(args) => match &args.command {
            ProjectCommentCommand::Add(args) => comment_add::run(ctx, args),
            ProjectCommentCommand::List(args) => comment_list::run(ctx, args),
        },
    }
}

/// The error for a failed walk over the pages of `what` (like "projects").
fn pagination_error(what: &str, error: PaginationError<Error>) -> Error {
    match error {
        PaginationError::Fetch { source, .. } => source,
        PaginationError::MissingCursor { .. } | PaginationError::RepeatedCursor { .. } => {
            Error::new(format!(
                "Linear reported more {what} but did not return a usable cursor"
            ))
            .with_hint("Retry the command; if it keeps happening, report it.")
        }
    }
}
