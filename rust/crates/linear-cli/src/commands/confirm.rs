//! Confirmation before destructive actions.
use crate::commands::outcome;
use crate::ctx::Ctx;
use crate::error::Result;

/// Whether a deletion goes ahead: `force` (the `--force` flag), or a yes at
/// the prompt. A no reports the cancellation.
pub fn deletion(ctx: &Ctx, force: bool, question: &str) -> Result<bool> {
    if force || ctx.confirm(question, "--force")? {
        return Ok(true);
    }
    outcome::canceled(ctx)?;
    Ok(false)
}
