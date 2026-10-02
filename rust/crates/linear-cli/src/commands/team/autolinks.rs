//! `team autolinks`: register a GitHub autolink so `ENG-123` in the current
//! repository links to the Linear issue, using the `gh` CLI.
use std::io::ErrorKind;
use std::num::NonZeroU8;
use std::process::{Command, Stdio};

use crate::cli::team::TeamAutolinks;
use crate::commands::team_key::configured_team_key;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};

pub fn run(ctx: &Ctx, _args: &TeamAutolinks) -> Result<()> {
    autolinks(ctx).context("Failed to configure autolinks")
}

fn autolinks(ctx: &Ctx) -> Result<()> {
    let team = configured_team_key(ctx.options()).ok_or_else(|| {
        Error::new("No team is configured").with_hint("Run `linear config` to set a team.")
    })?;
    let workspace = ctx.workspace_url_key()?;
    ctx.flush()?;
    let status = Command::new("gh")
        .args([
            "api".to_owned(),
            "repos/{owner}/{repo}/autolinks".to_owned(),
            "-f".to_owned(),
            format!("key_prefix={team}-"),
            "-f".to_owned(),
            format!("url_template=https://linear.app/{workspace}/issue/{team}-<num>"),
        ])
        .current_dir(ctx.cwd())
        .envs(ctx.config().child_env.iter())
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|error| {
            match error.kind() {
                ErrorKind::NotFound => Error::new("The GitHub CLI (`gh`) was not found")
                    .with_hint("Install it from https://cli.github.com."),
                _ => Error::new(format!("Failed to run `gh`: {error}")),
            }
            .with_source(error)
        })?;
    if status.success() {
        return Ok(());
    }
    // `gh` has already reported why it failed; exit with its status.
    let code = status
        .code()
        .and_then(|code| u8::try_from(code).ok())
        .and_then(NonZeroU8::new)
        .unwrap_or(NonZeroU8::MIN);
    Err(Error::exit(code))
}
