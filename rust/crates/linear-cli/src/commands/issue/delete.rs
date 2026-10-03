//! `issue delete`: permanently delete, for one issue or in bulk.
use crate::cli::issue::IssueDelete;
use crate::commands::bulk::BulkInput;
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};

use super::archive_or_delete::{Mode, Request};

pub fn run(ctx: &Ctx, args: &IssueDelete) -> Result<()> {
    let request = Request {
        issue_id: args.issue_id.as_deref(),
        confirmed: args.confirm,
        bulk: BulkInput {
            argv: args.bulk.as_deref(),
            file: args.bulk_file.as_deref().map(std::path::Path::new),
            stdin: args.bulk_stdin,
        },
    };
    super::archive_or_delete::run(ctx, Mode::Delete, &request).context("Failed to delete issue")
}
