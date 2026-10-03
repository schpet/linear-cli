//! `linear config`: a workspace, team and sort order from flags or prompts,
//! written to a project config file at the repository root.
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::auth::ApiKeyInput;
use crate::cli::Sort;
use crate::cli::config::Config;
use crate::config::{RealFileSource, repo_root};
use crate::ctx::{self, Ctx};
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::user::GetViewer;
use crate::platform::prompt::Choice;
use crate::refs::{self, team::ResolvedTeam, team::TeamReference};

const BANNER: &str = "\n██      ██ ███    ██ ███████  █████  ██████      ██████ ██      ██\n██      ██ ████   ██ ██      ██   ██ ██   ██    ██      ██      ██\n██      ██ ██ ██  ██ █████   ███████ ██████     ██      ██      ██\n██      ██ ██  ██ ██ ██      ██   ██ ██   ██    ██      ██      ██\n███████ ██ ██   ████ ███████ ██   ██ ██   ██     ██████ ███████ ██\n\n";
const HEADER: &str = "# linear cli\n# https://github.com/schpet/linear-cli\n\n";

/// The values written to the config file.
#[derive(Serialize)]
struct ProjectConfig {
    workspace: String,
    team_id: String,
    issue_sort: &'static str,
}

/// The team `--team` named, or the teams to choose from.
enum Team {
    Given(ResolvedTeam),
    Choose(Vec<ResolvedTeam>),
}

pub fn run(ctx: &Ctx, args: &Config) -> Result<()> {
    generate(ctx, args).context("Failed to generate configuration")
}

fn generate(ctx: &Ctx, args: &Config) -> Result<()> {
    let workspaces = stored_workspaces(ctx)?;
    let asks = args.team.is_none() || args.sort.is_none() || workspaces.len() > 1;
    if asks && !ctx.interactive() {
        let mut flags = vec!["--team <team>", "--sort <manual|priority>"];
        if workspaces.len() > 1 {
            flags.push("--workspace <workspace>");
        }
        return Err(Error::new(
            "Some settings are missing and there is no terminal to ask for them",
        )
        .with_hint(format!("Pass {}.", flags.join(" and "))));
    }
    let team = args
        .team
        .as_deref()
        .map(|team| TeamReference::parse(team, &ctx.scope()?))
        .transpose()?;
    if asks {
        ctx.print(BANNER)?;
    }
    let workspace = match workspaces.as_slice() {
        [] => ctx.workspace().map(str::to_owned),
        [only] => Some(only.clone()),
        _ => Some(pick_workspace(ctx, &workspaces)?),
    };
    let inputs = ctx::selection_inputs(ctx.options(), workspace.as_deref());
    let key = ctx::select_key(&inputs, ctx.credentials()?);
    ctx.report_credential_warnings()?;
    let client = ctx::connect(ctx.options(), key?, &ctx.config().network_env)?;
    let (url_key, team) = ctx.spin(true, async {
        let url_key = client
            .query::<GetViewer, _>(())
            .await?
            .viewer
            .organization
            .url_key;
        let team = match &team {
            Some(lookup) => Team::Given(refs::team::resolve(&client, lookup).await?),
            None => Team::Choose(refs::team::fetch_all(&client).await?),
        };
        Ok::<_, Error>((url_key, team))
    })?;
    let team = match team {
        Team::Given(team) => team,
        Team::Choose(teams) => pick_team(ctx, teams)?,
    };
    let sort = match args.sort {
        Some(sort) => sort,
        None => ctx.prompter()?.select(
            "Select sort order:",
            vec![
                Choice::new("manual", Sort::Manual),
                Choice::new("priority", Sort::Priority),
            ],
        )?,
    };
    let config = ProjectConfig {
        workspace: url_key,
        team_id: team.key,
        issue_sort: match sort {
            Sort::Manual => "manual",
            Sort::Priority => "priority",
        },
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

fn pick_workspace(ctx: &Ctx, workspaces: &[String]) -> Result<String> {
    let default = ctx.credentials()?.default();
    let start = workspaces
        .iter()
        .position(|name| default == Some(name.as_str()))
        .unwrap_or(0);
    let choices = workspaces
        .iter()
        .map(|name| {
            let label = if default == Some(name.as_str()) {
                format!("{name} (default)")
            } else {
                name.clone()
            };
            Choice::new(label, name.clone())
        })
        .collect();
    ctx.prompter()?
        .select_from("Select workspace:", choices, start)
}

fn pick_team(ctx: &Ctx, teams: Vec<ResolvedTeam>) -> Result<ResolvedTeam> {
    if teams.is_empty() {
        return Err(Error::new("No teams available to select"));
    }
    let choices = teams
        .into_iter()
        .map(|team| Choice::new(format!("{} ({})", team.name, team.key), team))
        .collect();
    ctx.prompter()?.select("Select a team:", choices)
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
