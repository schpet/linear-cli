//! `linear cycle`: a team's cycles.
mod list;
pub mod view;

use crate::cli::cycle::CycleCommand;
use crate::ctx::Ctx;
use crate::error::Result;

pub fn run(ctx: &Ctx, command: &CycleCommand) -> Result<()> {
    match command {
        CycleCommand::List(args) => list::run(ctx, args),
        CycleCommand::View(args) => view::run(ctx, args),
    }
}
