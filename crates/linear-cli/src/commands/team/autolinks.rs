//! `team autolinks`: register a GitHub autolink so `ENG-123` in the current
//! repository links to the Linear issue, using the `gh` CLI.
use crate::cli::team::TeamAutolinks;
use crate::commands::team_key::configured_team_key;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::platform::gh;

pub fn run(ctx: &Ctx, _args: &TeamAutolinks) -> Result<()> {
    autolinks(ctx).context("Failed to configure autolinks")
}

fn autolinks(ctx: &Ctx) -> Result<()> {
    let team = configured_team_key(ctx.options()).ok_or_else(|| {
        Error::new("No team is configured").with_hint("Run `linear config` to set a team.")
    })?;
    let workspace = ctx.workspace_url_key()?;
    ctx.flush()?;
    gh::run(
        [
            "api".to_owned(),
            "repos/{owner}/{repo}/autolinks".to_owned(),
            "-f".to_owned(),
            format!("key_prefix={team}-"),
            "-f".to_owned(),
            format!("url_template=https://linear.app/{workspace}/issue/{team}-<num>"),
        ],
        ctx.cwd(),
        &ctx.config().child_env,
    )
}
