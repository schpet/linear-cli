//! Asking for a reference until it names something that exists, so a typo is
//! caught at its question rather than after the final confirmation.
use std::future::Future;

use crate::cli::values::UserRef;
use crate::ctx::Ctx;
use crate::error::{Error, Result};
use crate::platform::prompt::{Prompter, Text};
use crate::platform::style;
use crate::refs;

/// Asks with `ask` until `look_up` finds what the answer names. Each lookup
/// runs behind a spinner; a failed one is shown under the question, which is
/// then asked again (Esc or Ctrl-C cancels). `None` when `ask` takes a blank
/// answer as no value.
pub fn ask_until_found<A, T, Fut>(
    ctx: &Ctx,
    mut ask: impl FnMut() -> Result<Option<A>>,
    mut look_up: impl FnMut(A) -> Fut,
) -> Result<Option<T>>
where
    Fut: Future<Output = Result<T>>,
{
    loop {
        let Some(answer) = ask()? else {
            return Ok(None);
        };
        match ctx.spin(true, look_up(answer)) {
            Ok(found) => return Ok(Some(found)),
            Err(error) => show(ctx, &error)?,
        }
    }
}

/// Asks `message` for a user (blank for none) until the answer names
/// exactly one; `role` names the user in a not-found error, like "Lead".
/// A failed lookup is shown under the question asked again, which starts
/// from the refused answer, the way the prompt shows an answer it refuses
/// itself.
pub fn ask_user(
    ctx: &Ctx,
    prompter: &Prompter<'_>,
    message: &str,
    role: &str,
) -> Result<Option<String>> {
    let client = ctx.client()?;
    let check = |raw: &str| raw.parse::<UserRef>().map(drop);
    let mut refused: Option<(String, String)> = None;
    loop {
        let mut text = Text::new(message).with_check(&check);
        if let Some((input, reason)) = &refused {
            text = text.with_retry(input, reason);
        }
        let answer = prompter.text(text)?;
        if answer.is_empty() {
            return Ok(None);
        }
        let user: UserRef = answer
            .parse()
            .expect("the prompt only accepts answers that parse");
        match ctx.spin(true, refs::user::resolve(client, &user, role)) {
            Ok(id) => return Ok(Some(id)),
            Err(error) => refused = Some((answer, error.message().to_owned())),
        }
    }
}

fn show(ctx: &Ctx, error: &Error) -> Result<()> {
    let color = ctx.terminal().stderr_color();
    let mut lines = format!("{}\n", style::red(&format!("✗ {error}"), color));
    if let Some(hint) = error.hint() {
        lines.push_str(&format!("{}\n", style::gray(&format!("  {hint}"), color)));
    }
    ctx.eprint(lines)
}
