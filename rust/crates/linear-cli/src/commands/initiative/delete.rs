//! `initiative delete`: permanently delete, for one initiative or in bulk.
use crate::cli::initiative::InitiativeDelete;
use crate::commands::bulk::BulkInput;
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};

use super::archive_or_delete::{Mode, Request};

pub fn run(ctx: &Ctx, args: &InitiativeDelete) -> Result<()> {
    let request = Request {
        initiative: args.initiative_id.as_deref(),
        yes: args.confirm.yes,
        bulk: BulkInput {
            argv: args.bulk.as_deref(),
            file: args.bulk_file.as_deref().map(std::path::Path::new),
            stdin: args.bulk_stdin,
        },
    };
    super::archive_or_delete::run(ctx, Mode::Delete, &request)
        .context("Failed to delete initiative")
}
