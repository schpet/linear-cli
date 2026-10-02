//! Confirmation before destructive actions.
use crate::ctx::Ctx;
use crate::error::Result;

/// Whether a deletion goes ahead: `force` (the `--force` flag), or a yes at
/// the prompt. A no prints "Deletion canceled".
pub fn deletion(ctx: &Ctx, force: bool, question: &str) -> Result<bool> {
    if force || ctx.confirm(question, "--force")? {
        return Ok(true);
    }
    ctx.print("Deletion canceled\n")?;
    Ok(false)
}
