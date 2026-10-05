//! `initiative-update create`: post a status update to an initiative.
use crate::cli::initiative_update::InitiativeUpdateCreate;
use crate::commands::status_update::{self, Target};
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};

pub fn run(ctx: &Ctx, args: &InitiativeUpdateCreate) -> Result<()> {
    status_update::create(ctx, Target::Initiative(&args.initiative_id), &args.update)
        .context("Failed to create initiative update")
}
