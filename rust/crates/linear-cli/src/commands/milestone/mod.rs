//! `linear milestone`: project milestones.
pub mod create;
mod delete;
mod list;
mod update;
mod view;

use crate::cli::milestone::MilestoneCommand;
use crate::ctx::Ctx;
use crate::error::Result;

pub fn run(ctx: &Ctx, command: &MilestoneCommand) -> Result<()> {
    match command {
        MilestoneCommand::List(args) => list::run(ctx, args),
        MilestoneCommand::View(args) => view::run(ctx, args),
        MilestoneCommand::Create(args) => create::run(ctx, args),
        MilestoneCommand::Update(args) => update::run(ctx, args),
        MilestoneCommand::Delete(args) => delete::run(ctx, args),
    }
}
