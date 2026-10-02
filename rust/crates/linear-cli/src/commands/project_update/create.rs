//! `project-update create`: post a status update to a project.
use crate::cli::project_update::ProjectUpdateCreate;
use crate::commands::status_update::{self, Target};
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};

pub fn run(ctx: &Ctx, args: &ProjectUpdateCreate) -> Result<()> {
    status_update::create(ctx, Target::Project(&args.project_id), &args.update)
        .context("Failed to create project update")
}
