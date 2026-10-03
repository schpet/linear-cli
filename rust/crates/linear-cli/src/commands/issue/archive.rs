//! `issue archive`: archive, for one issue or in bulk.
use crate::cli::issue::IssueArchive;
use crate::commands::bulk::BulkInput;
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};

use super::archive_or_delete::{Mode, Request};

pub fn run(ctx: &Ctx, args: &IssueArchive) -> Result<()> {
    let request = Request {
        issue_id: args.issue_id.as_deref(),
        yes: args.confirm.yes,
        bulk: BulkInput {
            argv: args.bulk.as_deref(),
            file: args.bulk_file.as_deref().map(std::path::Path::new),
            stdin: args.bulk_stdin,
        },
    };
    super::archive_or_delete::run(ctx, Mode::Archive, &request).context("Failed to archive issue")
}
