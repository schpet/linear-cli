//! The final question before a destructive action, or before sending content
//! that was typed at a prompt or in the editor.
use crate::commands::outcome;
use crate::ctx::Ctx;
use crate::error::Result;

/// Whether the command goes ahead: `yes` (the `--yes` flag), or a yes at a
/// question that defaults to no, so a stray Enter never deletes anything or
/// sends a half-finished draft. A no reports the cancellation. Commands whose
/// content comes entirely from flags do not ask before creating or updating.
pub fn proceed(ctx: &Ctx, yes: bool, question: &str) -> Result<bool> {
    if yes || ctx.confirm(question, "--yes")? {
        return Ok(true);
    }
    outcome::canceled(ctx)?;
    Ok(false)
}
