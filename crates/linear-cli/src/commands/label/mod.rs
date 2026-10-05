//! `linear label`: issue labels.
mod create;
mod delete;
mod list;

pub(crate) use list::for_team;

use crate::cli::label::LabelCommand;
use crate::ctx::Ctx;
use crate::error::Result;

pub fn run(ctx: &Ctx, command: &LabelCommand) -> Result<()> {
    match command {
        LabelCommand::List(args) => list::run(ctx, args),
        LabelCommand::Create(args) => create::run(ctx, args),
        LabelCommand::Delete(args) => delete::run(ctx, args),
    }
}
