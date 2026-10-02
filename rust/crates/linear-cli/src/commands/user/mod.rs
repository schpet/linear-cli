//! `linear user`: workspace members.
mod list;

use crate::cli::user::UserCommand;
use crate::ctx::Ctx;
use crate::error::Result;

pub fn run(ctx: &Ctx, command: &UserCommand) -> Result<()> {
    match command {
        UserCommand::List(args) => list::run(ctx, args),
    }
}
