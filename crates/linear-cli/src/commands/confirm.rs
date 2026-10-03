//! Confirmation before destructive actions.
use crate::commands::outcome;
use crate::ctx::Ctx;
use crate::error::Result;

/// Whether a deletion goes ahead: `yes` (the `--yes` flag), or a yes at the
/// prompt. A no reports the cancellation.
pub fn deletion(ctx: &Ctx, yes: bool, question: &str) -> Result<bool> {
    if yes || ctx.confirm(question, "--yes")? {
        return Ok(true);
    }
    outcome::canceled(ctx)?;
    Ok(false)
}
