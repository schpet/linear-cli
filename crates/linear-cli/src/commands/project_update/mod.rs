//! `linear project-update`.
pub mod create;
pub mod list;

use crate::cli::project_update::ProjectUpdateCommand;
use crate::ctx::Ctx;
use crate::error::Result;

pub fn run(ctx: &Ctx, command: &ProjectUpdateCommand) -> Result<()> {
    match command {
        ProjectUpdateCommand::Create(args) => create::run(ctx, args),
        ProjectUpdateCommand::List(args) => list::run(ctx, args),
    }
}
