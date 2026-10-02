//! `config`: pick a workspace and team, then write `.linear.toml`.
use crate::{
    auth::{ApiKeyInput, CredentialStore},
    config::{ChildEnvOverlay, ConfigOptions},
    error::Error,
    graphql::{
        bulk_error,
        envelope::GraphQlRequest,
        operations::config_generate::{Config, ConfigTeam},
        transport::GraphQlTransport,
    },
    platform::{
        markdown_assets::posix_join,
        prompt::{PlainOption, PlainSelect, PromptOutcome, PromptSession},
        selector::SelectOption,
    },
};
use cynic::QueryBuilder;
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::io::AsyncReadExt;
pub const CONTEXT: &str = "Failed to generate configuration";
pub const BANNER: &str = "\n██      ██ ███    ██ ███████  █████  ██████      ██████ ██      ██\n██      ██ ████   ██ ██      ██   ██ ██   ██    ██      ██      ██\n██      ██ ██ ██  ██ █████   ███████ ██████     ██      ██      ██\n██      ██ ██  ██ ██ ██      ██   ██ ██   ██    ██      ██      ██\n███████ ██ ██   ████ ███████ ██   ██ ██   ██     ██████ ███████ ██\n\n";
#[derive(Clone, Debug)]
pub enum WorkspaceChoice {
    Existing,
    Only(String),
    Menu {
        options: Vec<PlainOption>,
        default_index: usize,
    },
}
pub fn workspace_choice(
    options: &ConfigOptions,
    store: &CredentialStore,
    cli: Option<&str>,
) -> Result<WorkspaceChoice, Error> {
    let key = ApiKeyInput::from_options(options);
    let explicit = match key {
        ApiKeyInput::Raw { value, .. } | ApiKeyInput::Sourced { value, .. } => {
            !value.expose().is_empty()
        }
        ApiKeyInput::Absent => false,
    } || cli.is_some_and(|v| !v.is_empty());
    if explicit {
        return Ok(WorkspaceChoice::Existing);
    }
    match store.workspaces() {
        [] => Err(Error::auth("No authentication configured")
            .with_hint("Run `linear auth login` to add a workspace.")),
        [only] => Ok(WorkspaceChoice::Only(only.clone())),
        names => {
            // Only values that actually need this native menu are preflighted.
            for name in names {
                if name.trim().is_empty() || name.chars().any(char::is_control) {
                    return Err(Error::new(
                        "Workspace names containing control characters or only whitespace cannot be selected interactively",
                    ));
                }
            }
            let default_index = names
                .iter()
                .position(|name| store.default() == Some(name.as_str()))
                .unwrap_or(0);
            Ok(WorkspaceChoice::Menu {
                options: names
                    .iter()
                    .map(|name| PlainOption {
                        label: if store.default() == Some(name.as_str()) {
                            format!("{name} (default)")
                        } else {
                            name.clone()
                        },
                        value: name.clone(),
                        script_token: name.clone(),
                    })
                    .collect(),
                default_index,
            })
        }
    }
}
pub fn request() -> GraphQlRequest<()> {
    GraphQlRequest::without_variables(Config::build(()))
}
pub async fn fetch(transport: &GraphQlTransport) -> Result<Config, Error> {
    let query = request();
    let response = transport.send_request(&query).await?;
    if let Some(error) = bulk_error::observe_source_error(&response, &query)
        .map_err(|failure| failure.into_error())?
    {
        return Err(Error::new(error.preferred_message.unwrap_or(error.message)));
    }
    crate::graphql::transport::classify_typed(response).map_err(|error| match error {
        crate::graphql::transport::TransportFailure::Response(
            crate::graphql::envelope::ResponseError::UnexpectedShape(source),
        ) => Error::new(format!(
            "Linear returned an unexpected response: {source}; no configuration written"
        ))
        .with_source(source),
        error => Error::from(error),
    })
}
pub fn prepare_teams(mut teams: Vec<ConfigTeam>) -> Result<Vec<ConfigTeam>, Error> {
    if teams.is_empty() {
        return Err(Error::new("No teams available to select"));
    }
    for team in &teams {
        if team.id.inner().trim().is_empty() || team.id.inner().chars().any(char::is_control) {
            return Err(Error::new(
                "Team IDs containing control characters or only whitespace cannot be selected interactively",
            ));
        }
    }
    teams.sort_by(|a, b| {
        crate::platform::collation::compare(&a.name.to_lowercase(), &b.name.to_lowercase())
    });
    Ok(teams)
}
pub fn team_options(teams: &[ConfigTeam]) -> Vec<SelectOption> {
    teams
        .iter()
        .map(|team| SelectOption {
            label: format!("{} ({})", team.name, team.key),
            value: team.id.inner().to_owned(),
        })
        .collect()
}
pub fn team_key<'a>(teams: &'a [ConfigTeam], id: &str) -> Result<&'a str, Error> {
    teams
        .iter()
        .find(|team| team.id.inner() == id)
        .map(|team| team.key.as_str())
        .ok_or_else(|| Error::not_found("Team", id))
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortChoice {
    Manual,
    Priority,
}
impl SortChoice {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Priority => "priority",
        }
    }
}
pub fn sort_prompt<R: Read, W: Write>(
    session: &mut PromptSession<R, W>,
) -> Result<PromptOutcome<SortChoice>, Error> {
    let options = [
        PlainOption {
            label: "manual".into(),
            value: "manual".into(),
            script_token: "manual".into(),
        },
        PlainOption {
            label: "priority".into(),
            value: "priority".into(),
            script_token: "priority".into(),
        },
    ];
    match session.select(&PlainSelect {
        message: "Select sort order:",
        options: &options,
        default_index: 0,
        default_hint: None,
    })? {
        PromptOutcome::Submitted(value) => match value.as_str() {
            "manual" => Ok(PromptOutcome::Submitted(SortChoice::Manual)),
            "priority" => Ok(PromptOutcome::Submitted(SortChoice::Priority)),
            _ => Err(Error::new("sort menu produced an unknown value")),
        },
        PromptOutcome::Interrupted => Ok(PromptOutcome::Interrupted),
        PromptOutcome::EndOfInput => Ok(PromptOutcome::EndOfInput),
    }
}
pub fn stage<T>(outcome: PromptOutcome<T>, name: &str) -> Result<PromptOutcome<T>, Error> {
    match outcome {
        PromptOutcome::EndOfInput => {
            Err(Error::new(format!("unexpected EOF while selecting {name}")))
        }
        outcome => Ok(outcome),
    }
}
pub fn template(workspace: &str, team: &str, sort: SortChoice) -> String {
    format!(
        "# linear cli\n# https://github.com/schpet/linear-cli\n\nworkspace = \"{workspace}\"\nteam_id = \"{team}\"\nissue_sort = \"{}\"\n",
        sort.as_str()
    )
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LateRoot {
    Fallback,
    Completed(String),
}
#[derive(Clone, Copy, Debug)]
pub struct GitLimits {
    pub bytes: u64,
    pub timeout: Duration,
}
impl Default for GitLimits {
    fn default() -> Self {
        Self {
            bytes: 64 * 1024,
            timeout: Duration::from_secs(30),
        }
    }
}
#[derive(Debug)]
enum ProbeFailure {
    Read(std::io::Error),
    Wait(std::io::Error),
    Cap,
}
fn git_bound(message: impl Into<String>) -> Error {
    Error::new(format!(
        "Could not find the repository root: {}",
        message.into()
    ))
}
pub async fn late_root(
    cwd: &Path,
    overlay: &ChildEnvOverlay,
    limits: GitLimits,
) -> Result<LateRoot, Error> {
    late_root_with_program(cwd, overlay, limits, Path::new("git")).await
}

/// Explicit child-program seam; the CLI always passes literal git through late_root.
pub async fn late_root_with_program(
    cwd: &Path,
    overlay: &ChildEnvOverlay,
    limits: GitLimits,
    program: &Path,
) -> Result<LateRoot, Error> {
    let cap = limits
        .bytes
        .checked_add(1)
        .ok_or_else(|| git_bound("late Git cap overflow"))?;
    let mut command = tokio::process::Command::new(program);
    command
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(cwd)
        .envs(overlay.iter())
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .stdout(Stdio::piped())
        .kill_on_drop(true);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(_) => return Ok(LateRoot::Fallback),
    };
    let Some(stdout) = child.stdout.take() else {
        let kill = child.start_kill();
        child.wait().await.map_err(|error| {
            git_bound(format!(
                "missing stdout cleanup failed: {error}; kill={kill:?}"
            ))
        })?;
        return Err(git_bound("late Git stdout pipe unavailable"));
    };
    let mut bytes = Vec::new();
    let result = tokio::time::timeout(limits.timeout, async {
        let read = async {
            stdout
                .take(cap)
                .read_to_end(&mut bytes)
                .await
                .map_err(ProbeFailure::Read)?;
            if u64::try_from(bytes.len()).is_ok_and(|n| n > limits.bytes) {
                return Err(ProbeFailure::Cap);
            }
            Ok(())
        };
        let wait = async { child.wait().await.map_err(ProbeFailure::Wait) };
        tokio::try_join!(read, wait)
    })
    .await;
    match result {
        Ok(Ok((_read, _status))) => Ok(LateRoot::Completed(
            String::from_utf8_lossy(&bytes).trim().to_owned(),
        )),
        failure => {
            let kill = child.start_kill();
            let reap = child.wait().await;
            if let Err(error) = reap {
                return Err(git_bound(format!(
                    "late Git cleanup failed; kill={kill:?}; wait={error}"
                )));
            }
            match failure {
                Ok(Err(ProbeFailure::Read(_source))) => Ok(LateRoot::Fallback),
                Ok(Err(ProbeFailure::Wait(error))) => {
                    Err(git_bound(format!("late Git wait failed: {error}")))
                }
                Ok(Err(ProbeFailure::Cap)) => Err(git_bound("late Git stdout exceeds 65536 bytes")),
                Err(_) => Err(git_bound("late Git deadline exceeded")),
                Ok(Ok(_)) => Err(Error::new("completed late Git branch became failure")),
            }
        }
    }
}
/// Join a config path; Windows normalizes it lexically, other platforms join with `/`.
fn joined(root: &str, leaf: &str) -> String {
    if cfg!(windows) {
        crate::config::lexical_config_path(&Path::new(root).join(leaf))
            .to_string_lossy()
            .into_owned()
    } else {
        posix_join(&[root, leaf])
    }
}
pub fn destination(root: &LateRoot, mut stat: impl FnMut(&Path) -> bool) -> String {
    match root {
        LateRoot::Fallback => "./.linear.toml".into(),
        LateRoot::Completed(root) => {
            let directory = joined(root, ".config");
            if stat(Path::new(&directory)) {
                joined(&directory, "linear.toml")
            } else {
                joined(root, ".linear.toml")
            }
        }
    }
}
pub fn write_config(cwd: &Path, display: &str, content: &str) -> Result<Vec<u8>, Error> {
    let path = if Path::new(display).is_absolute() {
        PathBuf::from(display)
    } else {
        cwd.join(display)
    };
    // Direct truncate/create write, no parent creation/atomic rename/TOML escaping.
    std::fs::write(&path, content).map_err(|source| {
        Error::new(format!(
            "Failed to write configuration: {display}: {source}"
        ))
        .with_source(source)
    })?;
    Ok(format!("Configuration written to {display}\n").into_bytes())
}

/// Refuses prompting when stdout is a FIFO. Call before the first prompt: after
/// the teams are fetched on the automatic path, before the workspace menu otherwise.
pub fn check_prompt_topology(stdin_tty: bool, stdout_fifo: bool) -> Result<(), Error> {
    if stdin_tty && stdout_fifo {
        return Err(Error::new("Configuration prompts require terminal or regular-file stdout when stdin is a terminal").with_hint("Keep stdout on the terminal, redirect it to a regular file, or provide piped prompt answers."));
    }
    Ok(())
}
#[cfg(unix)]
pub fn stdout_is_fifo() -> Result<bool, Error> {
    let stat = rustix::fs::fstat(std::io::stdout()).map_err(|source| {
        Error::new("Failed to inspect configuration prompt stdout").with_source(source)
    })?;
    Ok(rustix::fs::FileType::from_raw_mode(stat.st_mode) == rustix::fs::FileType::Fifo)
}
#[cfg(not(unix))]
pub fn stdout_is_fifo() -> Result<bool, Error> {
    Ok(false)
}

use crate::app::legacy::block_on_network;
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};

pub fn run(ctx: &Ctx) -> Result<()> {
    dispatch_config_generate(ctx, ctx.workspace())
}

fn dispatch_config_generate(context: &Ctx, workspace: Option<&str>) -> Result<()> {
    use crate::{
        commands::config_generate as command,
        platform::prompt::{PlainSelect, PromptOutcome, PromptSession},
    };
    let result = (|| {
        context.print(command::BANNER.as_bytes())?;
        // Borrow disjoint startup/stdout fields, not a full-context reference held by the session.
        let loaded = crate::app::legacy::Loaded::new(context)?;
        let config = &loaded.config;
        let credentials = &loaded.credentials;
        let choice = command::workspace_choice(&config.options, credentials, workspace)?;
        let mut session = None;
        let mut prompt_output = Some(context.stdout());
        let answers = (|| {
            let selected = match choice {
                command::WorkspaceChoice::Existing => workspace.map(str::to_owned),
                command::WorkspaceChoice::Only(name) => Some(name),
                command::WorkspaceChoice::Menu {
                    options,
                    default_index,
                } => {
                    if context.stdin_tty() {
                        command::check_prompt_topology(true, command::stdout_is_fifo()?)?;
                    }
                    let current = PromptSession::stdin_stdio_cr_or_lf(
                        prompt_output
                            .take()
                            .ok_or_else(|| Error::new("config prompt output already owned"))?,
                    )?;
                    session = Some(current);
                    let current = session
                        .as_mut()
                        .ok_or_else(|| Error::new("workspace session absent"))?;
                    let answer = command::stage(
                        current.select(&PlainSelect {
                            message: "Select workspace:",
                            options: &options,
                            default_index,
                            default_hint: credentials.default(),
                        })?,
                        "workspace",
                    )?;
                    match answer {
                        PromptOutcome::Submitted(name) => Some(name),
                        PromptOutcome::Interrupted => return Ok(PromptOutcome::Interrupted),
                        PromptOutcome::EndOfInput => {
                            return Err(Error::new("workspace EOF conversion absent"));
                        }
                    }
                }
            };
            if let Some(current) = session.as_mut() {
                current.suspend()?;
            }
            let transport = crate::commands::client::prepare_transport(
                &config.options,
                credentials,
                selected.as_deref(),
                &config.transport_env,
            )?;
            let data = block_on_network(command::fetch(&transport))?;
            // Validate all selectable IDs before team raw mode resumes/starts.
            let teams = command::prepare_teams(data.teams.nodes)?;
            if context.stdin_tty() {
                command::check_prompt_topology(true, command::stdout_is_fifo()?)?;
            }
            match session.as_mut() {
                Some(current) => current.resume()?,
                None => {
                    session = Some(PromptSession::stdin_stdio_cr_or_lf(
                        prompt_output
                            .take()
                            .ok_or_else(|| Error::new("config prompt output already owned"))?,
                    )?)
                }
            }
            let current = session
                .as_mut()
                .ok_or_else(|| Error::new("team session absent"))?;
            let choices = command::team_options(&teams);
            let id = match command::stage(
                current.searchable_select("Select a team:", "Search teams", &choices)?,
                "team",
            )? {
                PromptOutcome::Submitted(id) => id,
                PromptOutcome::Interrupted => return Ok(PromptOutcome::Interrupted),
                PromptOutcome::EndOfInput => {
                    return Err(Error::new("team EOF conversion absent"));
                }
            };
            let key = command::team_key(&teams, &id)?.to_owned();
            let sort = match command::stage(command::sort_prompt(current)?, "sort order")? {
                PromptOutcome::Submitted(sort) => sort,
                PromptOutcome::Interrupted => return Ok(PromptOutcome::Interrupted),
                PromptOutcome::EndOfInput => {
                    return Err(Error::new("sort EOF conversion absent"));
                }
            };
            Ok(PromptOutcome::Submitted((
                data.viewer.organization.url_key,
                key,
                sort,
            )))
        })();
        // Always finish immediately after sort/control/error, before late Git and local IO.
        let answers = match session.as_mut() {
            Some(current) => current.finish_result(answers)?,
            None => answers?,
        };
        drop(session);
        let (written_workspace, key, sort) = match answers {
            PromptOutcome::Submitted(values) => values,
            PromptOutcome::Interrupted => return Err(Error::cancelled()),
            PromptOutcome::EndOfInput => {
                return Err(Error::new("config stage EOF conversion absent"));
            }
        };
        let root = block_on_network(command::late_root(
            context.cwd(),
            &config.child_env,
            command::GitLimits::default(),
        ))?;
        let path = command::destination(&root, |path| {
            let absolute = if path.is_absolute() {
                path.to_owned()
            } else {
                context.cwd().join(path)
            };
            std::fs::metadata(absolute).is_ok() // follows symlinks, any stat success, ordinary errors fallback.
        });
        let content = command::template(&written_workspace, &key, sort);
        let output = command::write_config(context.cwd(), &path, &content)?;
        context.print(&output)?;
        Ok(())
    })();
    result.context(command::CONTEXT)
}
