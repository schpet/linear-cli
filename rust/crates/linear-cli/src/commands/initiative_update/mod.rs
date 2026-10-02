//! `linear initiative-update`.
pub mod create;
pub mod list;

use crate::cli::initiative_update::InitiativeUpdateCommand;
use crate::ctx::Ctx;
use crate::error::Result;

pub fn run(ctx: &Ctx, command: &InitiativeUpdateCommand) -> Result<()> {
    match command {
        InitiativeUpdateCommand::Create(args) => create::run(ctx, args),
        InitiativeUpdateCommand::List(args) => list::run(ctx, args),
    }
}
