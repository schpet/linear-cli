//! `issue delete`: permanently delete, for one issue or in bulk.
use crate::cli::issue::IssueDelete;
use crate::commands::bulk::BulkInput;
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};

use super::archive_or_delete::{Mode, Request};

pub fn run(ctx: &Ctx, args: &IssueDelete) -> Result<()> {
    let request = Request {
        issue_id: args.issue_id.as_deref(),
        yes: args.confirm.yes,
        bulk: BulkInput::from(&args.bulk),
    };
    super::archive_or_delete::run(ctx, Mode::Delete, &request).context("Failed to delete issue")
}
