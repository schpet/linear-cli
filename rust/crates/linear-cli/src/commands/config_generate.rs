//! `linear config`: pick a workspace, team and sort order, then write a
//! project config file at the repository root.
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::auth::ApiKeyInput;
use crate::config::{RealFileSource, repo_root};
use crate::ctx::{self, Ctx};
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::viewer;
use crate::platform::prompt::{PlainOption, PlainSelect, PromptOutcome, PromptSession};
use crate::platform::selector::SelectOption;
use crate::refs::{ResolvedTeam, fetch_all_teams_with_transport};

const BANNER: &str = "\n██      ██ ███    ██ ███████  █████  ██████      ██████ ██      ██\n██      ██ ████   ██ ██      ██   ██ ██   ██    ██      ██      ██\n██      ██ ██ ██  ██ █████   ███████ ██████     ██      ██      ██\n██      ██ ██  ██ ██ ██      ██   ██ ██   ██    ██      ██      ██\n███████ ██ ██   ████ ███████ ██   ██ ██   ██     ██████ ███████ ██\n\n";
const HEADER: &str = "# linear cli\n# https://github.com/schpet/linear-cli\n\n";
const SORT_ORDERS: [&str; 2] = ["manual", "priority"];

/// The values written to the config file.
#[derive(Serialize)]
struct ProjectConfig {
    workspace: String,
    team_id: String,
    issue_sort: String,
}

pub fn run(ctx: &Ctx) -> Result<()> {
    generate(ctx).context("Failed to generate configuration")
}

fn generate(ctx: &Ctx) -> Result<()> {
    ctx.print(BANNER)?;
    let workspaces = stored_workspaces(ctx)?;
    let mut session = ctx.prompts()?;
    let answers = ask(ctx, &workspaces, &mut session);
    let config = match session.finish_result(answers)? {
        PromptOutcome::Submitted(config) => config,
        PromptOutcome::Interrupted => return Err(Error::cancelled()),
        PromptOutcome::EndOfInput => {
            return Err(Error::new("Unexpected end of input at a prompt"));
        }
    };
    let path = destination(ctx.cwd());
    let contents = format!(
        "{HEADER}{}",
        toml::to_string(&config).expect("project config always serializes as TOML")
    );
    std::fs::write(&path, contents).map_err(|error| {
        Error::new(format!("Could not write {}: {error}", path.display())).with_source(error)
    })?;
    ctx.print(format!("Configuration written to {}\n", path.display()))
}

/// The stored workspaces to choose from: none when `--workspace` or an API
/// key from the environment or a config file already decides.
fn stored_workspaces(ctx: &Ctx) -> Result<Vec<String>> {
    let decided = ctx.workspace().is_some()
        || match ApiKeyInput::from_options(ctx.options()) {
            ApiKeyInput::Raw { value, .. } | ApiKeyInput::Sourced { value, .. } => {
                !value.expose().is_empty()
            }
            ApiKeyInput::Absent => false,
        };
    if decided {
        return Ok(Vec::new());
    }
    let store = ctx.credentials()?;
    if store.workspaces().is_empty() {
        return Err(Error::auth("No authentication configured")
            .with_hint("Run `linear auth login` to add a workspace."));
    }
    Ok(store.workspaces().to_vec())
}

fn ask<R: std::io::Read, W: std::io::Write>(
    ctx: &Ctx,
    workspaces: &[String],
    session: &mut PromptSession<R, W>,
) -> Result<PromptOutcome<ProjectConfig>> {
    macro_rules! answer {
        ($outcome:expr) => {
            match $outcome? {
                PromptOutcome::Submitted(value) => value,
                PromptOutcome::Interrupted => return Ok(PromptOutcome::Interrupted),
                PromptOutcome::EndOfInput => return Ok(PromptOutcome::EndOfInput),
            }
        };
    }
    let workspace = match workspaces {
        [] => ctx.workspace().map(str::to_owned),
        [only] => Some(only.clone()),
        _ => Some(answer!(pick_workspace(ctx, workspaces, session))),
    };
    session.suspend()?;
    let inputs = ctx::selection_inputs(ctx.options(), workspace.as_deref());
    let client = ctx::connect(
        ctx.options(),
        ctx.credentials()?,
        &inputs,
        &ctx.config().transport_env,
    )?;
    ctx.report_credential_warnings()?;
    let (url_key, teams) = ctx.spin(true, async {
        let url_key = viewer::url_key(&client).await?;
        let teams = fetch_all_teams_with_transport(&client).await?;
        Ok::<_, Error>((url_key, teams))
    })?;
    if teams.is_empty() {
        return Err(Error::new("No teams available to select"));
    }
    session.resume()?;
    let team_id =
        answer!(session.searchable_select("Select a team:", "Search teams", &team_options(&teams)));
    let team = teams
        .iter()
        .find(|team| team.id == team_id)
        .expect("the picked team is one of the options");
    let sort_options: Vec<PlainOption> = SORT_ORDERS.iter().map(|sort| option(sort)).collect();
    let issue_sort = answer!(session.select(&PlainSelect {
        message: "Select sort order:",
        options: &sort_options,
        default_index: 0,
        default_hint: None,
    }));
    Ok(PromptOutcome::Submitted(ProjectConfig {
        workspace: url_key,
        team_id: team.key.clone(),
        issue_sort,
    }))
}

fn pick_workspace<R: std::io::Read, W: std::io::Write>(
    ctx: &Ctx,
    workspaces: &[String],
    session: &mut PromptSession<R, W>,
) -> Result<PromptOutcome<String>> {
    let default = ctx.credentials()?.default();
    let options: Vec<PlainOption> = workspaces
        .iter()
        .map(|name| PlainOption {
            label: if default == Some(name.as_str()) {
                format!("{name} (default)")
            } else {
                name.clone()
            },
            value: name.clone(),
            script_token: name.clone(),
        })
        .collect();
    session.select(&PlainSelect {
        message: "Select workspace:",
        options: &options,
        default_index: workspaces
            .iter()
            .position(|name| default == Some(name.as_str()))
            .unwrap_or(0),
        default_hint: default,
    })
}

fn option(value: &str) -> PlainOption {
    PlainOption {
        label: value.to_owned(),
        value: value.to_owned(),
        script_token: value.to_owned(),
    }
}

fn team_options(teams: &[ResolvedTeam]) -> Vec<SelectOption> {
    teams
        .iter()
        .map(|team| SelectOption {
            label: format!("{} ({})", team.name, team.key),
            value: team.id.clone(),
        })
        .collect()
}

/// `.config/linear.toml` at the repository root when `.config` exists there,
/// else `.linear.toml` at the root, or in `cwd` outside a repository. Each is
/// a place config discovery looks.
fn destination(cwd: &Path) -> PathBuf {
    let Some(root) = repo_root(cwd, &RealFileSource) else {
        return cwd.join(".linear.toml");
    };
    let config_dir = root.join(".config");
    if config_dir.is_dir() {
        config_dir.join("linear.toml")
    } else {
        root.join(".linear.toml")
    }
}
