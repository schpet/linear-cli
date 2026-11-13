//! `linear skill`: the guide that teaches a coding agent to use this CLI.
//!
//! The guide ships in the binary, so what an agent reads always matches the
//! commands it can run. None of this needs configuration, credentials or the
//! network; `app` runs it before any of those load.
use crate::cli;
use crate::cli::skill::SkillCommand;
use crate::error::Result;
use crate::platform::output::Stdout;

mod guide;

#[cfg(test)]
mod tests;

pub fn run(command: &SkillCommand) -> Result<()> {
    match command {
        SkillCommand::Show(_) => show(),
    }
}

/// Prints the guide as raw Markdown: no pager, color or wrapping, so an agent
/// reads all of it in one call.
fn show() -> Result<()> {
    let stdout = Stdout::new();
    stdout.write(guide::render(&cli::command()).as_bytes())?;
    stdout.flush()
}
