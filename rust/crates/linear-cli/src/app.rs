//! The process entry point: load configuration, build the [`Ctx`], run the
//! selected command and report its error.
mod issue_write_dispatch;
mod legacy;
use issue_write_dispatch::{dispatch_issue_create, dispatch_issue_update};
use legacy::{block_on_network, spinner};

use std::error::Error as StdError;

use crate::auth::credentials_path;
use crate::cli::{self, Cli, RootCommand};
use crate::commands;
use crate::commands::completions::{self, CompletionShell};
use crate::commands::team_key::configured_team_key;
use crate::commands::{
    auth_default, auth_list, auth_token, auth_whoami, client, comment_add, cycle_list, cycle_view,
    document_comment_list, initiative_bulk, initiative_comment_list, initiative_create,
    initiative_list, initiative_projects, initiative_unarchive, initiative_update_list,
    initiative_view, issue_comment_delete, issue_comment_list, issue_details, label_create,
    label_delete, label_list, milestone_create, milestone_delete, milestone_list, milestone_update,
    milestone_view, project_comment_list, project_delete, project_list, project_update_list,
    project_view, table, team_create, team_id, team_list, team_members, team_states, template_list,
    template_view, user_list,
};
use crate::config::{
    DisplaySettings, OsFamily, ProcessEnvSnapshot, RealFileSource, load_startup, render_diagnostic,
};
use crate::ctx::{Ctx, CtxInit, Terminal};
use crate::error::{Error, ErrorKind, Result, ResultExt};
use crate::platform::output::{self, Stdout, StdoutWriter};
use crate::platform::style;
use crate::refs::{
    InitiativeReference, ProjectReference, WorkspaceScope, prepare_initiative_lookup,
    prepare_project_lookup, prepare_team_lookup, resolve_document_reference,
    resolve_initiative_with_transport, resolve_project_with_transport, resolve_team_with_transport,
};

/// Runs a parsed command line and returns the process exit status.
pub fn main(cli: Cli) -> u8 {
    let mut settings = DisplaySettings {
        debug: false,
        no_color: false,
    };
    let result = run(cli, &mut settings);
    match result {
        Ok(()) => 0,
        Err(error) => {
            report(&error, settings);
            error.exit_code()
        }
    }
}

fn run(cli: Cli, settings: &mut DisplaySettings) -> Result<()> {
    let Cli { workspace, command } = cli;
    let command = match command {
        RootCommand::Completions(action) => return completions_command(&action),
        RootCommand::Markdown(_) => return markdown(),
        command => command,
    };
    let cwd = std::env::current_dir().map_err(|error| {
        Error::new(format!("Failed to read the working directory: {error}")).with_source(error)
    })?;
    let os = if cfg!(windows) {
        OsFamily::Windows
    } else {
        OsFamily::Unix
    };
    let process = ProcessEnvSnapshot::capture(cwd.clone(), os)
        .map_err(|error| Error::new(format!("Invalid environment: {error}")))?;
    let report = load_startup(&process, &RealFileSource);
    *settings = report.settings;
    let terminal = Terminal::detect(report.settings.no_color);
    for diagnostic in &report.diagnostics {
        output::eprint(render_diagnostic(diagnostic, terminal.stderr_color()).as_bytes())?;
    }
    let config = report.result?;
    let env = |name| process.inputs.env(name);
    let ctx = Ctx::new(CtxInit {
        config,
        debug: report.settings.debug,
        workspace,
        cwd,
        terminal,
        credentials_path: credentials_path(os, env("XDG_CONFIG_HOME"), env("HOME"), env("APPDATA")),
    })?;
    let result = dispatch(&ctx, command);
    // Output written before a failure still reaches the reader.
    let flushed = ctx.flush();
    result.and(flushed)
}

/// Prints `error` to stderr: `✗ message`, an optional hint, and under
/// `LINEAR_DEBUG` the debug detail and source chain.
fn report(error: &Error, settings: DisplaySettings) {
    let lines = match error.kind() {
        ErrorKind::Cancelled | ErrorKind::Exit(_) | ErrorKind::BrokenPipe => return,
        ErrorKind::Usage => match error.usage_error() {
            Some(usage) => usage.render().to_string(),
            None => format!("{error}\n"),
        },
        ErrorKind::Other | ErrorKind::Auth | ErrorKind::NotFound => {
            let color = Terminal::detect(settings.no_color).stderr_color();
            let mut lines = format!("{}\n", style::red(&format!("✗ {error}"), color));
            if let Some(hint) = error.hint() {
                lines.push_str(&format!("{}\n", style::gray(&format!("  {hint}"), color)));
            }
            if settings.debug {
                if let Some(detail) = error.debug_detail() {
                    lines.push_str(&format!("  debug: {detail}\n"));
                }
                let mut source = error.source();
                while let Some(cause) = source {
                    lines.push_str(&format!("  caused by: {cause}\n"));
                    source = cause.source();
                }
            }
            lines
        }
    };
    // Nothing is left to report a failure to.
    let _ignored = output::eprint(lines.as_bytes());
}

fn completions_command(action: &cli::completions::Completions) -> Result<()> {
    use cli::completions::CompletionsCommand;
    let output = match &action.command {
        CompletionsCommand::Bash(action) => {
            completions::script(CompletionShell::Bash, action.name.as_deref())?
        }
        CompletionsCommand::Fish(action) => {
            completions::script(CompletionShell::Fish, action.name.as_deref())?
        }
        CompletionsCommand::Zsh(action) => {
            completions::script(CompletionShell::Zsh, action.name.as_deref())?
        }
        CompletionsCommand::Complete(action) => completions::complete(action)?,
    };
    let stdout = Stdout::new();
    stdout.write(&output)?;
    stdout.flush()
}

fn markdown() -> Result<()> {
    let stdout = Stdout::new();
    stdout.write(include_str!("cli/markdown.txt").as_bytes())?;
    stdout.flush()
}

fn dispatch(ctx: &Ctx, command: RootCommand) -> Result<()> {
    let workspace = ctx.workspace();
    let context = ctx;
    match command {
        RootCommand::Auth(action) => match action.command {
            cli::auth::AuthCommand::Login(action) => dispatch_auth_login(context, &action),
            cli::auth::AuthCommand::Logout(action) => dispatch_auth_logout(context, &action),
            cli::auth::AuthCommand::List(action) => dispatch_auth_list(context, &action, workspace),
            cli::auth::AuthCommand::Default(action) => dispatch_auth_default(context, &action),
            cli::auth::AuthCommand::Token(_) => dispatch_auth_token(context, workspace),
            cli::auth::AuthCommand::Whoami(action) => {
                dispatch_auth_whoami(context, &action, workspace)
            }
            cli::auth::AuthCommand::Migrate(_) => dispatch_auth_migrate(context),
        },
        RootCommand::Issue(action) => match action.command {
            cli::issue::IssueCommand::Id(_) => dispatch_issue_id(context),
            cli::issue::IssueCommand::Mine(action) => {
                dispatch_issue_mine(context, &action, workspace)
            }
            cli::issue::IssueCommand::Query(action) => {
                dispatch_issue_query(context, &action, workspace)
            }
            cli::issue::IssueCommand::Title(action) => dispatch_issue_detail(
                context,
                action.issue_id.as_deref(),
                workspace,
                IssueDetailField::Title,
            ),
            cli::issue::IssueCommand::Start(action) => {
                dispatch_issue_start(context, &action, workspace)
            }
            cli::issue::IssueCommand::View(action) => {
                dispatch_issue_view(context, &action, workspace)
            }
            cli::issue::IssueCommand::Url(action) => dispatch_issue_detail(
                context,
                action.issue_id.as_deref(),
                workspace,
                IssueDetailField::Url,
            ),
            cli::issue::IssueCommand::Describe(action) => {
                dispatch_issue_describe(context, &action, workspace)
            }
            cli::issue::IssueCommand::Commits(action) => {
                dispatch_issue_commits(context, &action, workspace)
            }
            cli::issue::IssueCommand::PullRequest(action) => {
                dispatch_issue_pull_request(context, &action, workspace)
            }
            cli::issue::IssueCommand::Archive(action) => dispatch_issue_archive_delete(
                context,
                IssueArchiveDeleteAction {
                    target: action.issue_id.as_deref(),
                    confirm: action.confirm,
                    bulk: initiative_bulk::BulkInput {
                        argv: action.bulk.as_deref(),
                        file: action.bulk_file.as_deref().map(std::path::Path::new),
                        stdin: action.bulk_stdin,
                    },
                },
                crate::commands::issue_archive_delete::Mode::Archive,
                workspace,
            )
            .context("Failed to archive issue"),
            cli::issue::IssueCommand::Delete(action) => dispatch_issue_archive_delete(
                context,
                IssueArchiveDeleteAction {
                    target: action.issue_id.as_deref(),
                    confirm: action.confirm,
                    bulk: initiative_bulk::BulkInput {
                        argv: action.bulk.as_deref(),
                        file: action.bulk_file.as_deref().map(std::path::Path::new),
                        stdin: action.bulk_stdin,
                    },
                },
                crate::commands::issue_archive_delete::Mode::Delete,
                workspace,
            )
            .context("Failed to delete issue"),
            cli::issue::IssueCommand::Create(action) => {
                dispatch_issue_create(context, &action, workspace)
            }
            cli::issue::IssueCommand::Update(action) => {
                dispatch_issue_update(context, &action, workspace)
            }
            cli::issue::IssueCommand::Comment(action) => match action.command {
                cli::issue::IssueCommentCommand::Add(action) => {
                    dispatch_issue_comment_add(context, &action, workspace)
                }
                cli::issue::IssueCommentCommand::Delete(action) => {
                    dispatch_issue_comment_delete(context, &action, workspace)
                }
                cli::issue::IssueCommentCommand::Update(action) => {
                    dispatch_issue_comment_update(context, &action, workspace)
                }
                cli::issue::IssueCommentCommand::List(action) => {
                    dispatch_issue_comment_list(context, &action, workspace)
                }
            },
            cli::issue::IssueCommand::Attach(action) => {
                dispatch_issue_attach(context, &action, workspace)
            }
            cli::issue::IssueCommand::Link(action) => {
                dispatch_issue_link(context, &action, workspace)
            }
            cli::issue::IssueCommand::Relation(action) => match action.command {
                cli::issue::IssueRelationCommand::Add(action) => {
                    dispatch_issue_relation_add(context, &action, workspace)
                }
                cli::issue::IssueRelationCommand::Delete(action) => {
                    dispatch_issue_relation_delete(context, &action, workspace)
                }
                cli::issue::IssueRelationCommand::List(action) => {
                    dispatch_issue_relation_list(context, &action, workspace)
                }
            },
            cli::issue::IssueCommand::AgentSession(action) => match action.command {
                cli::issue::IssueAgentSessionCommand::List(action) => {
                    dispatch_agent_session_list(context, &action, workspace)
                }
                cli::issue::IssueAgentSessionCommand::View(action) => {
                    dispatch_agent_session_view(context, &action, workspace)
                }
            },
        },
        RootCommand::Team(action) => match action.command {
            cli::team::TeamCommand::Create(action) => {
                dispatch_team_create(context, &action, workspace)
            }
            cli::team::TeamCommand::Delete(action) => {
                dispatch_team_delete(context, &action, workspace)
            }
            cli::team::TeamCommand::List(action) => dispatch_team_list(context, &action, workspace),
            cli::team::TeamCommand::Id(action) => dispatch_team_id(context, &action, workspace),
            cli::team::TeamCommand::Autolinks(_) => {
                crate::commands::team_autolinks::execute(context.config(), workspace, context.cwd())
            }
            cli::team::TeamCommand::Members(action) => {
                dispatch_team_members(context, &action, workspace)
            }
            cli::team::TeamCommand::States(action) => {
                dispatch_team_states(context, &action, workspace)
            }
        },
        RootCommand::User(action) => match action.command {
            cli::user::UserCommand::List(action) => dispatch_user_list(context, &action, workspace),
        },
        RootCommand::Project(action) => match action.command {
            cli::project::ProjectCommand::List(action) => {
                dispatch_project_list(context, &action, workspace)
            }
            cli::project::ProjectCommand::View(action) => {
                dispatch_project_view(context, &action, workspace)
            }
            cli::project::ProjectCommand::Create(action) => {
                dispatch_project_create(context, &action, workspace)
            }
            cli::project::ProjectCommand::Update(action) => {
                dispatch_project_update(context, &action, workspace)
            }
            cli::project::ProjectCommand::Delete(action) => {
                dispatch_project_delete(context, &action, workspace)
            }
            cli::project::ProjectCommand::Comment(action) => match action.command {
                cli::project::ProjectCommentCommand::Add(action) => {
                    dispatch_project_comment_add(context, &action, workspace)
                }
                cli::project::ProjectCommentCommand::List(action) => {
                    dispatch_project_comment_list(context, &action, workspace)
                }
            },
        },
        RootCommand::ProjectUpdate(action) => match action.command {
            cli::project_update::ProjectUpdateCommand::Create(action) => dispatch_update_create(
                context,
                UpdateCreateAction {
                    original: &action.project_id,
                    body: action.body.as_deref(),
                    file: action.body_file.as_deref(),
                    health: action.health.as_deref(),
                    interactive: action.interactive,
                },
                crate::commands::update_create::Mode::Project,
                workspace,
            ),
            cli::project_update::ProjectUpdateCommand::List(action) => {
                dispatch_project_update_list(context, &action, workspace)
            }
        },
        RootCommand::Cycle(action) => match action.command {
            cli::cycle::CycleCommand::List(action) => {
                dispatch_cycle_list(context, &action, workspace)
            }
            cli::cycle::CycleCommand::View(action) => {
                dispatch_cycle_view(context, &action, workspace)
            }
        },
        RootCommand::Milestone(action) => match action.command {
            cli::milestone::MilestoneCommand::List(action) => {
                dispatch_milestone_list(context, &action, workspace)
            }
            cli::milestone::MilestoneCommand::View(action) => {
                dispatch_milestone_view(context, &action, workspace)
            }
            cli::milestone::MilestoneCommand::Create(action) => {
                dispatch_milestone_create(context, &action, workspace)
            }
            cli::milestone::MilestoneCommand::Update(action) => {
                dispatch_milestone_update(context, &action, workspace)
            }
            cli::milestone::MilestoneCommand::Delete(action) => {
                dispatch_milestone_delete(context, &action, workspace)
            }
        },
        RootCommand::Initiative(action) => match action.command {
            cli::initiative::InitiativeCommand::List(action) => {
                dispatch_initiative_list(context, &action, workspace)
            }
            cli::initiative::InitiativeCommand::View(action) => {
                dispatch_initiative_view(context, &action, workspace)
            }
            cli::initiative::InitiativeCommand::Create(action) => {
                dispatch_initiative_create(context, &action, workspace)
            }
            cli::initiative::InitiativeCommand::Archive(action) => dispatch_initiative_bulk(
                context,
                InitiativeAction {
                    target: action.initiative_id.as_deref(),
                    force: action.force,
                    bulk: initiative_bulk::BulkInput {
                        argv: action.bulk.as_deref(),
                        file: action.bulk_file.as_deref().map(std::path::Path::new),
                        stdin: action.bulk_stdin,
                    },
                },
                initiative_bulk::Mode::Archive,
                workspace,
            ),
            cli::initiative::InitiativeCommand::Update(action) => {
                dispatch_initiative_update(context, &action, workspace)
            }
            cli::initiative::InitiativeCommand::Unarchive(action) => {
                dispatch_initiative_unarchive(context, &action, workspace)
            }
            cli::initiative::InitiativeCommand::Delete(action) => dispatch_initiative_bulk(
                context,
                InitiativeAction {
                    target: action.initiative_id.as_deref(),
                    force: action.force,
                    bulk: initiative_bulk::BulkInput {
                        argv: action.bulk.as_deref(),
                        file: action.bulk_file.as_deref().map(std::path::Path::new),
                        stdin: action.bulk_stdin,
                    },
                },
                initiative_bulk::Mode::Delete,
                workspace,
            ),
            cli::initiative::InitiativeCommand::AddProject(action) => dispatch_initiative_projects(
                context,
                &action.initiative,
                &action.project,
                action.sort_order,
                true,
                initiative_projects::Mode::Add,
                workspace,
            ),
            cli::initiative::InitiativeCommand::RemoveProject(action) => {
                dispatch_initiative_projects(
                    context,
                    &action.initiative,
                    &action.project,
                    None,
                    action.force,
                    initiative_projects::Mode::Remove,
                    workspace,
                )
            }
            cli::initiative::InitiativeCommand::Comment(action) => match action.command {
                cli::initiative::InitiativeCommentCommand::Add(action) => {
                    dispatch_initiative_comment_add(context, &action, workspace)
                }
                cli::initiative::InitiativeCommentCommand::List(action) => {
                    dispatch_initiative_comment_list(context, &action, workspace)
                }
            },
        },
        RootCommand::InitiativeUpdate(action) => match action.command {
            cli::initiative_update::InitiativeUpdateCommand::Create(action) => {
                dispatch_update_create(
                    context,
                    UpdateCreateAction {
                        original: &action.initiative_id,
                        body: action.body.as_deref(),
                        file: action.body_file.as_deref(),
                        health: action.health.as_deref(),
                        interactive: action.interactive,
                    },
                    crate::commands::update_create::Mode::Initiative,
                    workspace,
                )
            }
            cli::initiative_update::InitiativeUpdateCommand::List(action) => {
                dispatch_initiative_update_list(context, &action, workspace)
            }
        },
        RootCommand::Label(action) => match action.command {
            cli::label::LabelCommand::List(action) => {
                dispatch_label_list(context, &action, workspace)
            }
            cli::label::LabelCommand::Create(action) => {
                dispatch_label_create(context, &action, workspace)
            }
            cli::label::LabelCommand::Delete(action) => {
                dispatch_label_delete(context, &action, workspace)
            }
        },
        RootCommand::Template(action) => match action.command {
            cli::template::TemplateCommand::List(action) => {
                dispatch_template_list(context, &action, workspace)
            }
            cli::template::TemplateCommand::View(action) => {
                dispatch_template_view(context, &action, workspace)
            }
        },
        RootCommand::Document(action) => match action.command {
            cli::document::DocumentCommand::List(action) => {
                dispatch_document_list(context, &action, workspace)
            }
            cli::document::DocumentCommand::View(action) => {
                dispatch_document_view(context, &action, workspace)
            }
            cli::document::DocumentCommand::Create(action) => {
                dispatch_document_create(context, &action, workspace)
            }
            cli::document::DocumentCommand::Update(action) => {
                dispatch_document_update(context, &action, workspace)
            }
            cli::document::DocumentCommand::Delete(action) => {
                dispatch_document_delete(context, &action, workspace)
            }
            cli::document::DocumentCommand::Comment(action) => match action.command {
                cli::document::DocumentCommentCommand::Add(action) => {
                    dispatch_document_comment_add(context, &action, workspace)
                }
                cli::document::DocumentCommentCommand::List(action) => {
                    dispatch_document_comment_list(context, &action, workspace)
                }
            },
        },
        RootCommand::Config(_) => dispatch_config_generate(context, workspace),
        RootCommand::Schema(args) => commands::schema::run(ctx, &args),
        RootCommand::Api(args) => commands::api::run(ctx, &args),
        RootCommand::Completions(_) | RootCommand::Markdown(_) => {
            unreachable!("handled before configuration loads")
        }
    }
}

fn missing_team_key() -> Error {
    Error::new("Could not determine team key from directory name")
        .with_hint("Please specify a team key, name, or ID as an argument.")
}

fn dispatch_project_view(
    context: &Ctx,
    action: &cli::project::ProjectView,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::platform::selector;
    use crate::refs::is_linear_uuid;

    let json = action.json;
    let web = action.web;
    let app = action.app;
    let pager_enabled = !action.no_pager;
    let explicit = action.project_id.as_deref();
    let cli_workspace = workspace;
    let original = if let Some(reference) = explicit {
        reference.to_owned()
    } else {
        if json {
            return Err(Error::new("A project is required with --json")
                .with_hint("Pass a project UUID, slug ID, or exact name, or drop --json to pick one from a list.")
                .context(project_view::CONTEXT));
        }
        let interactive = {
            let config = context.config();
            selector::interactive_allowed(
                context.stdin_tty(),
                context.stdout_tty(),
                config.ci.as_deref(),
            )
        };
        if !interactive {
            return Err(Error::new("No project specified")
                .with_hint("Pass a project UUID, slug ID, or exact name. Running `linear project view` with no argument picks from a list, but only on a terminal.")
                .context(project_view::CONTEXT));
        }
        let config = context.config();
        let team_key = configured_team_key(&config.options);
        let transport = client::prepare_transport(
            &config.options,
            context.credentials()?,
            cli_workspace,
            &config.transport_env,
        )
        .context(project_view::CONTEXT)?;
        let projects =
            block_on_network(project_view::fetch_picker(&transport, team_key.as_deref()))
                .context(project_view::CONTEXT)?;
        let options = project_view::picker_options(&projects);
        let ci = context.config().ci.clone();
        let selection = selector::run(
            &options,
            &selector::PromptLabels {
                message: "Select a project",
                search_label: "Search projects",
                max_rows: 10,
            },
            ci.as_deref(),
            &mut context.stdout(),
        )
        .context(project_view::CONTEXT)?;
        match selection {
            selector::Selection::Selected(id) => id,
            selector::Selection::Interrupted => return Err(Error::reported()),
            selector::Selection::EndOfInput => {
                return Err(
                    Error::new("Project selection ended before a project was chosen")
                        .context(project_view::CONTEXT),
                );
            }
        }
    };

    // A UUID browser reference needs neither credential selection nor GraphQL.
    let (resolved_id, transport) =
        if explicit.is_some() && is_linear_uuid(&original) && (web || app) {
            (original.clone(), None)
        } else {
            let config = context.config();
            let credentials = context.credentials()?;
            let inputs = client::selection_inputs(&config.options, cli_workspace);
            let reference = if explicit.is_some() {
                Some(
                    prepare_project_lookup(
                        &original,
                        &WorkspaceScope::from_selection(&inputs, credentials),
                    )
                    .context(project_view::CONTEXT)?,
                )
            } else {
                None
            };
            let transport = client::prepare_transport_with_inputs(
                &config.options,
                credentials,
                &inputs,
                &config.transport_env,
            )
            .context(project_view::CONTEXT)?;
            let id = match reference {
                Some(reference) => block_on_network(resolve_project_with_transport(
                    &reference, &original, &transport,
                ))
                .context(project_view::CONTEXT)?,
                None => original.clone(),
            };
            (id, Some(transport))
        };

    if web || app {
        let workspace = context
            .config()
            .options
            .workspace()
            .map(|value| value.value().clone())
            .filter(|value| !value.is_empty());
        let Some(workspace) = workspace else {
            context.eprint(
                b"workspace is not set via command line, configuration file, or environment.\n",
            )?;
            return Err(Error::reported());
        };
        let url = format!("https://linear.app/{workspace}/project/{resolved_id}");
        let destination = if app { "Linear.app" } else { "web browser" };
        context.print(format!("Opening {url} in {destination}\n").as_bytes())?;
        crate::platform::opener::open(&url, app).context(project_view::CONTEXT)?;
        return Ok(());
    }

    let transport = transport.ok_or_else(|| Error::new("project transport missing"))?;
    let show_spinner = spinner::enabled(json, context.stdout_tty(), true);
    if show_spinner {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let result = if show_spinner {
        block_on_network(async {
            let pending = project_view::fetch_details(&transport, &resolved_id, &original);
            tokio::pin!(pending);
            let mut ticks = tokio::time::interval(spinner::TICK_INTERVAL);
            ticks.tick().await;
            let mut frame = 1;
            loop {
                tokio::select! {
                    biased;
                    result = &mut pending => break result,
                    _ = ticks.tick() => {
                        context.print(spinner::frame(frame).as_bytes())?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        })
    } else {
        block_on_network(project_view::fetch_details(
            &transport,
            &resolved_id,
            &original,
        ))
    };
    if show_spinner {
        context.print(spinner::CLEAR)?;
    }
    let project = result.context(project_view::CONTEXT)?;
    if json {
        let bytes = project_view::json(&project).context(project_view::CONTEXT)?;
        context.print(&bytes)?;
        return Ok(());
    }
    let markdown = project_view::markdown(&project, chrono::Utc::now(), &chrono::Local)
        .context(project_view::CONTEXT)?;
    if !context.stdout_tty() {
        context.print(format!("{markdown}\n").as_bytes())?;
        return Ok(());
    }
    let rendered = context
        .render_markdown(&markdown)
        .context(project_view::CONTEXT)?;
    context
        .page(&rendered, pager_enabled)
        .context(project_view::CONTEXT)?;
    Ok(())
}

fn dispatch_milestone_view(
    context: &Ctx,
    action: &cli::milestone::MilestoneView,
    workspace: Option<&str>,
) -> Result<()> {
    let json = action.json;
    let all = action.all;
    let original = action.milestone.clone();
    let project = action.project.clone();
    let show_spinner = spinner::enabled(json, context.stdout_tty(), true);
    if show_spinner {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let prepared = (|| {
        let config = context.config();
        let credentials = context.credentials()?;

        let inputs = client::selection_inputs(&config.options, workspace);
        let reference = project
            .as_deref()
            .map(|value| {
                prepare_project_lookup(value, &WorkspaceScope::from_selection(&inputs, credentials))
            })
            .transpose()?;
        // A UUID project needs no network resolution, so the milestone URL
        // diagnostic takes precedence over credential selection in that case.
        if reference.is_none() || project.as_deref().is_some_and(crate::refs::is_linear_uuid) {
            crate::refs::reject_linear_url(&original, "a milestone name or UUID")?;
        }
        let transport = client::prepare_transport_with_inputs(
            &config.options,
            credentials,
            &inputs,
            &config.transport_env,
        )?;
        Ok::<_, Error>((reference, transport))
    })();
    let (reference, transport) = match prepared {
        Ok(value) => value,
        Err(error) => {
            if show_spinner {
                context.print(spinner::CLEAR)?;
            }
            return Err(error.context(milestone_view::CONTEXT));
        }
    };
    let fetch = async {
        let request_id = match (reference.as_ref(), project.as_deref()) {
            (Some(reference), Some(project)) => {
                let project_id =
                    resolve_project_with_transport(reference, project, &transport).await?;
                crate::refs::reject_linear_url(&original, "a milestone name or UUID")?;
                milestone_view::resolve_id(&transport, &original, &project_id).await?
            }
            (None, None) => original.clone(),
            _ => {
                return Err(Error::new("project reference mismatch"));
            }
        };
        milestone_view::fetch(&transport, &original, &request_id, all).await
    };
    let result = if show_spinner {
        block_on_network(async {
            tokio::pin!(fetch);
            let mut ticks = tokio::time::interval(spinner::TICK_INTERVAL);
            ticks.tick().await;
            let mut frame = 1;
            loop {
                tokio::select! {
                    biased;
                    result = &mut fetch => break result,
                    _ = ticks.tick() => {
                        context.print(spinner::frame(frame).as_bytes())?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        })
    } else {
        block_on_network(fetch)
    };
    if show_spinner {
        context.print(spinner::CLEAR)?;
    }
    let milestone = result.context(milestone_view::CONTEXT)?;
    let output = if json {
        milestone_view::json(&milestone).context(milestone_view::CONTEXT)?
    } else {
        let markdown =
            milestone_view::markdown(&milestone, all, chrono::Utc::now(), &chrono::Local);
        let rendered = if context.stdout_tty() {
            use std::num::NonZeroU16;
            let columns = u16::try_from(table::stdout_columns(true))
                .ok()
                .and_then(NonZeroU16::new)
                .unwrap_or(crate::platform::markdown_terminal::FALLBACK_COLUMNS);
            let options = crate::platform::markdown_terminal::RenderOptions::for_terminal(
                columns,
                context.color(),
                None,
                crate::platform::markdown_terminal::HostSource::System,
            );
            crate::platform::markdown_terminal::render(&markdown, &options)
                .context(milestone_view::CONTEXT)?
        } else {
            markdown
        };
        format!("{rendered}\n").into_bytes()
    };
    context.print(&output)?;
    Ok(())
}

fn dispatch_project_update_list(
    context: &Ctx,
    action: &cli::project_update::ProjectUpdateList,
    workspace: Option<&str>,
) -> Result<()> {
    let json = action.json;
    let first = project_update_list::graphql_int(action.limit)?;
    let original = action.project_id.clone();
    let show_spinner = spinner::enabled(json, context.stdout_tty(), true);
    if show_spinner {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let result = (|| {
        let config = context.config();
        let credentials = context.credentials()?;

        let inputs = client::selection_inputs(&config.options, workspace);
        let reference = prepare_project_lookup(
            &original,
            &WorkspaceScope::from_selection(&inputs, credentials),
        )?;
        let transport = client::prepare_transport_with_inputs(
            &config.options,
            credentials,
            &inputs,
            &config.transport_env,
        )?;
        let columns = table::stdout_columns(context.stdout_tty());
        let color = project_update_list::output_color(context.stdout_tty(), !context.color());
        block_on_network(async {
            let id = resolve_project_with_transport(&reference, &original, &transport).await?;
            project_update_list::run(&transport, &original, &id, first, json, columns, color).await
        })
    })();
    if show_spinner {
        context.print(spinner::CLEAR)?;
    }
    let output = result.map_err(|error| {
        if error.has_context() {
            error
        } else {
            error.context(project_update_list::CONTEXT)
        }
    })?;
    context.print(&output)?;
    Ok(())
}

fn dispatch_initiative_create(
    context: &Ctx,
    action: &cli::initiative::InitiativeCreate,
    workspace: Option<&str>,
) -> Result<()> {
    let mut options = initiative_create::Options {
        name: action.name.clone(),
        description: action.description.clone(),
        status: action.status.clone(),
        owner: action.owner.clone(),
        target_date: action.target_date.clone(),
        color: action.color.clone(),
        icon: action.icon.clone(),
        interactive: action.interactive,
    };
    let config = context.config();
    let credentials = context.credentials()?;
    let cli_workspace = workspace;
    let inputs = client::selection_inputs(&config.options, cli_workspace);
    let transport = client::prepare_transport_with_inputs(
        &config.options,
        credentials,
        &inputs,
        &config.transport_env,
    )
    .context(initiative_create::CREATE_CONTEXT)?;
    if initiative_create::should_prompt(&options, context.stdout_tty()) {
        context.print(b"\nCreate a new initiative\n\n")?;
        let mut session = crate::platform::prompt::PromptSession::stdio(context.stdout())?;
        let prompted = initiative_create::prompt(&mut options, &mut session);
        let result = match prompted {
            Ok(outcome) => {
                session.close()?;
                outcome
            }
            Err(error) => {
                return Err(match session.close() {
                    Ok(()) => error,
                    Err(mut cleanup) => {
                        cleanup.push_message(&format!("; prompt also failed: {}", error));
                        cleanup
                    }
                });
            }
        };
        match result {
            initiative_create::PromptResult::Complete => {}
            initiative_create::PromptResult::Interrupted => {
                return Err(Error::cancelled());
            }
            initiative_create::PromptResult::EndOfInput => {
                return Err(Error::new("unexpected EOF while prompting for initiative"));
            }
        }
    }
    let status = initiative_create::validate(&options)?;
    let owner_id = block_on_network(initiative_create::resolve_owner(
        &transport,
        options.owner.as_deref(),
    ))
    .context(initiative_create::CREATE_CONTEXT)?;
    let show_spinner = spinner::enabled(false, context.stdout_tty(), true);
    if show_spinner {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let result = block_on_network(initiative_create::submit_create(
        &transport, options, status, owner_id,
    ));
    if show_spinner {
        context.print(spinner::CLEAR)?;
    }
    let result = result.context(initiative_create::CREATE_CONTEXT)?;
    context.print(&result)?;
    Ok(())
}

fn dispatch_initiative_update_list(
    context: &Ctx,
    action: &cli::initiative_update::InitiativeUpdateList,
    workspace: Option<&str>,
) -> Result<()> {
    let json = action.json;
    let first = initiative_update_list::graphql_int(action.limit)?;
    let original = &action.initiative_id;
    let show_spinner = spinner::enabled(json, context.stdout_tty(), true);
    if show_spinner {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let result = (|| {
        let config = context.config();
        let credentials = context.credentials()?;

        let inputs = client::selection_inputs(&config.options, workspace);
        let reference = initiative_view::prepare_reference(
            original,
            &WorkspaceScope::from_selection(&inputs, credentials),
        )?;
        let transport = client::prepare_transport_with_inputs(
            &config.options,
            credentials,
            &inputs,
            &config.transport_env,
        )?;
        let columns = table::stdout_columns(context.stdout_tty());
        let color = context.stdout_tty() && context.color();
        block_on_network(async {
            let id = initiative_view::resolve_reference(&transport, &reference, original)
                .await
                .context(initiative_update_list::CONTEXT)?;
            initiative_update_list::run(&transport, original, &id, first, json, columns, color)
                .await
        })
    })();
    if show_spinner {
        context.print(spinner::CLEAR)?;
    }
    let output = result.map_err(|error| {
        if error.has_context() {
            error
        } else {
            error.context(initiative_update_list::CONTEXT)
        }
    })?;
    context.print(&output)?;
    Ok(())
}

fn dispatch_initiative_projects(
    context: &Ctx,
    initiative_arg: &str,
    project_arg: &str,
    sort_order: Option<f64>,
    force: bool,
    mode: initiative_projects::Mode,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::platform::prompt::{PromptOutcome, PromptSession};
    let config = context.config();
    let credentials = context.credentials()?;
    let inputs = client::selection_inputs(&config.options, workspace);
    let transport = client::prepare_transport_with_inputs(
        &config.options,
        credentials,
        &inputs,
        &config.transport_env,
    )?;
    let scope = WorkspaceScope::from_selection(&inputs, credentials);
    let initiative = block_on_network(initiative_projects::resolve_initiative(
        &transport,
        initiative_arg,
        &scope,
        mode,
    ))?;
    let project = block_on_network(initiative_projects::resolve_project(
        &transport,
        project_arg,
        &scope,
        mode,
    ))?;
    let link = match mode {
        initiative_projects::Mode::Add => None,
        initiative_projects::Mode::Remove => {
            let link = block_on_network(initiative_projects::find_link(
                &transport,
                &initiative,
                &project,
            ))?;
            if link.is_none() {
                context.print(
                    format!(
                        "Project \"{}\" is not linked to initiative \"{}\"\n",
                        project.name, initiative.name
                    )
                    .as_bytes(),
                )?;
                return Ok(());
            }
            if !force {
                if !context.stdin_tty() {
                    return Err(Error::new(
                        "Interactive confirmation required. Use --force to skip.",
                    ));
                }
                let outcome = {
                    let mut session = PromptSession::confirmation_stdio(context.stdout())?;
                    let result = session.confirm(
                        &format!(
                            "Remove \"{}\" from initiative \"{}\"?",
                            project.name, initiative.name
                        ),
                        true,
                    );
                    session.finish_result(result)?
                };
                match outcome {
                    PromptOutcome::Submitted(true) => {}
                    PromptOutcome::Submitted(false) => {
                        context.print(b"Removal cancelled.\n")?;
                        return Ok(());
                    }
                    PromptOutcome::Interrupted => {
                        return Err(Error::cancelled());
                    }
                    PromptOutcome::EndOfInput => {
                        return Err(Error::new(
                            "unexpected EOF while prompting for confirmation",
                        ));
                    }
                }
            }
            link
        }
    };
    let show_spinner = spinner::enabled(false, context.stdout_tty(), true);
    if show_spinner {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let result = match mode {
        initiative_projects::Mode::Add => block_on_network(initiative_projects::add(
            &transport,
            &initiative,
            &project,
            sort_order,
        )),
        initiative_projects::Mode::Remove => {
            let link_id = link.ok_or_else(|| Error::new("confirmed removal requires a link ID"))?;
            block_on_network(initiative_projects::remove(
                &transport,
                &link_id,
                &initiative,
                &project,
            ))
        }
    };
    if show_spinner {
        context.print(spinner::CLEAR)?;
    }
    context.print(&result?)?;
    Ok(())
}

fn dispatch_initiative_unarchive(
    context: &Ctx,
    action: &cli::initiative::InitiativeUnarchive,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::platform::prompt::{PromptOutcome, PromptSession};
    let original = &action.initiative_id;
    let config = context.config();
    let credentials = context.credentials()?;
    let inputs = client::selection_inputs(&config.options, workspace);
    let transport = client::prepare_transport_with_inputs(
        &config.options,
        credentials,
        &inputs,
        &config.transport_env,
    )
    .context(initiative_view::RESOLVE_CONTEXT)?;
    let reference = initiative_view::prepare_reference(
        original,
        &WorkspaceScope::from_selection(&inputs, credentials),
    )
    .context(initiative_view::RESOLVE_CONTEXT)?;
    let id = block_on_network(initiative_unarchive::resolve_reference(
        &transport, &reference, original,
    ))?;
    let detail = block_on_network(initiative_unarchive::fetch_details(
        &transport, &id, original,
    ))?;
    if let Some(output) = initiative_unarchive::active_output(&detail) {
        context.print(&output)?;
        return Ok(());
    }
    if !action.force {
        if !context.stdin_tty() {
            return Err(Error::new(
                "Interactive confirmation required. Use --force to skip.",
            ));
        }
        let outcome = {
            let mut session = PromptSession::confirmation_stdio(context.stdout())?;
            let result = session.confirm(
                &format!("Are you sure you want to unarchive \"{}\"?", detail.name),
                true,
            );
            session.finish_result(result)?
        };
        match outcome {
            PromptOutcome::Submitted(true) => {}
            PromptOutcome::Submitted(false) => {
                context.print(b"Unarchive cancelled.\n")?;
                return Ok(());
            }
            PromptOutcome::Interrupted => {
                return Err(Error::cancelled());
            }
            PromptOutcome::EndOfInput => {
                return Err(Error::new(
                    "unexpected EOF while prompting for confirmation",
                ));
            }
        }
    }
    let show_spinner = spinner::enabled(false, context.stdout_tty(), true);
    if show_spinner {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let result = block_on_network(initiative_unarchive::submit(&transport, &id));
    if show_spinner {
        context.print(spinner::CLEAR)?;
    }
    let output = result?;
    context.print(&output)?;
    Ok(())
}

fn dispatch_initiative_view(
    context: &Ctx,
    action: &cli::initiative::InitiativeView,
    workspace: Option<&str>,
) -> Result<()> {
    let original = &action.initiative_id;
    let app = action.app;
    let web = action.web;
    let json = action.json;
    let config = context.config();
    let credentials = context.credentials()?;
    let cli_workspace = workspace;
    let inputs = client::selection_inputs(&config.options, cli_workspace);
    let transport = client::prepare_transport_with_inputs(
        &config.options,
        credentials,
        &inputs,
        &config.transport_env,
    )
    .context(initiative_view::RESOLVE_CONTEXT)?;
    let scope = WorkspaceScope::from_selection(&inputs, credentials);
    let reference = initiative_view::prepare_reference(original, &scope)
        .context(initiative_view::RESOLVE_CONTEXT)?;
    let id = block_on_network(initiative_view::resolve_reference(
        &transport, &reference, original,
    ))?;
    let show_spinner = !(app || web) && spinner::enabled(json, context.stdout_tty(), true);
    if show_spinner {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let result = block_on_network(initiative_view::fetch_details(&transport, id, original));
    if show_spinner {
        context.print(spinner::CLEAR)?;
    }
    let detail = result?;
    if app || web {
        if detail.url.is_empty() {
            return Err(
                Error::not_found("Initiative", original).context(initiative_view::FETCH_CONTEXT)
            );
        }
        context.print(initiative_view::opening(&detail, app))?;
        crate::platform::opener::open(&detail.url, app).context(initiative_view::OPEN_CONTEXT)?;
        return Ok(());
    }
    let output = if json {
        initiative_view::render_json(&detail)
    } else {
        let columns = std::num::NonZeroU16::new(
            u16::try_from(table::stdout_columns(context.stdout_tty())).unwrap_or(80),
        )
        .unwrap_or(crate::platform::markdown_terminal::FALLBACK_COLUMNS);
        initiative_view::render_text(&detail, context.stdout_tty(), columns, context.color())
    }
    .context(initiative_view::FETCH_CONTEXT)?;
    context.print(&output)?;
    Ok(())
}

fn dispatch_initiative_list(
    context: &Ctx,
    action: &cli::initiative::InitiativeList,
    workspace: Option<&str>,
) -> Result<()> {
    let options = initiative_list::Options {
        status: action.status.clone(),
        all_statuses: action.all_statuses,
        owner: action.owner.clone(),
        web: action.web,
        app: action.app,
        json: action.json,
        archived: action.archived,
    };
    let cli_workspace = workspace;
    if options.web || options.app {
        let config = context.config();
        let workspace = match config
            .options
            .workspace()
            .map(|value| value.value().clone())
            .filter(|value| !value.is_empty())
        {
            Some(workspace) => workspace,
            None => {
                let credentials = context
                    .credentials()
                    .context(initiative_list::OPEN_CONTEXT)?;
                let inputs = client::selection_inputs(&config.options, cli_workspace);
                let transport = client::prepare_transport_with_inputs(
                    &config.options,
                    credentials,
                    &inputs,
                    &config.transport_env,
                )
                .context(initiative_list::OPEN_CONTEXT)?;
                block_on_network(initiative_list::viewer_workspace(&transport))
                    .context(initiative_list::OPEN_CONTEXT)?
            }
        };
        let (url, opening) = initiative_list::opening(&workspace, options.app);
        context.print(&opening)?;
        initiative_list::open(&url, options.app)?;
        return Ok(());
    }

    let show_spinner = spinner::enabled(options.json, context.stdout_tty(), true);
    if show_spinner {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let result = (|| {
        let status =
            initiative_list::status_filter(options.status.as_deref(), options.all_statuses)?;
        initiative_list::validate_owner(options.owner.as_deref())?;
        let config = context.config();
        let credentials = context.credentials()?;
        let transport = client::prepare_transport(
            &config.options,
            credentials,
            cli_workspace,
            &config.transport_env,
        )?;
        block_on_network(initiative_list::run(
            &transport,
            status.as_deref(),
            options.owner.as_deref(),
            options.archived,
            options.json,
            table::stdout_columns(context.stdout_tty()),
            context.stdout_tty() && context.color(),
        ))
    })();
    if show_spinner {
        context.print(spinner::CLEAR)?;
    }
    let output = result.context(initiative_list::FETCH_CONTEXT)?;
    context.print(&output)?;
    Ok(())
}

fn dispatch_project_comment_list(
    context: &Ctx,
    action: &cli::project::ProjectCommentList,
    workspace: Option<&str>,
) -> Result<()> {
    let json = action.json;
    let original = &action.project;
    let config = context.config();
    let credentials = context.credentials()?;

    let inputs = client::selection_inputs(&config.options, workspace);
    let reference = prepare_project_lookup(
        original,
        &WorkspaceScope::from_selection(&inputs, credentials),
    )
    .context(project_comment_list::CONTEXT)?;
    let transport = client::prepare_transport_with_inputs(
        &config.options,
        credentials,
        &inputs,
        &config.transport_env,
    )
    .context(project_comment_list::CONTEXT)?;
    let color = context.color();
    let output = block_on_network(async {
        let id = resolve_project_with_transport(&reference, original, &transport)
            .await
            .context(project_comment_list::CONTEXT)?;
        project_comment_list::run(&transport, original, &id, json, color).await
    })?;
    context.print(&output)?;
    Ok(())
}

fn dispatch_milestone_list(
    context: &Ctx,
    action: &cli::milestone::MilestoneList,
    workspace: Option<&str>,
) -> Result<()> {
    let json = action.json;
    let original = action.project.clone();
    // The spinner starts before config, credential and URL preparation and
    // stops before any error is reported.
    let show_spinner = spinner::enabled(json, context.stdout_tty(), true);
    if show_spinner {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let prepared = (|| {
        let config = context.config();
        let credentials = context.credentials()?;

        let inputs = client::selection_inputs(&config.options, workspace);
        let reference = prepare_project_lookup(
            &original,
            &WorkspaceScope::from_selection(&inputs, credentials),
        )?;
        let transport = client::prepare_transport_with_inputs(
            &config.options,
            credentials,
            &inputs,
            &config.transport_env,
        )?;
        Ok::<_, Error>((reference, transport))
    })();
    let (reference, transport) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            if show_spinner {
                context.print(spinner::CLEAR)?;
            }
            return Err(error.context(milestone_list::CONTEXT));
        }
    };
    let columns = table::stdout_columns(context.stdout_tty());
    let color = context.stdout_tty() && context.color();
    // Resolver errors gain the context here; `milestone_list::run` already
    // applies it to every page, cursor and rendering error.
    let fetch = async {
        let project_id = resolve_project_with_transport(&reference, &original, &transport)
            .await
            .context(milestone_list::CONTEXT)?;
        milestone_list::run(&transport, &original, &project_id, json, columns, color).await
    };
    let output_result = if show_spinner {
        block_on_network(async {
            tokio::pin!(fetch);
            let mut ticks = tokio::time::interval(spinner::TICK_INTERVAL);
            ticks.tick().await;
            let mut frame = 1;
            loop {
                tokio::select! {
                    biased;
                    result = &mut fetch => break result,
                    _ = ticks.tick() => {
                        context.print(
                            spinner::frame(frame).as_bytes())?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        })
    } else {
        block_on_network(fetch)
    };
    if show_spinner {
        context.print(spinner::CLEAR)?;
    }
    let output = output_result?;
    context.print(&output)?;
    Ok(())
}

fn dispatch_milestone_create(
    context: &Ctx,
    action: &cli::milestone::MilestoneCreate,
    workspace: Option<&str>,
) -> Result<()> {
    let original = action.project.clone();
    let options = milestone_create::Options {
        name: action.name.clone(),
        description: action.description.clone(),
        target_date: action.target_date.clone(),
    };
    // The spinner starts before config, credential and URL preparation and
    // stops before any error is reported.
    let show_spinner = spinner::enabled(false, context.stdout_tty(), true);
    if show_spinner {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let prepared = (|| {
        let config = context.config();
        let credentials = context.credentials()?;

        let inputs = client::selection_inputs(&config.options, workspace);
        let reference = prepare_project_lookup(
            &original,
            &WorkspaceScope::from_selection(&inputs, credentials),
        )?;
        let transport = client::prepare_transport_with_inputs(
            &config.options,
            credentials,
            &inputs,
            &config.transport_env,
        )?;
        Ok::<_, Error>((reference, transport))
    })();
    let (reference, transport) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            if show_spinner {
                context.print(spinner::CLEAR)?;
            }
            return Err(error.context(milestone_create::CONTEXT));
        }
    };
    let create = async {
        let project_id = resolve_project_with_transport(&reference, &original, &transport).await?;
        milestone_create::submit(&transport, &project_id, &options).await
    };
    let result = if show_spinner {
        block_on_network(async {
            tokio::pin!(create);
            let mut ticks = tokio::time::interval(spinner::TICK_INTERVAL);
            ticks.tick().await;
            let mut frame = 1;
            loop {
                tokio::select! {
                    biased;
                    result = &mut create => break result,
                    _ = ticks.tick() => {
                        context.print(
                            spinner::frame(frame).as_bytes())?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        })
    } else {
        block_on_network(create)
    };
    if show_spinner {
        context.print(spinner::CLEAR)?;
    }
    let output = result.context(milestone_create::CONTEXT)?;
    context.print(&output)?;
    Ok(())
}

fn dispatch_milestone_update(
    context: &Ctx,
    action: &cli::milestone::MilestoneUpdate,
    workspace: Option<&str>,
) -> Result<()> {
    let id = &action.id;
    crate::refs::reject_linear_url(id, "a milestone UUID").context(milestone_update::CONTEXT)?;
    let sort_order = action.sort_order;
    let mut options = milestone_update::Options {
        name: action.name.clone(),
        description: action.description.clone(),
        target_date: action.target_date.clone(),
        sort_order,
        project_id: action.project.clone(),
    };
    // Checked before the spinner, config or client.
    options.require_update()?;
    let show_spinner = spinner::enabled(false, context.stdout_tty(), true);
    if show_spinner {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let prepared = (|| {
        let config = context.config();
        let credentials = context.credentials()?;

        let inputs = client::selection_inputs(&config.options, workspace);
        // The client is built first so missing credentials are reported before a bad project.
        let transport = client::prepare_transport_with_inputs(
            &config.options,
            credentials,
            &inputs,
            &config.transport_env,
        )?;
        let reference = options
            .project_id
            .as_ref()
            .filter(|value| !value.is_empty())
            .map(|original| {
                prepare_project_lookup(
                    original,
                    &WorkspaceScope::from_selection(&inputs, credentials),
                )
            })
            .transpose()?;
        Ok::<_, Error>((reference, transport))
    })();
    let (reference, transport) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            if show_spinner {
                context.print(spinner::CLEAR)?;
            }
            return Err(error.context(milestone_update::CONTEXT));
        }
    };
    let update = async {
        if let Some(reference) = reference {
            let original = options
                .project_id
                .as_ref()
                .ok_or_else(|| Error::new("prepared project has no original reference"))?;
            options.project_id =
                Some(resolve_project_with_transport(&reference, original, &transport).await?);
        }
        milestone_update::submit(&transport, id, &options).await
    };
    let result = if show_spinner {
        block_on_network(async {
            tokio::pin!(update);
            let mut ticks = tokio::time::interval(spinner::TICK_INTERVAL);
            ticks.tick().await;
            let mut frame = 1;
            loop {
                tokio::select! {
                    biased;
                    result = &mut update => break result,
                    _ = ticks.tick() => {
                        context.print(
                            spinner::frame(frame).as_bytes())?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        })
    } else {
        block_on_network(update)
    };
    if show_spinner {
        context.print(spinner::CLEAR)?;
    }
    let output = result.context(milestone_update::CONTEXT)?;
    context.print(&output)?;
    Ok(())
}

/// None means the user confirmed; every other result is a completed command.
/// Whether a deletion should go ahead: `--force`, or a yes at the prompt.
fn confirm_deletion(context: &Ctx, force: bool, message: &str) -> Result<bool> {
    if force || context.confirm(message, "--force")? {
        return Ok(true);
    }
    context.print("Deletion canceled\n")?;
    Ok(false)
}

fn dispatch_label_delete(
    context: &Ctx,
    action: &cli::label::LabelDelete,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::platform::prompt::{PromptOutcome, PromptSession};
    let result = (|| {
        let config = context.config();
        let credentials = context.credentials()?;
        let inputs = client::selection_inputs(&config.options, workspace);
        // The client is built first so missing credentials are reported before a bad team.
        let transport = client::prepare_transport_with_inputs(
            &config.options,
            credentials,
            &inputs,
            &config.transport_env,
        )?;
        let team = match action.team.as_deref() {
            Some(reference) => {
                let prepared = prepare_team_lookup(
                    reference,
                    &WorkspaceScope::from_selection(&inputs, credentials),
                )?;
                Some(block_on_network(resolve_team_with_transport(&prepared, &transport))?.key)
            }
            None => configured_team_key(&config.options),
        };
        let labels = label_delete::scoped(
            block_on_network(label_delete::lookup(&transport, &action.name_or_id))?,
            team.as_deref(),
        );
        let label = match labels.as_slice() {
            [] => return Err(label_delete::missing(&action.name_or_id, team.as_deref())),
            [label] => label.clone(),
            _ => {
                if !context.stdin_tty() {
                    return Err(Error::new(format!(
                        "Multiple labels named \"{}\" found",
                        action.name_or_id
                    ))
                    .with_hint("Use --team to disambiguate."));
                }
                let outcome = {
                    let mut session = PromptSession::stdin_stdio(context.stdout())?;
                    let result = label_delete::choose(&mut session, &action.name_or_id, &labels);
                    session.finish_result(result)?
                };
                match outcome {
                    PromptOutcome::Submitted(label) => label,
                    PromptOutcome::Interrupted => {
                        return Err(Error::cancelled());
                    }
                    PromptOutcome::EndOfInput => {
                        return Err(Error::new("unexpected EOF while selecting a label"));
                    }
                }
            }
        };
        if !confirm_deletion(
            context,
            action.force,
            &format!(
                "Are you sure you want to delete label \"{}\"?",
                label_delete::display(&label)
            ),
        )? {
            return Ok(());
        }
        let show_spinner = spinner::enabled(false, context.stdout_tty(), true);
        if show_spinner {
            context.print(spinner::frame(0).as_bytes())?;
        }
        let result = block_on_network(async {
            let pending = label_delete::submit(&transport, &label);
            tokio::pin!(pending);
            if !show_spinner {
                return pending.await;
            }
            let mut ticks = tokio::time::interval(spinner::TICK_INTERVAL);
            ticks.tick().await;
            let mut frame = 1;
            loop {
                tokio::select! {
                    biased;
                    result = &mut pending => break result,
                    _ = ticks.tick() => {
                        context.print(spinner::frame(frame).as_bytes())?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        });
        if show_spinner {
            context.print(spinner::CLEAR)?;
        }
        context.print(&result?)?;
        Ok(())
    })();
    result.context(label_delete::CONTEXT)
}

fn dispatch_project_delete(
    context: &Ctx,
    action: &cli::project::ProjectDelete,
    workspace: Option<&str>,
) -> Result<()> {
    let original = &action.project_id;
    if !confirm_deletion(
        context,
        action.force,
        &format!("Are you sure you want to delete project {original}?"),
    )? {
        return Ok(());
    }
    let show_spinner = spinner::enabled(false, context.stdout_tty(), true);
    if show_spinner {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let result = (|| {
        let config = context.config();
        let credentials = context.credentials()?;
        let inputs = client::selection_inputs(&config.options, workspace);
        // The client is built first so missing credentials are reported before a bad URL.
        let transport = client::prepare_transport_with_inputs(
            &config.options,
            credentials,
            &inputs,
            &config.transport_env,
        )?;
        let reference = prepare_project_lookup(
            original,
            &WorkspaceScope::from_selection(&inputs, credentials),
        )?;
        block_on_network(async {
            let id = resolve_project_with_transport(&reference, original, &transport).await?;
            project_delete::submit(&transport, original, &id).await
        })
    })();
    if show_spinner {
        context.print(spinner::CLEAR)?;
    }
    let output = result.context(project_delete::CONTEXT)?;
    context.print(&output)?;
    Ok(())
}

fn dispatch_milestone_delete(
    context: &Ctx,
    action: &cli::milestone::MilestoneDelete,
    workspace: Option<&str>,
) -> Result<()> {
    let id = &action.id;
    crate::refs::reject_linear_url(id, "a milestone UUID").context(milestone_delete::CONTEXT)?;
    if !confirm_deletion(
        context,
        action.force,
        &format!("Are you sure you want to delete milestone {id}?"),
    )? {
        return Ok(());
    }
    let show_spinner = spinner::enabled(false, context.stdout_tty(), true);
    if show_spinner {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let result = (|| {
        let config = context.config();
        let credentials = context.credentials()?;
        let inputs = client::selection_inputs(&config.options, workspace);
        let transport = client::prepare_transport_with_inputs(
            &config.options,
            credentials,
            &inputs,
            &config.transport_env,
        )?;
        block_on_network(milestone_delete::submit(&transport, id))
    })();
    if show_spinner {
        context.print(spinner::CLEAR)?;
    }
    let output = result.context(milestone_delete::CONTEXT)?;
    context.print(&output)?;
    Ok(())
}

/// These leaves have distinct unresolved diagnostics, while sharing the
/// maintained reference and real VCS inference implementation.
fn resolve_relation_reference(
    context: &Ctx,
    input: Option<&str>,
    workspace: Option<&str>,
    unresolved: impl FnOnce() -> Error,
) -> Result<String, Error> {
    let reference = match input {
        None => crate::refs::IssueReference::Inferred,
        Some(_) => {
            let config = context.config();
            let credentials = context.credentials()?;
            let inputs = client::selection_inputs(&config.options, workspace);
            let team = configured_team_key(&config.options);
            crate::refs::prepare_issue_reference(
                input,
                team.as_deref(),
                &WorkspaceScope::from_selection(&inputs, credentials),
            )?
        }
    };
    let identifier = match reference {
        crate::refs::IssueReference::Identifier(id) => Some(id),
        crate::refs::IssueReference::Unresolved => None,
        crate::refs::IssueReference::Inferred => inferred_issue(context)?,
    };
    identifier.ok_or_else(unresolved)
}

fn relation_transport(
    context: &Ctx,
    workspace: Option<&str>,
) -> Result<crate::graphql::transport::GraphQlTransport, Error> {
    let config = context.config();
    client::prepare_transport(
        &config.options,
        context.credentials()?,
        workspace,
        &config.transport_env,
    )
}

/// Relation commands use a spinner; URL links do not. Clear it on every network
/// result before displaying the result or returning its contextual error.
fn relation_network(
    context: &Ctx,
    pending: impl std::future::Future<Output = Result<Vec<u8>, Error>>,
) -> Result<Vec<u8>, Error> {
    let enabled = spinner::enabled(false, context.stdout_tty(), true);
    if !enabled {
        return block_on_network(pending);
    }
    context.print(spinner::frame(0).as_bytes())?;
    let result = block_on_network(async {
        tokio::pin!(pending);
        let mut ticks = tokio::time::interval(spinner::TICK_INTERVAL);
        ticks.tick().await;
        let mut frame = 1_usize;
        loop {
            tokio::select! {
                biased;
                result = &mut pending => break result,
                _ = ticks.tick() => {
                    context.print(spinner::frame(frame).as_bytes())?;
                    frame = frame.wrapping_add(1);
                }
            }
        }
    });
    context.print(spinner::CLEAR)?;
    result
}

fn dispatch_issue_relation_list(
    context: &Ctx,
    action: &cli::issue::IssueRelationList,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::commands::issue_relations;
    let output = (|| {
        let identifier =
            resolve_relation_reference(context, action.issue_id.as_deref(), workspace, || {
                issue_details::unresolved(false)
            })?;
        let transport = relation_transport(context, workspace)?;
        relation_network(context, issue_relations::list(&transport, &identifier))
    })()
    .context(issue_relations::LIST_CONTEXT)?;
    context.print(&output)?;
    Ok(())
}

fn prepare_relation_pair(
    context: &Ctx,
    a: &str,
    b: &str,
    workspace: Option<&str>,
) -> Result<(String, String), Error> {
    let resolve = |input| {
        resolve_relation_reference(context, Some(input), workspace, || {
            Error::new(format!("Could not resolve issue identifier: {input}"))
        })
    };
    // Validate both references before creating the transport or looking up A.
    let a = resolve(a)?;
    let b = resolve(b)?;
    Ok((a, b))
}
fn dispatch_issue_relation_add(
    context: &Ctx,
    action: &cli::issue::IssueRelationAdd,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::commands::issue_relations;
    let output = (|| {
        let (a, b) = prepare_relation_pair(
            context,
            &action.issue_id,
            &action.related_issue_id,
            workspace,
        )?;
        let transport = relation_transport(context, workspace)?;
        relation_network(
            context,
            issue_relations::add(&transport, action.relation_type, &a, &b),
        )
    })()
    .context(issue_relations::ADD_CONTEXT)?;
    context.print(&output)?;
    Ok(())
}
fn dispatch_issue_relation_delete(
    context: &Ctx,
    action: &cli::issue::IssueRelationDelete,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::commands::issue_relations;
    let output = (|| {
        let (a, b) = prepare_relation_pair(
            context,
            &action.issue_id,
            &action.related_issue_id,
            workspace,
        )?;
        let transport = relation_transport(context, workspace)?;
        relation_network(
            context,
            issue_relations::delete(&transport, action.relation_type, &a, &b),
        )
    })()
    .context(issue_relations::DELETE_CONTEXT)?;
    context.print(&output)?;
    Ok(())
}
fn dispatch_issue_link(
    context: &Ctx,
    action: &cli::issue::IssueLink,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::commands::issue_link;
    let output = (|| {
        let (input, url) = issue_link::inputs(&action.url_or_issue_id, action.url.as_deref())?;
        let identifier = resolve_relation_reference(context, input, workspace, ||
            Error::new("Could not determine issue ID").with_hint(
                "Please provide an issue ID like 'ENG-123', or run from a branch that contains an issue identifier."))?;
        let transport = relation_transport(context, workspace)?;
        block_on_network(issue_link::submit(&transport, &identifier, url, action.title.as_deref()))
    })().context(issue_link::CONTEXT)?;
    context.print(&output)?;
    Ok(())
}

fn dispatch_issue_comment_list(
    context: &Ctx,
    action: &cli::issue::IssueCommentList,
    workspace: Option<&str>,
) -> Result<()> {
    let id = resolve_issue(context, action.issue_id.as_deref(), workspace)
        .context(issue_comment_list::CONTEXT)?;
    let config = context.config();
    let transport = client::prepare_transport(
        &config.options,
        context.credentials()?,
        workspace,
        &config.transport_env,
    )
    .context(issue_comment_list::CONTEXT)?;
    let output = block_on_network(issue_comment_list::run(
        &transport,
        &id,
        &id,
        action.json,
        context.color(),
    ))?;
    context.print(&output)?;
    Ok(())
}

fn dispatch_issue_comment_delete(
    context: &Ctx,
    action: &cli::issue::IssueCommentDelete,
    workspace: Option<&str>,
) -> Result<()> {
    let id = &action.comment_id;
    // Both URL checks come before config and credentials.
    crate::refs::reject_comment_url(id)
        .and_then(|()| crate::refs::reject_linear_url(id, "a comment UUID"))
        .context(issue_comment_delete::CONTEXT)?;
    let transport = (|| {
        let config = context.config();
        let credentials = context.credentials()?;
        let inputs = client::selection_inputs(&config.options, workspace);
        client::prepare_transport_with_inputs(
            &config.options,
            credentials,
            &inputs,
            &config.transport_env,
        )
    })()
    .context(issue_comment_delete::CONTEXT)?;
    let output = block_on_network(issue_comment_delete::submit(&transport, id))
        .context(issue_comment_delete::CONTEXT)?;
    context.print(&output)?;
    Ok(())
}

/// Order: body flags, project reference (a UUID needs no client), the
/// omitted-body prompt, then client construction before parent validation.
fn dispatch_project_comment_add(
    context: &Ctx,
    action: &cli::project::ProjectCommentAdd,
    workspace: Option<&str>,
) -> Result<()> {
    let original = &action.project;
    let result = (|| {
        let body = comment_add::resolve_body(action.body.as_deref(), action.body_file.as_deref())?;
        let project_id = {
            let config = context.config();
            let credentials = context.credentials()?;
            let inputs = client::selection_inputs(&config.options, workspace);
            let reference = prepare_project_lookup(
                original,
                &WorkspaceScope::from_selection(&inputs, credentials),
            )?;
            match &reference {
                ProjectReference::Id(id) => id.clone(),
                ProjectReference::NameOrSlug(_) | ProjectReference::Slug(_) => {
                    let transport = client::prepare_transport_with_inputs(
                        &config.options,
                        credentials,
                        &inputs,
                        &config.transport_env,
                    )?;
                    block_on_network(resolve_project_with_transport(
                        &reference, original, &transport,
                    ))?
                }
            }
        };
        let body = match body {
            Some(body) => body,
            None => prompt_comment_body(context)?,
        };
        submit_comment(
            context,
            workspace,
            comment_add::CommentTarget::Project { project_id },
            body,
            action.parent.as_deref(),
        )
        .map(|comment| comment_add::output("project", original, &comment))
    })();
    finish_comment_add(context, result)
}

/// Order: body flags, initiative reference (a UUID needs no client),
/// the omitted-body prompt, then client construction before parent validation.
fn dispatch_initiative_comment_add(
    context: &Ctx,
    action: &cli::initiative::InitiativeCommentAdd,
    workspace: Option<&str>,
) -> Result<()> {
    let original = &action.initiative;
    let result = (|| {
        let body = comment_add::resolve_body(action.body.as_deref(), action.body_file.as_deref())?;
        let initiative_id = {
            let config = context.config();
            let credentials = context.credentials()?;
            let inputs = client::selection_inputs(&config.options, workspace);
            let reference = prepare_initiative_lookup(
                original,
                &WorkspaceScope::from_selection(&inputs, credentials),
            )?;
            match &reference {
                InitiativeReference::Id(id) => id.clone(),
                InitiativeReference::NameOrSlug(_) | InitiativeReference::UrlSlug(_) => {
                    let transport = client::prepare_transport_with_inputs(
                        &config.options,
                        credentials,
                        &inputs,
                        &config.transport_env,
                    )?;
                    block_on_network(resolve_initiative_with_transport(
                        &reference, original, &transport,
                    ))?
                }
            }
        };
        let body = match body {
            Some(body) => body,
            None => prompt_comment_body(context)?,
        };
        submit_comment(
            context,
            workspace,
            comment_add::CommentTarget::Initiative { initiative_id },
            body,
            action.parent.as_deref(),
        )
        .map(|comment| comment_add::output("initiative", original, &comment))
    })();
    finish_comment_add(context, result)
}

/// Order: local document URL reduction, body flags, then the content
/// record lookup for every reference (UUIDs included), the omitted-body
/// prompt, and a second client before parent validation.
fn dispatch_document_comment_add(
    context: &Ctx,
    action: &cli::document::DocumentCommentAdd,
    workspace: Option<&str>,
) -> Result<()> {
    let result = (|| {
        let document = {
            let config = context.config();
            let credentials = context.credentials()?;
            let inputs = client::selection_inputs(&config.options, workspace);
            resolve_document_reference(
                &action.document,
                &WorkspaceScope::from_selection(&inputs, credentials),
            )?
        };
        let body = comment_add::resolve_body(action.body.as_deref(), action.body_file.as_deref())?;
        let document_content_id = {
            let config = context.config();
            let credentials = context.credentials()?;
            let inputs = client::selection_inputs(&config.options, workspace);
            let transport = client::prepare_transport_with_inputs(
                &config.options,
                credentials,
                &inputs,
                &config.transport_env,
            )?;
            block_on_network(comment_add::document_content_id(&transport, &document))?
        };
        let body = match body {
            Some(body) => body,
            None => prompt_comment_body(context)?,
        };
        submit_comment(
            context,
            workspace,
            comment_add::CommentTarget::Document {
                document_content_id,
            },
            body,
            action.parent.as_deref(),
        )
        .map(|comment| comment_add::output("document", &document, &comment))
    })();
    finish_comment_add(context, result)
}

/// An omitted body is always prompted for. Terminal cleanup completes
/// before any outcome, and a blank answer fails after submission.
fn prompt_comment_body(context: &Ctx) -> Result<String> {
    use crate::platform::prompt::{PromptOutcome, PromptSession};
    let outcome = {
        let mut session = PromptSession::stdin_stdio(context.stdout())?;
        let result = comment_add::prompt_body(&mut session);
        session.finish_result(result)?
    };
    match outcome {
        PromptOutcome::Submitted(body) => comment_add::require_prompted(body),
        PromptOutcome::Interrupted => Err(Error::cancelled()),
        PromptOutcome::EndOfInput => Err(Error::new(
            "unexpected EOF while prompting for comment body",
        )),
    }
}

/// `createComment` constructs its client before building the input.
fn submit_comment(
    context: &Ctx,
    workspace: Option<&str>,
    target: comment_add::CommentTarget,
    body: String,
    parent: Option<&str>,
) -> Result<crate::graphql::operations::comment_create::CreatedComment, Error> {
    let config = context.config();
    let credentials = context.credentials()?;
    let inputs = client::selection_inputs(&config.options, workspace);
    let transport = client::prepare_transport_with_inputs(
        &config.options,
        credentials,
        &inputs,
        &config.transport_env,
    )?;
    let input = comment_add::build_input(target, body, parent, None)?;
    block_on_network(comment_add::create(&transport, input))
}

fn finish_comment_add(context: &Ctx, result: Result<Vec<u8>>) -> Result<()> {
    context.print(result.context(comment_add::CONTEXT)?)
}

fn dispatch_initiative_comment_list(
    context: &Ctx,
    action: &cli::initiative::InitiativeCommentList,
    workspace: Option<&str>,
) -> Result<()> {
    let json = action.json;
    let original = &action.initiative;
    let config = context.config();
    let credentials = context.credentials()?;

    let inputs = client::selection_inputs(&config.options, workspace);
    let reference = prepare_initiative_lookup(
        original,
        &WorkspaceScope::from_selection(&inputs, credentials),
    )
    .context(initiative_comment_list::CONTEXT)?;
    let transport = client::prepare_transport_with_inputs(
        &config.options,
        credentials,
        &inputs,
        &config.transport_env,
    )
    .context(initiative_comment_list::CONTEXT)?;
    let color = context.color();
    let output = block_on_network(async {
        let id = resolve_initiative_with_transport(&reference, original, &transport)
            .await
            .context(initiative_comment_list::CONTEXT)?;
        initiative_comment_list::run(&transport, original, &id, json, color).await
    })?;
    context.print(&output)?;
    Ok(())
}

fn dispatch_document_comment_list(
    context: &Ctx,
    action: &cli::document::DocumentCommentList,
    workspace: Option<&str>,
) -> Result<()> {
    let json = action.json;
    let original = &action.document;
    let config = context.config();
    let credentials = context.credentials()?;

    let inputs = client::selection_inputs(&config.options, workspace);
    let reference = resolve_document_reference(
        original,
        &WorkspaceScope::from_selection(&inputs, credentials),
    )
    .context(document_comment_list::CONTEXT)?;
    let transport = client::prepare_transport_with_inputs(
        &config.options,
        credentials,
        &inputs,
        &config.transport_env,
    )
    .context(document_comment_list::CONTEXT)?;
    let color = context.color();
    let output = block_on_network(async {
        document_comment_list::run(&transport, &reference, &reference, json, color).await
    })?;
    context.print(&output)?;
    Ok(())
}

fn dispatch_auth_token(context: &Ctx, workspace: Option<&str>) -> Result<()> {
    let output = auth_token::run(&context.config().options, context.credentials()?, workspace)?;
    context.print(&output)?;
    Ok(())
}

fn dispatch_auth_default(context: &Ctx, action: &cli::auth::AuthDefault) -> Result<()> {
    use crate::auth::write::RealCredentialFileWriter;
    use crate::commands::auth_default::DefaultAction;
    use crate::platform::prompt::{PlainSelect, PromptOutcome, PromptSession};
    use std::io::IsTerminal;

    let result = (|| {
        let prepared =
            auth_default::prepare(context.credentials()?, action.workspace_name.as_deref())?;
        let selected = match prepared {
            DefaultAction::Select(options) => {
                // stdin_stdio otherwise falls back to the line-script protocol.
                if !std::io::stdin().is_terminal() {
                    return Err(auth_default::non_tty_error());
                }
                let outcome = {
                    let mut session = PromptSession::stdin_stdio(context.stdout())?;
                    let prompted = session.select(&PlainSelect {
                        message: "Select default workspace",
                        options: &options,
                        default_index: 0,
                        default_hint: None,
                    });
                    session.finish_result(prompted)?
                };
                match outcome {
                    PromptOutcome::Submitted(workspace) => {
                        auth_default::prepare(context.credentials()?, Some(&workspace))?
                    }
                    PromptOutcome::Interrupted => {
                        return Err(Error::cancelled());
                    }
                    PromptOutcome::EndOfInput => {
                        return Err(Error::new(
                            "unexpected EOF while selecting a default workspace",
                        ));
                    }
                }
            }
            other => other,
        };
        let output = match selected {
            DefaultAction::Output(bytes) => bytes,
            DefaultAction::Save(workspace) => {
                let startup = legacy::Loaded::new(context)?;
                auth_default::save(
                    startup.credentials,
                    &workspace,
                    startup.credentials_path.as_deref(),
                    &RealCredentialFileWriter,
                )?
            }
            DefaultAction::Select(_) => {
                return Err(Error::new(
                    "submitted workspace did not resolve to a default action",
                ));
            }
        };
        context.print(&output)?;
        Ok(())
    })();
    result.context(auth_default::CONTEXT)
}

fn dispatch_auth_list(
    context: &Ctx,
    _action: &cli::auth::AuthList,
    _workspace: Option<&str>,
) -> Result<()> {
    let config = context.config();
    let rows = auth_list::classify(context.credentials()?);
    let output = if rows.is_empty() {
        auth_list::EMPTY_OUTPUT.as_bytes().to_vec()
    } else {
        let prepared = auth_list::prepare_transports(
            rows,
            config.options.endpoint().value(),
            &config.transport_env,
        )
        .context(auth_list::CONTEXT)?;
        let listed = block_on_network(auth_list::fetch(prepared)).context(auth_list::CONTEXT)?;
        auth_list::render(&listed, context.stdout_tty() && context.color())
    };
    context.print(&output)?;
    Ok(())
}

fn dispatch_auth_whoami(
    context: &Ctx,
    _action: &cli::auth::AuthWhoami,
    workspace: Option<&str>,
) -> Result<()> {
    let config = context.config();
    let credentials = context.credentials()?;

    let transport = auth_whoami::prepare_transport(
        &config.options,
        credentials,
        workspace,
        &config.transport_env,
    )?;
    let output = block_on_network(async move { auth_whoami::run(&transport).await })?;
    context.print(&output)?;
    Ok(())
}

fn dispatch_team_list(
    context: &Ctx,
    action: &cli::team::TeamList,
    workspace: Option<&str>,
) -> Result<()> {
    let flags = team_list::Options {
        json: action.json,
        web: action.web,
        app: action.app,
    };
    if flags.web || flags.app {
        let (url, opening) =
            team_list::web_opening(workspace, &context.config().options, flags.app)?;
        context.print(&opening)?;
        team_list::open(&url, flags.app)?;
        return Ok(());
    }
    let spinner = spinner::enabled(flags.json, context.stdout_tty(), true);
    if spinner {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let prepared = (|| {
        let config = context.config();
        let credentials = context.credentials()?;

        client::prepare_transport(
            &config.options,
            credentials,
            workspace,
            &config.transport_env,
        )
        .context("Failed to fetch teams")
    })();
    let transport = match prepared {
        Ok(transport) => transport,
        Err(error) => {
            if spinner {
                context.print(spinner::CLEAR)?;
            }
            return Err(error);
        }
    };
    let columns = table::stdout_columns(context.stdout_tty());
    let color = context.stdout_tty() && context.color();
    let output_result = if spinner {
        block_on_network(async {
            let pending = team_list::run(&transport, flags.json, columns, color);
            tokio::pin!(pending);
            let mut ticks = tokio::time::interval(spinner::TICK_INTERVAL);
            ticks.tick().await;
            let mut frame = 1;
            loop {
                tokio::select! {
                    biased;
                    result = &mut pending => break result,
                    _ = ticks.tick() => {
                        context.print(
                            spinner::frame(frame).as_bytes())?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        })
    } else {
        block_on_network(async { team_list::run(&transport, flags.json, columns, color).await })
    };
    if spinner {
        context.print(spinner::CLEAR)?;
    }
    let output = output_result.map_err(|error| {
        if !error.has_context() {
            error.context("Failed to fetch teams")
        } else {
            error
        }
    })?;
    context.print(&output)?;
    Ok(())
}

fn dispatch_label_create(
    context: &Ctx,
    action: &cli::label::LabelCreate,
    workspace: Option<&str>,
) -> Result<()> {
    // The transport is built before validating required fields.
    let transport = (|| {
        let config = context.config();
        client::prepare_transport(
            &config.options,
            context.credentials()?,
            workspace,
            &config.transport_env,
        )
    })()
    .context(label_create::CONTEXT)?;
    let mut options = label_create::Options {
        name: action.name.clone(),
        color: action.color.clone(),
        description: action.description.clone(),
        team: action.team.clone(),
        interactive: action.interactive,
    };
    if label_create::should_prompt(&options, context.stdout_tty()) {
        context.print(label_create::PROMPT_HEADER)?;
        let configured_key = configured_team_key(&context.config().options);
        let mut session = crate::platform::prompt::PromptSession::stdio(context.stdout())?;
        let prompted = (|| {
            let outcome = label_create::prompt_fields(&mut options, &mut session)?;
            match outcome {
                crate::platform::prompt::PromptOutcome::Submitted(()) => {}
                crate::platform::prompt::PromptOutcome::Interrupted => return Ok(outcome),
                crate::platform::prompt::PromptOutcome::EndOfInput => return Ok(outcome),
            }
            if options.team.is_none() {
                session.suspend()?;
                let teams =
                    block_on_network(crate::refs::fetch_all_teams_with_transport(&transport))
                        .context(label_create::CONTEXT)?;
                session.resume()?;
                label_create::prompt_team(
                    &mut options,
                    &mut session,
                    &teams,
                    configured_key.as_deref(),
                )
            } else {
                Ok(crate::platform::prompt::PromptOutcome::Submitted(()))
            }
        })();
        let outcome = session.finish_result(prompted)?;
        match outcome {
            crate::platform::prompt::PromptOutcome::Submitted(()) => {}
            crate::platform::prompt::PromptOutcome::Interrupted => {
                return Err(Error::cancelled());
            }
            crate::platform::prompt::PromptOutcome::EndOfInput => {
                return Err(Error::new("unexpected EOF while prompting for label"));
            }
        }
    }
    label_create::validate(&options).context(label_create::CONTEXT)?;
    let team_id = match options.team.as_deref().filter(|team| !team.is_empty()) {
        None => None,
        Some(team) => {
            let config = context.config();
            let inputs = client::selection_inputs(&config.options, workspace);
            let prepared = prepare_team_lookup(
                team,
                &WorkspaceScope::from_selection(&inputs, context.credentials()?),
            )
            .context(label_create::CONTEXT)?;
            Some(
                block_on_network(resolve_team_with_transport(&prepared, &transport))
                    .context(label_create::CONTEXT)?
                    .id,
            )
        }
    };
    let show_spinner = spinner::enabled(false, context.stdout_tty(), true);
    if show_spinner {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let create = label_create::submit(&transport, &options, team_id);
    let result = if show_spinner {
        block_on_network(async {
            tokio::pin!(create);
            let mut ticks = tokio::time::interval(spinner::TICK_INTERVAL);
            ticks.tick().await;
            let mut frame = 1;
            loop {
                tokio::select! {
                    biased;
                    result = &mut create => break result,
                    _ = ticks.tick() => {
                        context.print(spinner::frame(frame).as_bytes())?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        })
    } else {
        block_on_network(create)
    };
    if show_spinner {
        context.print(spinner::CLEAR)?;
    }
    let output = result.context(label_create::CONTEXT)?;
    context.print(&output)?;
    Ok(())
}

fn dispatch_team_create(
    context: &Ctx,
    action: &cli::team::TeamCreate,
    workspace: Option<&str>,
) -> Result<()> {
    let mut options = team_create::Options {
        name: action.name.clone(),
        description: action.description.clone(),
        key: action.key.clone(),
        private: action.private,
    };
    let interactive = team_create::interactive(action.no_interactive, context.stdout_tty());
    let mode = team_create::mode(&options, interactive);
    if mode == team_create::Mode::Prompt {
        context.print(team_create::PROMPT_HEADER)?;
        let mut session = crate::platform::prompt::PromptSession::stdio(context.stdout())?;
        let prompted = team_create::prompt(&mut options, &mut session);
        let result = match prompted {
            Ok(outcome) => {
                session.close()?;
                outcome
            }
            Err(error) => {
                return Err(match session.close() {
                    Ok(()) => error,
                    Err(mut cleanup) => {
                        cleanup.push_message(&format!("; prompt also failed: {}", error));
                        cleanup
                    }
                });
            }
        };
        match result {
            team_create::PromptResult::Complete => {}
            team_create::PromptResult::Interrupted => {
                return Err(Error::cancelled());
            }
            team_create::PromptResult::EndOfInput => {
                return Err(Error::new("unexpected EOF while prompting for team"));
            }
        }
    }
    let announcement = team_create::required_name(&options)
        .map(|name| team_create::announcement(name, mode))
        .context(team_create::CONTEXT)?;
    context.print(&announcement)?;
    // Only flag mode starts the spinner, after its progress line and before
    // the client is built; the catch path stops it before any error.
    let show_spinner = mode == team_create::Mode::Flags
        && interactive
        && spinner::enabled(false, context.stdout_tty(), true);
    if show_spinner {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let prepared = (|| {
        let config = context.config();
        let credentials = context.credentials()?;
        client::prepare_transport(
            &config.options,
            credentials,
            workspace,
            &config.transport_env,
        )
    })();
    let transport = match prepared {
        Ok(transport) => transport,
        Err(error) => {
            if show_spinner {
                context.print(spinner::CLEAR)?;
            }
            return Err(error.context(team_create::CONTEXT));
        }
    };
    let create = team_create::submit(&transport, &options);
    let result = if show_spinner {
        block_on_network(async {
            tokio::pin!(create);
            let mut ticks = tokio::time::interval(spinner::TICK_INTERVAL);
            ticks.tick().await;
            let mut frame = 1;
            loop {
                tokio::select! {
                    biased;
                    result = &mut create => break result,
                    _ = ticks.tick() => {
                        context.print(
                            spinner::frame(frame).as_bytes())?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        })
    } else {
        block_on_network(create)
    };
    if show_spinner {
        context.print(spinner::CLEAR)?;
    }
    let output = result.context(team_create::CONTEXT)?;
    context.print(&output)?;
    Ok(())
}

fn dispatch_team_id(
    context: &Ctx,
    _action: &cli::team::TeamId,
    _workspace: Option<&str>,
) -> Result<()> {
    let text = team_id::render(context)?;
    context.print(text.as_bytes())?;
    Ok(())
}

fn dispatch_team_members(
    context: &Ctx,
    action: &cli::team::TeamMembers,
    workspace: Option<&str>,
) -> Result<()> {
    let flags = team_members::Options {
        all: action.all,
        json: action.json,
    };
    let explicit = action.team.as_ref().filter(|value| !value.is_empty());

    let show_spinner = spinner::enabled(flags.json, context.stdout_tty(), true);
    let mut spinner_started = false;
    let fallback_key = if explicit.is_none() {
        Some(
            crate::commands::team_key::configured_team_key(&context.config().options)
                .ok_or_else(|| missing_team_key().context(team_members::CONTEXT))?,
        )
    } else {
        None
    };
    if show_spinner && fallback_key.is_some() {
        context.print(spinner::frame(0).as_bytes())?;
        spinner_started = true;
    }

    // A missing local key fails before credential selection. Explicit
    // references are locally prepared before transport, and resolved
    // before the member-query spinner begins.
    let selected = (|| {
        let config = context.config();
        if let Some(reference) = explicit {
            let credentials = context.credentials()?;
            let inputs = client::selection_inputs(&config.options, workspace);
            let scope = WorkspaceScope::from_selection(&inputs, credentials);
            let prepared = prepare_team_lookup(reference, &scope)?;
            let transport = client::prepare_transport_with_inputs(
                &config.options,
                credentials,
                &inputs,
                &config.transport_env,
            )?;
            let team = block_on_network(async {
                resolve_team_with_transport(&prepared, &transport).await
            })?;
            if team.key.is_empty() {
                return Err(missing_team_key());
            }
            Ok((team.key, transport))
        } else {
            let key = fallback_key.ok_or_else(|| Error::new("configured team key was lost"))?;
            Ok((
                key,
                client::prepare_transport(
                    &config.options,
                    context.credentials()?,
                    workspace,
                    &config.transport_env,
                )?,
            ))
        }
    })();
    let selected = selected.map_err(|error: Error| {
        if !error.has_context() {
            error.context(team_members::CONTEXT)
        } else {
            error
        }
    });
    if selected.is_err() && spinner_started {
        context.print(spinner::CLEAR)?;
    }
    let (team_key, transport) = selected?;
    if show_spinner && !spinner_started {
        context.print(spinner::frame(0).as_bytes())?;
        spinner_started = true;
    }
    let output_result = if spinner_started {
        block_on_network(async {
            let pending = team_members::run(&transport, &team_key, flags);
            tokio::pin!(pending);
            let mut ticks = tokio::time::interval(spinner::TICK_INTERVAL);
            ticks.tick().await;
            let mut frame = 1;
            loop {
                tokio::select! {
                    biased;
                    result = &mut pending => break result,
                    _ = ticks.tick() => {
                        context.print(
                            spinner::frame(frame).as_bytes())?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        })
    } else {
        block_on_network(team_members::run(&transport, &team_key, flags))
    };
    if spinner_started {
        context.print(spinner::CLEAR)?;
    }
    context.print(&output_result?)?;
    Ok(())
}

fn dispatch_team_states(
    context: &Ctx,
    action: &cli::team::TeamStates,
    workspace: Option<&str>,
) -> Result<()> {
    let json = action.json;
    let cli_workspace = workspace;
    let explicit = action.team.as_ref().filter(|value| !value.is_empty());
    let prepared = if let Some(reference) = explicit {
        let config = context.config();
        let inputs = client::selection_inputs(&config.options, cli_workspace);
        let scope = WorkspaceScope::from_selection(&inputs, context.credentials()?);
        Some(prepare_team_lookup(reference, &scope).context(team_states::CONTEXT)?)
    } else {
        None
    };
    let configured_key = if prepared.is_none() {
        Some(
            configured_team_key(&context.config().options).ok_or_else(|| {
                Error::new("Could not determine team key from directory name")
                    .with_hint("Please specify a team key, name, or ID as an argument.")
                    .context(team_states::CONTEXT)
            })?,
        )
    } else {
        None
    };
    let spinner = spinner::enabled(json, context.stdout_tty(), true);
    if spinner && prepared.is_none() {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let transport = match (|| {
        let config = context.config();
        client::prepare_transport(
            &config.options,
            context.credentials()?,
            cli_workspace,
            &config.transport_env,
        )
    })() {
        Ok(transport) => transport,
        Err(error) => {
            if spinner && prepared.is_none() {
                context.print(spinner::CLEAR)?;
            }
            return Err(error.context(team_states::CONTEXT));
        }
    };
    let team_key = match prepared {
        Some(prepared) => {
            block_on_network(async { resolve_team_with_transport(&prepared, &transport).await })
                .context(team_states::CONTEXT)?
                .key
        }
        None => configured_key.ok_or_else(|| Error::new("configured team key disappeared"))?,
    };
    if spinner && explicit.is_some() {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let color = context.stdout_tty() && context.color();
    let output_result = if spinner {
        block_on_network(async {
            let pending = team_states::run(&transport, team_key, json, color);
            tokio::pin!(pending);
            let mut ticks = tokio::time::interval(spinner::TICK_INTERVAL);
            ticks.tick().await;
            let mut frame = 1;
            loop {
                tokio::select! {
                    biased;
                    result = &mut pending => break result,
                    _ = ticks.tick() => {
                        context.print(
                            spinner::frame(frame).as_bytes())?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        })
    } else {
        block_on_network(async { team_states::run(&transport, team_key, json, color).await })
    };
    if spinner {
        context.print(spinner::CLEAR)?;
    }
    let output = output_result.context(team_states::CONTEXT)?;
    context.print(&output)?;
    Ok(())
}

fn dispatch_user_list(
    context: &Ctx,
    action: &cli::user::UserList,
    workspace: Option<&str>,
) -> Result<()> {
    let include_disabled = action.all;
    let json = action.json;
    let show_spinner = spinner::enabled(json, context.stdout_tty(), true);
    if show_spinner {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let prepared = (|| {
        let config = context.config();
        let credentials = context.credentials()?;

        client::prepare_transport(
            &config.options,
            credentials,
            workspace,
            &config.transport_env,
        )
        .context(user_list::CONTEXT)
    })();
    let transport = match prepared {
        Ok(transport) => transport,
        Err(error) => {
            if show_spinner {
                context.print(spinner::CLEAR)?;
            }
            return Err(error);
        }
    };
    let output_result = if show_spinner {
        block_on_network(async {
            let pending = user_list::run(&transport, include_disabled, json);
            tokio::pin!(pending);
            let mut ticks = tokio::time::interval(spinner::TICK_INTERVAL);
            ticks.tick().await;
            let mut frame = 1;
            loop {
                tokio::select! {
                    biased;
                    result = &mut pending => break result,
                    _ = ticks.tick() => {
                        context.print(
                            spinner::frame(frame).as_bytes())?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        })
    } else {
        block_on_network(async { user_list::run(&transport, include_disabled, json).await })
    };
    if show_spinner {
        context.print(spinner::CLEAR)?;
    }
    let output = output_result.map_err(|error| {
        if !error.has_context() {
            error.context(user_list::CONTEXT)
        } else {
            error
        }
    })?;
    context.print(&output)?;
    Ok(())
}

fn dispatch_project_list(
    context: &Ctx,
    action: &cli::project::ProjectList,
    workspace: Option<&str>,
) -> Result<()> {
    let options = project_list::Options {
        team: action.team.clone(),
        all_teams: action.all_teams,
        status: action.status.clone(),
        web: action.web,
        app: action.app,
        json: action.json,
    };
    let cli_workspace = workspace;
    if options.web || options.app {
        let config = context.config();
        let credentials = context.credentials()?;
        let configured_workspace = config
            .options
            .workspace()
            .map(|value| value.value().clone())
            .filter(|value| !value.is_empty());
        let needs_viewer = configured_workspace.is_none();
        let needs_team_lookup = !options.all_teams && options.team.is_some();
        let configured_team = if options.all_teams {
            None
        } else {
            configured_team_key(&config.options)
        };
        let (workspace, team_key) = if needs_viewer || needs_team_lookup {
            let inputs = client::selection_inputs(&config.options, cli_workspace);
            let transport = client::prepare_transport_with_inputs(
                &config.options,
                credentials,
                &inputs,
                &config.transport_env,
            )
            .context(project_list::OPEN_CONTEXT)?;
            block_on_network(async {
                let workspace = match configured_workspace {
                    Some(workspace) => workspace,
                    None => project_list::viewer_workspace(&transport).await?,
                };
                let team_key = match options.team.as_deref() {
                    Some(team) if needs_team_lookup => {
                        let prepared = prepare_team_lookup(
                            team,
                            &WorkspaceScope::from_selection(&inputs, credentials),
                        )?;
                        Some(
                            resolve_team_with_transport(&prepared, &transport)
                                .await?
                                .key,
                        )
                    }
                    Some(_) | None => configured_team,
                };
                Ok((workspace, team_key))
            })
            .context(project_list::OPEN_CONTEXT)?
        } else {
            let workspace = configured_workspace
                .ok_or_else(|| Error::new("project browser workspace was not resolved"))?;
            (workspace, configured_team)
        };
        let (url, line) = project_list::opening(&workspace, team_key.as_deref(), options.app);
        context.print(&line)?;
        project_list::open(&url, options.app)?;
        return Ok(());
    }

    let show_spinner = spinner::enabled(options.json, context.stdout_tty(), true);
    if show_spinner {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let prepared = (|| {
        project_list::check_conflicting_flags(&options)?;
        let config = context.config();
        let credentials = context.credentials()?;
        let inputs = client::selection_inputs(&config.options, cli_workspace);
        let team_lookup = if options.all_teams {
            None
        } else {
            options
                .team
                .as_deref()
                .map(|team| {
                    prepare_team_lookup(team, &WorkspaceScope::from_selection(&inputs, credentials))
                })
                .transpose()?
        };
        let transport = client::prepare_transport_with_inputs(
            &config.options,
            credentials,
            &inputs,
            &config.transport_env,
        )?;
        Ok::<_, Error>((team_lookup, transport))
    })();
    let (team_lookup, transport) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            if show_spinner {
                context.print(spinner::CLEAR)?;
            }
            return Err(error.context(project_list::FETCH_CONTEXT));
        }
    };
    let columns = crate::commands::table::stdout_columns(context.stdout_tty());
    let color = context.stdout_tty() && context.color();
    let configured_team = if options.all_teams {
        None
    } else {
        configured_team_key(&context.config().options)
    };
    let pending = async {
        let team_key = if options.all_teams {
            None
        } else if let Some(prepared) = team_lookup.as_ref() {
            Some(resolve_team_with_transport(prepared, &transport).await?.key)
        } else {
            configured_team
        };
        project_list::run(
            &transport,
            team_key.as_deref(),
            options.status.as_deref(),
            options.json,
            columns,
            color,
        )
        .await
    };
    let result = if show_spinner {
        block_on_network(async {
            tokio::pin!(pending);
            let mut ticks = tokio::time::interval(spinner::TICK_INTERVAL);
            ticks.tick().await;
            let mut frame = 1;
            loop {
                tokio::select! {
                    biased;
                    result = &mut pending => break result,
                    _ = ticks.tick() => {
                        context.print(spinner::frame(frame).as_bytes())?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        })
    } else {
        block_on_network(pending)
    };
    if show_spinner {
        context.print(spinner::CLEAR)?;
    }
    let output = result.map_err(|error| {
        if error.has_context() {
            error
        } else {
            error.context(project_list::FETCH_CONTEXT)
        }
    })?;
    context.print(&output)?;
    Ok(())
}

fn dispatch_cycle_list(
    context: &Ctx,
    action: &cli::cycle::CycleList,
    workspace: Option<&str>,
) -> Result<()> {
    let json = action.json;
    let team_reference = match action.team.clone() {
        Some(explicit) => explicit,
        None => configured_team_key(&context.config().options).ok_or_else(|| {
            Error::new("Could not determine team key from directory name or team flag")
                .context(cycle_list::CONTEXT)
        })?,
    };
    let selected = (|| {
        let config = context.config();
        let credentials = context.credentials()?;

        let inputs = client::selection_inputs(&config.options, workspace);
        let scope = WorkspaceScope::from_selection(&inputs, credentials);
        let prepared = prepare_team_lookup(&team_reference, &scope)?;
        let transport = client::prepare_transport_with_inputs(
            &config.options,
            credentials,
            &inputs,
            &config.transport_env,
        )?;
        Ok::<_, Error>((prepared, transport))
    })()
    .context(cycle_list::CONTEXT)?;
    let (prepared, transport) = selected;
    let team = block_on_network(async { resolve_team_with_transport(&prepared, &transport).await })
        .context(cycle_list::CONTEXT)?;

    // The spinner starts after the team lookup; a cycle-fetch error leaves
    // its last frame visible.
    let show_spinner = spinner::enabled(json, context.stdout_tty(), true);
    if show_spinner {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let columns = table::stdout_columns(context.stdout_tty());
    let color = context.color();
    let output = if show_spinner {
        block_on_network(async {
            let pending = cycle_list::run(&transport, &team.id, json, columns, color);
            tokio::pin!(pending);
            let mut ticks = tokio::time::interval(spinner::TICK_INTERVAL);
            ticks.tick().await;
            let mut frame = 1;
            loop {
                tokio::select! {
                    biased;
                    result = &mut pending => break result,
                    _ = ticks.tick() => {
                        context.print(
                            spinner::frame(frame).as_bytes())?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        })
    } else {
        block_on_network(async {
            cycle_list::run(&transport, &team.id, json, columns, color).await
        })
    }
    .map_err(|error| {
        if !error.has_context() {
            error.context(cycle_list::CONTEXT)
        } else {
            error
        }
    })?;
    if show_spinner {
        context.print(spinner::CLEAR)?;
    }
    context.print(&output)?;
    Ok(())
}

fn dispatch_cycle_view(
    context: &Ctx,
    action: &cli::cycle::CycleView,
    workspace: Option<&str>,
) -> Result<()> {
    let reference = action.cycle_ref.clone();
    let json = action.json;
    let explicit_team = action.team.clone();
    let selected = (|| {
        let config = context.config();
        let credentials = context.credentials()?;

        let inputs = client::selection_inputs(&config.options, workspace);
        let scope = WorkspaceScope::from_selection(&inputs, credentials);
        let url = crate::refs::expect_url_kind(
            &reference,
            crate::refs::LinearUrlKind::Cycle,
            "a cycle URL, number, or name",
            &scope,
        )?;
        let url_team = match &url {
            Some(crate::refs::LinearUrlRef::Cycle { team_key, .. }) => Some(team_key.clone()),
            Some(_) => {
                return Err(Error::new("expected cycle URL"));
            }
            None => None,
        };
        let team_reference = explicit_team
            .or(url_team)
            .or_else(|| configured_team_key(&config.options))
            .ok_or_else(|| {
                Error::new("Could not determine team key from directory name or team flag")
            })?;
        let prepared = prepare_team_lookup(&team_reference, &scope)?;
        let transport = client::prepare_transport_with_inputs(
            &config.options,
            credentials,
            &inputs,
            &config.transport_env,
        )?;
        Ok::<_, Error>((url, prepared, transport))
    })()
    .context(cycle_view::CONTEXT)?;
    let (url, prepared, transport) = selected;
    let team = block_on_network(async { resolve_team_with_transport(&prepared, &transport).await })
        .context(cycle_view::CONTEXT)?;
    let cycle_id = block_on_network(async {
        cycle_view::resolve_id(&transport, &team.id, &reference, url.as_ref()).await
    })
    .context(cycle_view::CONTEXT)?;
    let show_spinner = spinner::enabled(json, context.stdout_tty(), true);
    if show_spinner {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let request = cycle_view::detail_request(&cycle_id);
    let response = if show_spinner {
        block_on_network(async {
            let pending = transport.send_request(&request);
            tokio::pin!(pending);
            let mut ticks = tokio::time::interval(spinner::TICK_INTERVAL);
            ticks.tick().await;
            let mut frame = 1;
            loop {
                tokio::select! {
                    biased;
                    result = &mut pending => break result.map_err(Error::from),
                    _ = ticks.tick() => {
                        context.print(
                            spinner::frame(frame).as_bytes())?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        })
    } else {
        block_on_network(async { transport.send_request(&request).await.map_err(Error::from) })
    }
    .context(cycle_view::CONTEXT)?;
    let details: Result<crate::graphql::operations::cycle_view::GetCycleDetails, _> =
        crate::graphql::transport::classify_typed(response);
    if show_spinner
        && (details.is_ok()
            || matches!(
                &details,
                Err(crate::graphql::transport::TransportFailure::Response(
                    crate::graphql::envelope::ResponseError::UnexpectedShape(_)
                ))
            ))
    {
        context.print(spinner::CLEAR)?;
    }
    let details = details.map_err(|error| Error::from(error).context(cycle_view::CONTEXT))?;
    let cycle = details
        .cycle
        .ok_or_else(|| Error::not_found("Cycle", &reference).context(cycle_view::CONTEXT))?;
    let output = if json {
        cycle_view::json(&cycle).context(cycle_view::CONTEXT)?
    } else {
        let markdown = cycle_view::markdown(&cycle, chrono::Utc::now(), &chrono::Local)
            .context(cycle_view::CONTEXT)?;
        let rendered = if context.stdout_tty() {
            use std::num::NonZeroU16;
            let columns = u16::try_from(table::stdout_columns(true))
                .ok()
                .and_then(NonZeroU16::new)
                .unwrap_or(crate::platform::markdown_terminal::FALLBACK_COLUMNS);
            let options = crate::platform::markdown_terminal::RenderOptions::for_terminal(
                columns,
                context.color(),
                None,
                crate::platform::markdown_terminal::HostSource::System,
            );
            crate::platform::markdown_terminal::render(&markdown, &options)
                .context(cycle_view::CONTEXT)?
        } else {
            markdown
        };
        format!("{rendered}\n").into_bytes()
    };
    context.print(&output)?;
    Ok(())
}

fn dispatch_label_list(
    context: &Ctx,
    action: &cli::label::LabelList,
    workspace: Option<&str>,
) -> Result<()> {
    let flags = label_list::Options {
        team: action.team.clone(),
        workspace_only: action.workspace_only,
        all: action.all,
        json: action.json,
    };
    let cli_workspace = workspace;
    let show_spinner = spinner::enabled(flags.json, context.stdout_tty(), true);
    if show_spinner {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let prepared = (|| {
        let config = context.config();
        let credentials = context.credentials()?;
        let inputs = client::selection_inputs(&config.options, cli_workspace);
        let transport = client::prepare_transport_with_inputs(
            &config.options,
            credentials,
            &inputs,
            &config.transport_env,
        )?;
        let scope = WorkspaceScope::from_selection(&inputs, credentials);
        let configured_team = configured_team_key(&config.options);
        let selection = label_list::select(&flags, configured_team.as_deref(), &scope)?;
        Ok::<_, Error>((transport, selection))
    })();
    let (transport, selection) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            if show_spinner {
                context.print(spinner::CLEAR)?;
            }
            return Err(if !error.has_context() {
                error.context(label_list::CONTEXT)
            } else {
                error
            });
        }
    };
    let columns = table::stdout_columns(context.stdout_tty());
    let color = context.stdout_tty() && context.color();
    let output_result = if show_spinner {
        block_on_network(async {
            let pending = label_list::run(&transport, selection, flags.json, columns, color);
            tokio::pin!(pending);
            let mut ticks = tokio::time::interval(spinner::TICK_INTERVAL);
            ticks.tick().await;
            let mut frame = 1;
            loop {
                tokio::select! {
                    biased;
                    result = &mut pending => break result,
                    _ = ticks.tick() => {
                        context.print(
                            spinner::frame(frame).as_bytes())?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        })
    } else {
        block_on_network(async {
            label_list::run(&transport, selection, flags.json, columns, color).await
        })
    };
    if show_spinner {
        context.print(spinner::CLEAR)?;
    }
    let output = output_result.map_err(|error| {
        if !error.has_context() {
            error.context(label_list::CONTEXT)
        } else {
            error
        }
    })?;
    context.print(&output)?;
    Ok(())
}

fn dispatch_template_list(
    context: &Ctx,
    action: &cli::template::TemplateList,
    workspace: Option<&str>,
) -> Result<()> {
    let json = action.json;
    let template_type = action.r#type.map(|value| match value {
        cli::TemplateType::Issue => template_list::TemplateType::Issue,
        cli::TemplateType::Project => template_list::TemplateType::Project,
        cli::TemplateType::Document => template_list::TemplateType::Document,
    });
    let team_reference = action.team.as_deref();
    let show_spinner = spinner::enabled(json, context.stdout_tty(), true);
    if show_spinner {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let prepared = (|| {
        let config = context.config();
        let credentials = context.credentials()?;

        template_list::prepare(
            &config.options,
            credentials,
            workspace,
            &config.transport_env,
            team_reference,
        )
    })();
    let prepared = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            if show_spinner {
                context.print(spinner::CLEAR)?;
            }
            return Err(error);
        }
    };
    let columns = table::stdout_columns(context.stdout_tty());
    let color = context.stdout_tty() && context.color();
    let options = template_list::Options {
        template_type,
        json,
    };
    let output_result = if show_spinner {
        block_on_network(async {
            let pending = template_list::run(
                &prepared.transport,
                prepared.team.as_ref(),
                options,
                columns,
                color,
            );
            tokio::pin!(pending);
            let mut ticks = tokio::time::interval(spinner::TICK_INTERVAL);
            ticks.tick().await;
            let mut frame = 1;
            loop {
                tokio::select! {
                    biased;
                    result = &mut pending => break result,
                    _ = ticks.tick() => {
                        context.print(
                            spinner::frame(frame).as_bytes())?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        })
    } else {
        block_on_network(async {
            template_list::run(
                &prepared.transport,
                prepared.team.as_ref(),
                options,
                columns,
                color,
            )
            .await
        })
    };
    if show_spinner {
        context.print(spinner::CLEAR)?;
    }
    let output = output_result.map_err(|error| {
        if !error.has_context() {
            error.context(template_list::CONTEXT)
        } else {
            error
        }
    })?;
    context.print(&output)?;
    Ok(())
}

fn dispatch_template_view(
    context: &Ctx,
    action: &cli::template::TemplateView,
    workspace: Option<&str>,
) -> Result<()> {
    let json = action.json;
    let reference = &action.template;
    // The spinner starts before the URL check and credential selection, and
    // stops before either failure is reported.
    let show_spinner = spinner::enabled(json, context.stdout_tty(), true);
    if show_spinner {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let prepared = (|| {
        let config = context.config();
        let credentials = context.credentials()?;

        template_view::prepare(
            &config.options,
            credentials,
            workspace,
            &config.transport_env,
            reference,
        )
    })();
    let prepared = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            if show_spinner {
                context.print(spinner::CLEAR)?;
            }
            return Err(if !error.has_context() {
                error.context(template_view::CONTEXT)
            } else {
                error
            });
        }
    };
    let zone = chrono::Local;
    let output_result = if show_spinner {
        block_on_network(async {
            let pending = template_view::run(&prepared.transport, &prepared.reference, json, &zone);
            tokio::pin!(pending);
            let mut ticks = tokio::time::interval(spinner::TICK_INTERVAL);
            ticks.tick().await;
            let mut frame = 1;
            loop {
                tokio::select! {
                    biased;
                    result = &mut pending => break result,
                    _ = ticks.tick() => {
                        context.print(
                            spinner::frame(frame).as_bytes())?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        })
    } else {
        block_on_network(async {
            template_view::run(&prepared.transport, &prepared.reference, json, &zone).await
        })
    };
    if show_spinner {
        context.print(spinner::CLEAR)?;
    }
    let output = output_result.map_err(|error| {
        if !error.has_context() {
            error.context(template_view::CONTEXT)
        } else {
            error
        }
    })?;
    context.print(&output)?;
    Ok(())
}

fn inferred_issue(context: &Ctx) -> Result<Option<String>, Error> {
    let vcs = context
        .config()
        .options
        .vcs()
        .map(|v| *v.value())
        .unwrap_or(crate::config::Vcs::Git);
    crate::platform::vcs::infer_issue(vcs, context.cwd())
}

fn dispatch_issue_id(context: &Ctx) -> Result<()> {
    let id = inferred_issue(context)
        .and_then(|id| id.ok_or_else(|| issue_details::unresolved(true)))
        .context("Failed to get issue ID")?;
    context.print(format!("{id}\n").as_bytes())?;
    Ok(())
}

#[derive(Clone, Copy)]
enum IssueDetailField {
    Title,
    Url,
}
impl IssueDetailField {
    fn context(self) -> &'static str {
        match self {
            Self::Title => "Failed to get issue title",
            Self::Url => "Failed to get issue URL",
        }
    }
}

fn resolve_issue(
    context: &Ctx,
    input: Option<&str>,
    workspace: Option<&str>,
) -> Result<String, Error> {
    let reference = if input.is_none() {
        crate::refs::IssueReference::Inferred
    } else {
        let config = context.config();
        let credentials = context.credentials()?;
        let inputs = client::selection_inputs(&config.options, workspace);
        let team = configured_team_key(&config.options);
        crate::refs::prepare_issue_reference(
            input,
            team.as_deref(),
            &WorkspaceScope::from_selection(&inputs, credentials),
        )?
    };
    let id = match reference {
        crate::refs::IssueReference::Identifier(id) => Some(id),
        crate::refs::IssueReference::Unresolved => None,
        crate::refs::IssueReference::Inferred => inferred_issue(context)?,
    };
    id.ok_or_else(|| issue_details::unresolved(false))
}

fn dispatch_issue_detail(
    context: &Ctx,
    input: Option<&str>,
    workspace: Option<&str>,
    field: IssueDetailField,
) -> Result<()> {
    let id = resolve_issue(context, input, workspace).context(field.context())?;
    let show_spinner = spinner::enabled(false, context.stdout_tty(), true);
    if show_spinner {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let result = (|| {
        let config = context.config();
        let transport = client::prepare_transport(
            &config.options,
            context.credentials()?,
            workspace,
            &config.transport_env,
        )?;
        if !show_spinner {
            return block_on_network(issue_details::fetch(&transport, id));
        }
        block_on_network(async {
            let pending = issue_details::fetch(&transport, id);
            tokio::pin!(pending);
            let mut ticks = tokio::time::interval(spinner::TICK_INTERVAL);
            ticks.tick().await;
            let mut frame = 1_usize;
            loop {
                tokio::select! {
                    biased;
                    result = &mut pending => break result,
                    _ = ticks.tick() => {
                        context.print(spinner::frame(frame).as_bytes())?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        })
    })();
    if show_spinner {
        context.print(spinner::CLEAR)?;
    }
    let detail = result.context(field.context())?;
    let value = match field {
        IssueDetailField::Title => detail.title,
        IssueDetailField::Url => detail.url,
    };
    context.print(format!("{value}\n").as_bytes())?;
    Ok(())
}

fn agent_session_network<T>(
    context: &Ctx,
    json: bool,
    pending: impl Future<Output = Result<T, Error>>,
) -> Result<T, Error> {
    let enabled = spinner::enabled(json, context.stdout_tty(), true);
    if !enabled {
        return block_on_network(pending);
    }
    context.print(spinner::frame(0).as_bytes())?;
    let result = block_on_network(async {
        tokio::pin!(pending);
        let mut ticks = tokio::time::interval(spinner::TICK_INTERVAL);
        ticks.tick().await;
        let mut frame = 1_usize;
        loop {
            tokio::select! {
                biased;
                result = &mut pending => break result,
                _ = ticks.tick() => {
                    context.print(spinner::frame(frame).as_bytes())?;
                    frame = frame.wrapping_add(1);
                }
            }
        }
    });
    context.print(spinner::CLEAR)?;
    result
}

fn dispatch_agent_session_view(
    context: &Ctx,
    action: &cli::issue::IssueAgentSessionView,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::commands::agent_session;
    let output = (|| {
        crate::refs::reject_linear_url(&action.session_id, "an agent session ID")?;
        let transport = relation_transport(context, workspace)?;
        let session = agent_session_network(
            context,
            action.json,
            agent_session::view(&transport, &action.session_id),
        )?;
        if action.json {
            return agent_session::json(&session);
        }
        let markdown = agent_session::markdown(&session, chrono::Utc::now(), &chrono::Local)?;
        let rendered = if context.stdout_tty() {
            use std::num::NonZeroU16;
            let columns = u16::try_from(table::stdout_columns(true))
                .ok()
                .and_then(NonZeroU16::new)
                .unwrap_or(crate::platform::markdown_terminal::FALLBACK_COLUMNS);
            let options = crate::platform::markdown_terminal::RenderOptions::for_terminal(
                columns,
                context.color(),
                None,
                crate::platform::markdown_terminal::HostSource::System,
            );
            crate::platform::markdown_terminal::render(&markdown, &options)?
        } else {
            markdown
        };
        Ok(format!("{rendered}\n").into_bytes())
    })()
    .context(agent_session::VIEW_CONTEXT)?;
    context.print(&output)?;
    Ok(())
}

fn dispatch_agent_session_list(
    context: &Ctx,
    action: &cli::issue::IssueAgentSessionList,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::commands::agent_session;
    let output = (|| {
        let id =
            resolve_relation_reference(context, action.issue_id.as_deref(), workspace, || {
                issue_details::unresolved(false)
            })?;
        let transport = relation_transport(context, workspace)?;
        let comments = agent_session_network(
            context,
            action.json,
            agent_session::list(&transport, &id, action.status),
        )?;
        if action.json {
            return agent_session::json(&comments);
        }
        Ok(agent_session::text(
            &comments,
            table::stdout_columns(context.stdout_tty()),
            context.color(),
        ))
    })()
    .context(agent_session::LIST_CONTEXT)?;
    context.print(&output)?;
    Ok(())
}

struct InitiativeAction<'a> {
    target: Option<&'a str>,
    force: bool,
    bulk: initiative_bulk::BulkInput<'a>,
}

fn initiative_prompt_confirm(
    context: &Ctx,
    message: &str,
    default: bool,
) -> Result<crate::platform::prompt::PromptOutcome<bool>, Error> {
    use crate::platform::prompt::PromptSession;
    if !context.stdin_tty() {
        return Err(Error::new(
            "Interactive confirmation required. Use --force to skip.",
        ));
    }
    let mut session = PromptSession::confirmation_stdio(context.stdout())?;
    let result = session.confirm(message, default);
    session.finish_result(result)
}

fn initiative_prompt_stop<T>(
    outcome: crate::platform::prompt::PromptOutcome<T>,
) -> Result<T, Error> {
    use crate::platform::prompt::PromptOutcome;
    match outcome {
        PromptOutcome::Submitted(value) => Ok(value),
        PromptOutcome::Interrupted => Err(Error::cancelled()),
        PromptOutcome::EndOfInput => Err(Error::new(
            "unexpected EOF while prompting for confirmation",
        )),
    }
}

fn initiative_interrupt_status() -> Result<()> {
    Err(Error::cancelled())
}

fn dispatch_initiative_bulk(
    context: &Ctx,
    action: InitiativeAction<'_>,
    mode: initiative_bulk::Mode,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::platform::prompt::{PromptOutcome, PromptSession};
    // The client is built before reading or validating the collected IDs.
    let transport = {
        let config = context.config();
        let credentials = context.credentials()?;
        let inputs = client::selection_inputs(&config.options, workspace);
        client::prepare_transport_with_inputs(
            &config.options,
            credentials,
            &inputs,
            &config.transport_env,
        )?
    };
    if action.bulk.requested() {
        let ids = initiative_bulk::collect_ids(&action.bulk, &mut std::io::stdin().lock())?;
        if ids.is_empty() {
            return Err(Error::new(format!(
                "No initiative IDs provided for bulk {}.",
                mode.verb()
            )));
        }
        context
            .print(format!("Found {} initiative(s) to {}.\n", ids.len(), mode.verb()).as_bytes())?;
        if mode == initiative_bulk::Mode::Delete {
            context.print("\n⚠️  This action is PERMANENT and cannot be undone.\n\n".as_bytes())?;
        }
        if !action.force {
            let message = match mode {
                initiative_bulk::Mode::Archive => format!("Archive {} initiative(s)?", ids.len()),
                initiative_bulk::Mode::Delete => {
                    format!("Permanently delete {} initiative(s)?", ids.len())
                }
            };
            let outcome = initiative_prompt_confirm(context, &message, false)?;
            if matches!(outcome, PromptOutcome::Interrupted) {
                return initiative_interrupt_status();
            }
            if !initiative_prompt_stop(outcome)? {
                context.print(mode.bulk_cancelled())?;
                return Ok(());
            }
        }
        let targets = {
            let config = context.config();
            let credentials = context.credentials()?;
            let inputs = client::selection_inputs(&config.options, workspace);
            let scope = WorkspaceScope::from_selection(&inputs, credentials);
            ids.into_iter()
                .map(|id| initiative_bulk::Target::prepare(id, &scope))
                .collect()
        };
        let progress_enabled = spinner::enabled(false, context.stdout_tty(), true);
        let results = block_on_network(initiative_bulk::execute(
            &transport,
            targets,
            mode,
            |progress| {
                if progress_enabled {
                    context.print(progress.render())?;
                }
                Ok(())
            },
        ))?;
        if progress_enabled {
            context.print(initiative_bulk::PROGRESS_CLEAR)?;
        }
        let (output, failed) = initiative_bulk::summary(&results, mode);
        context.print(&output)?;
        return if failed {
            Err(Error::reported())
        } else {
            Ok(())
        };
    }
    let original = action
        .target
        .filter(|target| !target.is_empty())
        .ok_or_else(|| {
            Error::new("Initiative ID required. Use --bulk for multiple initiatives.")
        })?;
    let target = {
        let config = context.config();
        let credentials = context.credentials()?;
        let inputs = client::selection_inputs(&config.options, workspace);
        initiative_bulk::Target::prepare(
            original.to_owned(),
            &WorkspaceScope::from_selection(&inputs, credentials),
        )
    };
    let id = block_on_network(initiative_bulk::resolve(
        &transport,
        &target.reference?,
        mode,
    ))?
    .ok_or_else(|| Error::not_found("Initiative", original))?;
    let detail = block_on_network(initiative_bulk::fetch_single(&transport, &id, mode))?
        .ok_or_else(|| Error::not_found("Initiative", original))?;
    if detail.already_archived() {
        context
            .print(format!("Initiative \"{}\" is already archived.\n", detail.name()).as_bytes())?;
        return Ok(());
    }
    if let Some(warning) = detail.linked_warning() {
        context.print(&warning)?;
    }
    if !action.force {
        // The terminal check comes before the permanent-deletion warning.
        if !context.stdin_tty() {
            return Err(Error::new(
                "Interactive confirmation required. Use --force to skip.",
            ));
        }
        let (message, default) = match mode {
            initiative_bulk::Mode::Archive => {
                (format!("Archive initiative \"{}\"?", detail.name()), true)
            }
            initiative_bulk::Mode::Delete => {
                context
                    .print("\n⚠️  This action is PERMANENT and cannot be undone.\n\n".as_bytes())?;
                (
                    format!(
                        "Are you sure you want to permanently delete \"{}\"?",
                        detail.name()
                    ),
                    false,
                )
            }
        };
        let outcome = initiative_prompt_confirm(context, &message, default)?;
        if matches!(outcome, PromptOutcome::Interrupted) {
            return initiative_interrupt_status();
        }
        if !initiative_prompt_stop(outcome)? {
            context.print(mode.single_cancelled())?;
            return Ok(());
        }
        if mode == initiative_bulk::Mode::Delete {
            // Keep the raw answer; the prompt owns terminal handling and rendering.
            let raw = std::cell::RefCell::new(String::new());
            let outcome = {
                let mut session = PromptSession::confirmation_stdio(context.stdout())?;
                let result = session.text(
                    "Type the initiative name to confirm deletion:",
                    0,
                    |answer| {
                        *raw.borrow_mut() = answer.to_owned();
                        Ok(())
                    },
                );
                session.finish_result(result)?
            };
            if matches!(outcome, PromptOutcome::Interrupted) {
                return initiative_interrupt_status();
            }
            initiative_prompt_stop(outcome)?;
            if raw.into_inner().trim() != detail.name() {
                context.print(b"Name does not match. Delete cancelled.\n")?;
                return Ok(());
            }
        }
    }
    let show_spinner = spinner::enabled(false, context.stdout_tty(), true);
    if show_spinner {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let result = block_on_network(initiative_bulk::submit_single(
        &transport,
        &id,
        detail.name(),
        mode,
    ));
    if show_spinner {
        context.print(spinner::CLEAR)?;
    }
    context.print(&result?)?;
    Ok(())
}

use crate::commands::{issue_upload, upload};
/// Order: hidden id, body flags, identifier, all-file prevalidation,
/// sequential uploads with immediate output, line prompt only with no links,
/// client then parent validation then AddComment. No pre-target API lookup.
fn dispatch_issue_comment_add(
    context: &Ctx,
    action: &cli::issue::IssueCommentAdd,
    workspace: Option<&str>,
) -> Result<()> {
    let result = (|| {
        issue_upload::validate_comment_id(action.id.as_deref())?;
        let text = comment_add::resolve_body(action.body.as_deref(), action.body_file.as_deref())?;
        let identifier = resolve_relation_reference(
            context,
            action.issue_id.as_deref(),
            workspace,
            issue_upload::unresolved,
        )?;
        if action.public && action.attach.is_empty() {
            return Err(Error::new("--public requires at least one --attach")
                .with_hint("Add --attach <file> to upload, or remove --public."));
        }
        upload::prevalidate(&action.attach, action.public)?;
        let mut files = Vec::with_capacity(action.attach.len());
        let mut upload_transport = None;
        for path in &action.attach {
            files.push(upload_issue_file(
                context,
                workspace,
                path,
                action.public,
                &mut upload_transport,
            )?);
        }
        let text = if text.is_none() && files.is_empty() {
            Some(prompt_comment_body(context)?)
        } else {
            text
        };
        let body = issue_upload::compose_body(text.as_deref(), &files);
        // `createComment` constructs its client BEFORE parent URL validation.
        let transport = match upload_transport {
            Some(transport) => transport,
            None => relation_transport(context, workspace)?,
        };
        let input = comment_add::build_input(
            comment_add::CommentTarget::Issue {
                issue_id: identifier.clone(),
            },
            body,
            action.parent.as_deref(),
            action.id.as_deref(),
        )?;
        let comment = block_on_network(comment_add::create(&transport, input))?;
        Ok(issue_upload::comment_output(&identifier, &comment.url))
    })();
    finish_comment_add(context, result)
}
fn dispatch_issue_attach(
    context: &Ctx,
    action: &cli::issue::IssueAttach,
    workspace: Option<&str>,
) -> Result<()> {
    let output = (|| -> Result<Vec<u8>> {
        let identifier = resolve_relation_reference(
            context,
            Some(&action.issue_id),
            workspace,
            issue_upload::unresolved,
        )?;
        upload::validate_file(std::path::Path::new(&action.filepath))?;
        let transport = relation_transport(context, workspace)?;
        let issue_uuid = block_on_network(issue_upload::lookup(&transport, &identifier))?;
        // Public-upload eligibility and size checks happen after the UUID lookup.
        let mut upload_transport = Some(transport);
        let file = upload_issue_file(
            context,
            workspace,
            &action.filepath,
            action.public,
            &mut upload_transport,
        )?;
        let Some(transport) = upload_transport.as_ref() else {
            unreachable!("lookup established transport");
        };
        let attachment = block_on_network(issue_upload::attach(
            transport,
            &issue_uuid,
            &file,
            action.title.as_deref(),
            action.comment.as_deref(),
        ))?;
        Ok(issue_upload::attach_output(
            &attachment,
            &identifier,
            &action.filepath,
            &file,
        ))
    })()
    .context(issue_upload::ATTACH_CONTEXT)?;
    context.print(&output)?;
    Ok(())
}
/// File metadata/public validation before client/spinner; spinner only encloses
/// FileUpload+PUT. Every completed file is printed before later failures.
fn upload_issue_file(
    context: &Ctx,
    workspace: Option<&str>,
    path: &str,
    public: bool,
    transport_slot: &mut Option<crate::graphql::transport::GraphQlTransport>,
) -> Result<upload::UploadedFile, Error> {
    let path = std::path::Path::new(path);
    let file = upload::prepare(path, public)?;
    if transport_slot.is_none() {
        *transport_slot = Some(relation_transport(context, workspace)?);
    }
    let Some(transport) = transport_slot.as_ref() else {
        unreachable!("transport initialized after metadata checks");
    };
    let show_spinner = context.stdout_tty();
    let filename = file.filename.clone();
    let pending = upload::upload(transport, path, file);
    let uploaded = if show_spinner {
        // Each frame clears the line and resets color before the message.
        let frame = |tick: usize| format!("{}Uploading {filename}...", spinner::frame(tick));
        context.print(frame(0).as_bytes())?;
        let result = block_on_network(async {
            tokio::pin!(pending);
            let mut ticks = tokio::time::interval(spinner::TICK_INTERVAL);
            ticks.tick().await;
            let mut tick = 1;
            loop {
                tokio::select! {biased;result=&mut pending=>break result,_=ticks.tick()=>{context.print(frame(tick).as_bytes())?;tick=tick.wrapping_add(1);}}
            }
        });
        context.print(spinner::CLEAR)?;
        result?
    } else {
        block_on_network(pending)?
    };
    context.print(upload::output(&uploaded))?;
    if let Some(warning) = upload::warning(&uploaded) {
        context.eprint(&warning)?;
    }
    Ok(uploaded)
}

fn dispatch_document_list(
    context: &Ctx,
    action: &cli::document::DocumentList,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::commands::{document_list, document_target};
    let result: Result<()> = (|| {
        let config = context.config();
        let credentials = context.credentials()?;
        let inputs = client::selection_inputs(&config.options, workspace);
        let team = configured_team_key(&config.options);
        let target = document_target::prepare(
            action,
            &WorkspaceScope::from_selection(&inputs, credentials),
            team.as_deref(),
        )?;
        let first = i32::try_from(action.limit.get()).map_err(|error| {
            Error::new("Document limit exceeds GraphQL's signed integer range").with_source(error)
        })?;
        let transport = client::prepare_transport_with_inputs(
            &config.options,
            credentials,
            &inputs,
            &config.transport_env,
        )?;
        let documents = document_fetch_with_spinner(context, action.json, async {
            let filter = match target {
                Some(target) => {
                    let (kind, id) = document_target::resolve(&target, &transport).await?;
                    Some(document_target::filter(kind, id))
                }
                None => None,
            };
            document_list::fetch(&transport, filter, first).await
        })?;
        let output = if action.json {
            document_list::json(&documents)?
        } else {
            let columns = crate::platform::pager::stdout_size()
                .map(|size| usize::from(size.columns))
                .unwrap_or(120);
            document_list::text(
                &documents,
                columns,
                context.stdout_tty() && context.color(),
                std::time::SystemTime::now(),
            )
            .into_bytes()
        };
        context.print(&output)?;
        Ok(())
    })();
    result.context(document_list::CONTEXT)
}

fn dispatch_document_view(
    context: &Ctx,
    action: &cli::document::DocumentView,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::commands::document_view;
    use crate::platform::{markdown_assets, markdown_ast, markdown_serializer, markdown_terminal};
    let result = (|| {
        let config = context.config();
        let credentials = context.credentials()?;
        let inputs = client::selection_inputs(&config.options, workspace);
        let id = resolve_document_reference(
            &action.id,
            &WorkspaceScope::from_selection(&inputs, credentials),
        )?;
        let cache_root = config.image_cache_root.clone();
        let download = !action.no_download
            && config
                .options
                .download_images()
                .is_none_or(|value| *value.value());
        let hyperlink = config
            .options
            .hyperlink_format()
            .map(|value| value.value().clone());
        let transport = client::prepare_transport_with_inputs(
            &config.options,
            credentials,
            &inputs,
            &config.transport_env,
        )?;
        let document = document_fetch_with_spinner(
            context,
            action.raw || action.json,
            document_view::fetch(&transport, &action.id, &id, action.json),
        )?;
        if action.web {
            context.print(format!("Opening {} in web browser\n", document.url()).as_bytes())?;
            crate::platform::opener::open(document.url(), false)?;
            return Ok(());
        }
        if action.json {
            context.print(&document.json()?)?;
            return Ok(());
        }
        let document = match document {
            document_view::DocumentResult::Body(document) => document,
            document_view::DocumentResult::WithComments(_) => {
                return Err(Error::new(
                    "Non-JSON document unexpectedly included comments",
                ));
            }
        };
        let mut content = document.content.clone();
        if download && let Some(original) = content.as_deref().filter(|value| !value.is_empty()) {
            let paths = block_on_network(markdown_assets::download_with(
                original,
                &cache_root,
                |url| {
                    let transport = &transport;
                    async move { transport.download_markdown_image(&url).await }
                },
                |bytes| context.eprint(bytes),
            ))?
            .paths;
            if !paths.is_empty() {
                content = Some(markdown_ast::rewrite_with(
                    original,
                    &paths,
                    markdown_serializer::serialize,
                )?);
            }
        }
        let output = if action.raw || !context.stdout_tty() {
            document_view::raw(content.as_deref())
        } else {
            let markdown = document_view::markdown(
                &document,
                content.as_deref(),
                chrono::Utc::now(),
                &chrono::Local,
            );
            let columns = crate::platform::pager::stdout_size()
                .and_then(|size| std::num::NonZeroU16::new(size.columns))
                .unwrap_or(markdown_terminal::FALLBACK_COLUMNS);
            let options = markdown_terminal::RenderOptions::for_terminal(
                columns,
                context.color(),
                hyperlink.as_deref(),
                markdown_terminal::HostSource::System,
            );
            format!("{}\n", markdown_terminal::render(&markdown, &options)?).into_bytes()
        };
        context.print(&output)?;
        Ok(())
    })();
    result.context(document_view::CONTEXT)
}

fn document_fetch_with_spinner<T>(
    context: &Ctx,
    json: bool,
    pending: impl std::future::Future<Output = Result<T, Error>>,
) -> Result<T, Error> {
    let enabled = spinner::enabled(json, context.stdout_tty(), true);
    if !enabled {
        return block_on_network(pending);
    }
    context.print(spinner::frame(0).as_bytes())?;
    let result = block_on_network(async {
        tokio::pin!(pending);
        let mut ticks = tokio::time::interval(spinner::TICK_INTERVAL);
        ticks.tick().await;
        let mut frame = 1;
        loop {
            tokio::select! {
                biased;
                result = &mut pending => break result,
                _ = ticks.tick() => {
                    context.print(spinner::frame(frame).as_bytes())?;
                    frame = frame.wrapping_add(1);
                }
            }
        }
    });
    context.print(spinner::CLEAR)?;
    result
}

fn delete_confirmation(
    context: &Ctx,
    message: &str,
    flag: &str,
) -> Result<crate::platform::prompt::PromptOutcome<bool>, Error> {
    if !context.stdin_tty() {
        return Err(Error::new("Interactive confirmation required")
            .with_hint(format!("Use --{flag} to skip.")));
    }
    let mut session = crate::platform::prompt::PromptSession::confirmation_stdio(context.stdout())?;
    let outcome = session.confirm(message, false);
    session.finish_result(outcome)
}
fn dispatch_team_delete(
    context: &Ctx,
    action: &cli::team::TeamDelete,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::{
        commands::team_delete,
        graphql::operations::team_delete::GetTeamDetails,
        platform::prompt::{PlainOption, PlainSelect, PromptOutcome, PromptSession},
    };
    let result = (|| {
        let transport = relation_transport(context, workspace)?;
        let prepared = {
            let inputs = client::selection_inputs(&context.config().options, workspace);
            prepare_team_lookup(
                &action.team,
                &WorkspaceScope::from_selection(&inputs, context.credentials()?),
            )?
        };
        let source = block_on_network(resolve_team_with_transport(&prepared, &transport))?;
        let details: GetTeamDetails = block_on_network(async {
            transport
                .execute(&team_delete::details_request(&source.id))
                .await
                .map_err(Error::from)
        })?;
        let team = details
            .team
            .ok_or_else(|| Error::not_found("Team", &action.team))?;
        let count = team.issues.nodes.len();
        if count > 0 {
            let target = match action.move_issues.as_deref().filter(|s| !s.is_empty()) {
                Some(reference) => {
                    let prepared = {
                        let inputs = client::selection_inputs(&context.config().options, workspace);
                        prepare_team_lookup(
                            reference,
                            &WorkspaceScope::from_selection(&inputs, context.credentials()?),
                        )?
                    };
                    let target =
                        block_on_network(resolve_team_with_transport(&prepared, &transport))?;
                    if target.id == source.id {
                        return Err(Error::new("Cannot move issues to the same team"));
                    }
                    target.id
                }
                None => {
                    context.print(team_delete::warning(&team))?;
                    if !context.stdin_tty() {
                        return Err(Error::new("Interactive selection required")
                            .with_hint("Use --move-issues <teamKey> to specify target team."));
                    }
                    let teams =
                        block_on_network(crate::refs::fetch_all_teams_with_transport(&transport))?;
                    let options: Vec<_> = teams
                        .into_iter()
                        .filter(|t| t.id != source.id)
                        .map(|t| PlainOption {
                            label: format!("{} ({})", t.name, t.key),
                            script_token: t.id.clone(),
                            value: t.id,
                        })
                        .collect();
                    if options.is_empty() {
                        return Err(Error::new("No other teams available to move issues to"));
                    }
                    let outcome = {
                        let mut session = PromptSession::confirmation_stdio(context.stdout())?;
                        let result = session.select(&PlainSelect {
                            message: "Select a team to move issues to:",
                            options: &options,
                            default_index: 0,
                            default_hint: None,
                        });
                        session.finish_result(result)?
                    };
                    match outcome {
                        PromptOutcome::Submitted(id) => {
                            if !options.iter().any(|o| o.value == id) {
                                return Err(Error::new("selected target team is missing"));
                            }
                            id
                        }
                        PromptOutcome::Interrupted => return initiative_interrupt_status(),
                        PromptOutcome::EndOfInput => {
                            return Err(Error::new("unexpected EOF while selecting a team"));
                        }
                    }
                }
            };
            team_delete_moves(context, &transport, &source.id, &target, count)
                .context(team_delete::MOVE_CONTEXT)?;
        }
        if !action.force {
            match delete_confirmation(
                context,
                &format!(
                    "Are you sure you want to delete team \"{}: {}\"?",
                    team.key, team.name
                ),
                "force",
            )? {
                PromptOutcome::Submitted(true) => {}
                PromptOutcome::Submitted(false) => {
                    context.print(b"Delete cancelled.\n")?;
                    return Ok(());
                }
                PromptOutcome::Interrupted => return initiative_interrupt_status(),
                PromptOutcome::EndOfInput => {
                    return Err(Error::new(
                        "unexpected EOF while prompting for confirmation",
                    ));
                }
            }
        }
        let result: crate::graphql::operations::team_delete::DeleteTeam =
            block_on_network(async {
                transport
                    .execute(&team_delete::delete_request(&source.id))
                    .await
                    .map_err(Error::from)
            })?;
        if !result.team_delete.success {
            return Err(Error::new("Failed to delete team"));
        }
        context.print(team_delete::deleted(&team))?;
        Ok(())
    })();
    result.map_err(|error: Error| {
        if error.has_context() {
            error
        } else {
            error.context(team_delete::CONTEXT)
        }
    })
}
fn team_delete_moves(
    context: &Ctx,
    transport: &crate::graphql::transport::GraphQlTransport,
    source: &str,
    target: &str,
    count: usize,
) -> Result<(), Error> {
    use crate::commands::team_delete;
    let show = spinner::enabled(false, context.stdout_tty(), true);
    let message = std::cell::RefCell::new(format!("Moving {count} issue(s) to target team..."));
    let pending = async {
        let issues = team_delete::all_issues(source, |request| async move {
            transport.execute(&request).await.map_err(Error::from)
        })
        .await?;
        team_delete::move_all(
            &issues,
            target,
            |request| async move { transport.execute(&request).await.map_err(Error::from) },
            |moved, total| {
                *message.borrow_mut() = format!("Moving issues... ({moved}/{total})");
                Ok(())
            },
        )
        .await
    };
    let result = if show {
        context.print(format!("{}{}", spinner::frame(0), message.borrow()).as_bytes())?;
        block_on_network(async {
            tokio::pin!(pending);
            let mut ticks = tokio::time::interval(spinner::TICK_INTERVAL);
            ticks.tick().await;
            let mut frame = 1;
            loop {
                tokio::select! {biased;result=&mut pending=>break result,_=ticks.tick()=>{context.print(format!("{}{}",spinner::frame(frame),message.borrow()).as_bytes())?;frame=frame.wrapping_add(1);}}
            }
        })
    } else {
        block_on_network(pending)
    };
    if show {
        context.print(spinner::CLEAR)?
    }
    let moved = result?;
    context.print(format!("✓ Moved {moved} issue(s) to target team\n").as_bytes())
}
fn dispatch_document_delete(
    context: &Ctx,
    action: &cli::document::DocumentDelete,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::{commands::document_delete as command, platform::prompt::PromptOutcome};
    let result = (|| {
        let transport = relation_transport(context, workspace)?;
        let input = initiative_bulk::BulkInput {
            argv: action.bulk.as_deref(),
            file: action.bulk_file.as_deref().map(std::path::Path::new),
            stdin: action.bulk_stdin,
        };
        if input.requested() {
            let ids = initiative_bulk::collect_ids(&input, &mut std::io::stdin().lock())?;
            if ids.is_empty() {
                return Err(Error::new("No document IDs provided for bulk delete"));
            }
            context.print(format!("Found {} document(s) to delete.\n", ids.len()).as_bytes())?;
            if !action.yes {
                match delete_confirmation(
                    context,
                    &format!("Delete {} document(s)?", ids.len()),
                    "yes",
                )? {
                    PromptOutcome::Submitted(true) => {}
                    PromptOutcome::Submitted(false) => {
                        context.print(b"Bulk delete cancelled.\n")?;
                        return Ok(());
                    }
                    PromptOutcome::Interrupted => return initiative_interrupt_status(),
                    PromptOutcome::EndOfInput => {
                        return Err(Error::new(
                            "unexpected EOF while prompting for confirmation",
                        ));
                    }
                }
            }
            let targets = {
                let inputs = client::selection_inputs(&context.config().options, workspace);
                let scope = WorkspaceScope::from_selection(&inputs, context.credentials()?);
                ids.into_iter()
                    .map(|id| command::Target::prepare(id, &scope))
                    .collect()
            };
            let show = spinner::enabled(false, context.stdout_tty(), true);
            let results = block_on_network(command::execute(&transport, targets, |progress| {
                if show {
                    context.print(progress.render())?
                }
                Ok(())
            }));
            if show {
                context.print(initiative_bulk::PROGRESS_CLEAR)?
            }
            let (output, failed) = command::summary(&results?);
            context.print(&output)?;
            return if failed {
                Err(Error::reported())
            } else {
                Ok(())
            };
        }
        let original = action
            .document_id
            .as_deref()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| {
                Error::new("Document ID required").with_hint("Use --bulk for multiple documents.")
            })?;
        let id = {
            let inputs = client::selection_inputs(&context.config().options, workspace);
            resolve_document_reference(
                original,
                &WorkspaceScope::from_selection(&inputs, context.credentials()?),
            )?
        };
        let document = block_on_network(command::single_details(&transport, original, &id))?;
        if !action.yes {
            match delete_confirmation(
                context,
                &format!("Are you sure you want to delete \"{}\"?", document.title),
                "yes",
            )? {
                PromptOutcome::Submitted(true) => {}
                PromptOutcome::Submitted(false) => {
                    context.print(b"Delete cancelled.\n")?;
                    return Ok(());
                }
                PromptOutcome::Interrupted => return initiative_interrupt_status(),
                PromptOutcome::EndOfInput => {
                    return Err(Error::new(
                        "unexpected EOF while prompting for confirmation",
                    ));
                }
            }
        }
        let output = block_on_network(command::submit_single(&transport, &document))?;
        context.print(&output)?;
        Ok(())
    })();
    result.context(command::CONTEXT)
}

fn document_write_target(
    context: &Ctx,
    target: crate::commands::document_target::TargetOptions<'_>,
    workspace: Option<&str>,
) -> Result<
    (
        crate::graphql::transport::GraphQlTransport,
        Option<crate::commands::document_target::PreparedTarget>,
    ),
    Error,
> {
    let config = context.config();
    let credentials = context.credentials()?;
    let inputs = client::selection_inputs(&config.options, workspace);
    let team = configured_team_key(&config.options);
    let prepared = crate::commands::document_target::prepare_options(
        target,
        &WorkspaceScope::from_selection(&inputs, credentials),
        team.as_deref(),
    )?;
    let transport = client::prepare_transport_with_inputs(
        &config.options,
        credentials,
        &inputs,
        &config.transport_env,
    )?;
    Ok((transport, prepared))
}
fn document_prompt_exit(
    outcome: crate::platform::prompt::PromptOutcome<crate::commands::document_write::Fields>,
) -> Result<crate::commands::document_write::Fields> {
    use crate::platform::prompt::PromptOutcome;
    match outcome {
        PromptOutcome::Submitted(fields) => Ok(fields),
        PromptOutcome::Interrupted => Err(Error::cancelled()),
        PromptOutcome::EndOfInput => Err(Error::new("unexpected EOF while prompting for document")),
    }
}
fn dispatch_document_create(
    context: &Ctx,
    action: &cli::document::DocumentCreate,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::commands::{document_target::TargetOptions, document_write as command, text_input};
    let result = (|| {
        let target = TargetOptions {
            project: action.project.as_deref(),
            issue: action.issue.as_deref(),
            initiative: action.initiative.as_deref(),
            team: action.team.as_deref(),
            cycle: action.cycle.as_deref(),
            release: action.release.as_deref(),
        };
        let interactive = context.stdout_tty()
            && (action.interactive
                || (action.title.is_none()
                    && action.content.is_none()
                    && action.content_file.is_none()
                    && action.icon.is_none()
                    && !target.any()));
        let root = std::env::temp_dir();
        let fields = if interactive {
            if target.any() {
                return Err(Error::new("Attachment target flags cannot be combined with interactive mode").with_hint("Drop the target flags to choose the attachment interactively, or drop -i/--interactive to use the flags."));
            }
            let config = context.config();
            let env = config.child_env.clone();
            let default_team = configured_team_key(&config.options);
            crate::platform::prompt_text::TextOptions {
                required: false,
                default: default_team.as_deref(),
            }
            .preflight()
            .map_err(Error::new)?;
            let mut session = crate::platform::prompt::PromptSession::stdio(context.stdout())?;
            let prompted = command::prompt(
                &mut session,
                &mut std::io::stderr(),
                command::PromptSettings {
                    env: &env,
                    temp_root: &root,
                    default_team: default_team.as_deref(),
                },
            );
            let outcome = session.finish_result(prompted)?;
            document_prompt_exit(outcome)?
        } else {
            let title = action.title.clone().ok_or_else(|| {
                Error::new("Title is required")
                    .with_hint("Use --title or run with -i for interactive mode.")
            })?;
            target.cardinality(true)?;
            let content = if let Some(content) = &action.content {
                Some(content.clone())
            } else if let Some(path) = &action.content_file {
                Some(command::file(path, false)?)
            } else if !context.stdin_tty() {
                text_input::read_stdin(std::io::stdin().lock())?
            } else if context.stdout_tty() {
                context.print(b"Opening editor for document content...\n")?;
                let env = context.config().child_env.clone();
                let content = command::optional_editor(&env, &mut std::io::stderr())?;
                if content.is_none() {
                    context.print(b"No content entered. Creating document without content.\n")?;
                }
                content
            } else {
                None
            };
            command::Fields {
                title: Some(title),
                content,
                icon: action.icon.clone(),
                project: action.project.clone(),
                issue: action.issue.clone(),
                initiative: action.initiative.clone(),
                team: action.team.clone(),
                cycle: action.cycle.clone(),
                release: action.release.clone(),
            }
        };
        let (transport, target) = document_write_target(context, fields.target(), workspace)?;
        let mut input = command::input(None, fields.icon);
        input.content = fields.content;
        let output = block_on_network(async {
            if let Some(target) = target {
                let (kind, id) =
                    crate::commands::document_target::resolve(&target, &transport).await?;
                command::attach(&mut input, kind, id);
            }
            let title = fields
                .title
                .filter(|title| !title.is_empty())
                .ok_or_else(|| Error::new("Title is required"))?;
            command::create(&transport, title, input).await
        })?;
        context.print(&output)?;
        Ok(())
    })();
    result.context("Failed to create document")
}
fn dispatch_document_update(
    context: &Ctx,
    action: &cli::document::DocumentUpdate,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::commands::{document_target::TargetOptions, document_write as command, text_input};
    let result = (|| {
        let config = context.config();
        let credentials = context.credentials()?;
        let inputs = client::selection_inputs(&config.options, workspace);
        let id = resolve_document_reference(
            &action.document_id,
            &WorkspaceScope::from_selection(&inputs, credentials),
        )?;
        let target = TargetOptions {
            project: action.project.as_deref(),
            issue: action.issue.as_deref(),
            initiative: action.initiative.as_deref(),
            team: action.team.as_deref(),
            cycle: action.cycle.as_deref(),
            release: action.release.as_deref(),
        };
        target.cardinality(false)?;
        let (transport, target) = document_write_target(context, target, workspace)?;
        let mut input = command::input(action.title.clone(), action.icon.clone());
        if let Some(target) = target {
            let (kind, id) = block_on_network(crate::commands::document_target::resolve(
                &target, &transport,
            ))?;
            command::attach(&mut input, kind, id);
        }
        input.content = if let Some(content) = &action.content {
            Some(content.clone())
        } else if let Some(path) = &action.content_file {
            Some(command::file(path, false)?)
        } else if action.edit {
            let document = block_on_network(command::for_edit(&transport, &id))?;
            let seed = document.content.unwrap_or_default();
            context.print(format!("Opening {} in editor...\n", document.title).as_bytes())?;
            let edited = context.edit_text(&seed)?;
            if edited == seed {
                context.print(b"No changes detected, update cancelled.\n")?;
                return Ok(());
            }
            let Some(content) = text_input::edited_body(&edited) else {
                context.print(b"No changes made, update cancelled.\n")?;
                return Ok(());
            };
            Some(content)
        } else if !context.stdin_tty() && !command::has_fields(&input) {
            text_input::read_stdin(std::io::stdin().lock())?
        } else {
            None
        };
        if !command::has_fields(&input) {
            return Err(Error::new("No update fields provided").with_hint("Use --title, --content, --content-file, --icon, --edit, or re-point the attachment with --project, --issue, --initiative, --team, --cycle, or --release."));
        }
        let output = block_on_network(async {
            if input.content.is_some() && !action.force {
                command::guard(&transport, &id).await?;
            }
            command::update(&transport, &id, input).await
        })?;
        context.print(&output)?;
        Ok(())
    })();
    result.context("Failed to update document")
}

fn project_ticks<T>(
    context: &Ctx,
    pending: impl std::future::Future<Output = Result<T, Error>>,
    enabled: bool,
) -> Result<T, Error> {
    if !enabled {
        return block_on_network(pending);
    }
    block_on_network(async {
        tokio::pin!(pending);
        let mut ticks = tokio::time::interval(spinner::TICK_INTERVAL);
        ticks.tick().await;
        let mut frame = 1_usize;
        loop {
            tokio::select! {biased;result=&mut pending=>break result,_=ticks.tick()=>{context.print(spinner::frame(frame).as_bytes())?;frame=frame.wrapping_add(1);}}
        }
    })
}
fn dispatch_project_create(
    context: &Ctx,
    action: &cli::project::ProjectCreate,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::commands::project_create as command;
    let result = (|| {
        // Original content/priority validation before authentication.
        let mut fields = command::local(action)?;
        let (options, default_workspace, transport) = {
            let config = context.config();
            let credentials = context.credentials()?;
            let options = config.options.clone();
            let inputs = client::selection_inputs(&options, workspace);
            let transport = client::prepare_transport_with_inputs(
                &options,
                credentials,
                &inputs,
                &config.transport_env,
            )?;
            (options, credentials.default().map(str::to_owned), transport)
        };
        let inputs = client::selection_inputs(&options, workspace);
        let scope = WorkspaceScope {
            cli_workspace: inputs.cli_workspace,
            sourced_workspace: inputs.sourced_workspace.as_ref().map(|(value, _)| *value),
            default_workspace: default_workspace.as_deref(),
            api_key: inputs.api_key.clone(),
        };
        let default_team = configured_team_key(&options);
        if command::interactive(&fields, action.interactive, context.stdout_tty()) {
            // Only stdout is checked here; piped stdin is not refused.
            context.print(b"\nCreate a new project\n\n")?;
            let mut session = crate::platform::prompt::PromptSession::stdio(context.stdout())?;
            let prompted = block_on_network(command::prompt(
                &mut session,
                &transport,
                fields,
                default_team.as_deref(),
            ));
            let outcome = session.finish_result(prompted)?;
            fields = match outcome {
                crate::platform::prompt::PromptOutcome::Submitted(fields) => fields,
                crate::platform::prompt::PromptOutcome::Interrupted => {
                    return initiative_interrupt_status();
                }
                crate::platform::prompt::PromptOutcome::EndOfInput => {
                    return Err(Error::new("unexpected EOF while prompting for project"));
                }
            };
        }
        let input = block_on_network(command::input(
            &transport,
            &scope,
            &fields,
            default_team.as_deref(),
        ))?;
        let enabled = spinner::enabled(action.json, context.stdout_tty(), true);
        if enabled {
            context.print(spinner::frame(0).as_bytes())?;
        }
        let created = project_ticks(context, command::submit(&transport, input), enabled);
        if enabled {
            context.print(spinner::CLEAR)?;
        }
        let payload = created?;
        // Spinner has stopped BEFORE postcreate initiative resolution/join/output.
        let output = block_on_network(command::followup_and_output(
            &transport,
            &scope,
            &payload,
            fields.initiative.as_deref(),
            action.json,
        ))?;
        context.eprint(&output.stderr)?;
        context.print(&output.stdout)?;
        Ok(())
    })();
    result.context("Failed to create project")
}
fn dispatch_project_update(
    context: &Ctx,
    action: &cli::project::ProjectUpdate,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::commands::project_update as command;
    let options = command::Options::from_cli(action);
    // Local input, file and date checks run before the spinner and client.
    let local = command::local(&options).context("Failed to update project")?;
    let enabled = spinner::enabled(false, context.stdout_tty(), true);
    if enabled {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let result = (|| {
        let (config_options, default_workspace, transport) = {
            let config = context.config();
            let credentials = context.credentials()?;
            let config_options = config.options.clone();
            let inputs = client::selection_inputs(&config_options, workspace);
            let transport = client::prepare_transport_with_inputs(
                &config_options,
                credentials,
                &inputs,
                &config.transport_env,
            )?;
            (
                config_options,
                credentials.default().map(str::to_owned),
                transport,
            )
        };
        let inputs = client::selection_inputs(&config_options, workspace);
        let scope = WorkspaceScope {
            cli_workspace: inputs.cli_workspace,
            sourced_workspace: inputs.sourced_workspace.as_ref().map(|(value, _)| *value),
            default_workspace: default_workspace.as_deref(),
            api_key: inputs.api_key.clone(),
        };
        project_ticks(
            context,
            async {
                let plan =
                    command::plan(&transport, &scope, &action.project_id, &options, local).await?;
                command::submit(&transport, plan).await
            },
            enabled,
        )
    })();
    // Every setup/lookup/write failure clears, and success clears BEFORE printing.
    if enabled {
        context.print(spinner::CLEAR)?;
    }
    let project = result.context("Failed to update project")?;
    context.print(command::output(project.as_ref()))?;
    Ok(())
}

struct IssueArchiveDeleteAction<'a> {
    target: Option<&'a str>,
    confirm: bool,
    bulk: initiative_bulk::BulkInput<'a>,
}
fn issue_archive_delete_confirm(
    context: &Ctx,
    message: &str,
) -> Result<crate::platform::prompt::PromptOutcome<bool>, Error> {
    use crate::platform::prompt::PromptSession;
    if !context.stdin_tty() {
        return Err(
            Error::new("Interactive confirmation required").with_hint("Use --confirm to skip.")
        );
    }
    if crate::commands::issue_archive_delete::stdout_is_pipe()? {
        return Err(Error::new(
            "Cannot confirm while stdout is a pipe; pass --confirm to continue.",
        ));
    }
    let mut session = PromptSession::confirmation_stdio(context.stdout())?;
    let result = session.confirm(message, false);
    session.finish_result(result)
}
fn dispatch_issue_archive_delete(
    context: &Ctx,
    action: IssueArchiveDeleteAction<'_>,
    mode: crate::commands::issue_archive_delete::Mode,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::commands::issue_archive_delete as command;
    use crate::platform::prompt::PromptOutcome;
    // The client is built before collecting or resolving any IDs.
    let transport = relation_transport(context, workspace)?;
    if action.bulk.requested() {
        if mode == command::Mode::Archive && action.target.is_some() {
            return Err(Error::new("Cannot combine a positional issue ID with --bulk").with_hint("Pass every identifier through --bulk (or --bulk-file / --bulk-stdin), or drop the positional one."));
        }
        let ids = initiative_bulk::collect_ids(&action.bulk, &mut std::io::stdin().lock())?;
        if ids.is_empty() {
            return Err(Error::new(format!(
                "No issue identifiers provided for bulk {}",
                mode.verb()
            )));
        }
        context.print(format!("Found {} issue(s) to {}.\n", ids.len(), mode.verb()).as_bytes())?;
        if !action.confirm {
            let outcome = issue_archive_delete_confirm(
                context,
                &format!(
                    "{} {} issue(s)?",
                    match mode {
                        command::Mode::Archive => "Archive",
                        command::Mode::Delete => "Delete",
                    },
                    ids.len()
                ),
            )?;
            if matches!(outcome, PromptOutcome::Interrupted) {
                return initiative_interrupt_status();
            }
            if !initiative_prompt_stop(outcome)? {
                context.print(format!("Bulk {} cancelled.\n", mode.verb()).as_bytes())?;
                return Ok(());
            }
        }
        let targets = {
            let config = context.config();
            let credentials = context.credentials()?;
            let inputs = client::selection_inputs(&config.options, workspace);
            let scope = WorkspaceScope::from_selection(&inputs, credentials);
            let team = configured_team_key(&config.options);
            ids.into_iter()
                .map(|id| command::Target::prepare(id, team.as_deref(), &scope))
                .collect()
        };
        let progress_enabled = spinner::enabled(false, context.stdout_tty(), true);
        let results = block_on_network(command::execute(&transport, targets, mode, |progress| {
            if progress_enabled {
                context.print(progress.render())?;
            }
            Ok(())
        }))?;
        if progress_enabled {
            context.print(initiative_bulk::PROGRESS_CLEAR)?;
        }
        let (output, failed) = command::summary(&results, mode);
        context.print(&output)?;
        return if failed {
            Err(Error::reported())
        } else {
            Ok(())
        };
    }
    if mode == command::Mode::Delete && action.target.is_none_or(str::is_empty) {
        return Err(Error::new("Issue ID required").with_hint("Use --bulk for multiple issues."));
    }
    let id = resolve_relation_reference(context, action.target, workspace, || match mode {
        command::Mode::Archive => Error::new("Could not determine issue ID")
            .with_hint("Please provide an issue ID like 'ENG-123'."),
        command::Mode::Delete => Error::not_found("Issue", action.target.unwrap_or_default()),
    })?;
    let details = block_on_network(command::single_details(&transport, &id, mode))?;
    if details.already_archived {
        context.print(format!("Issue \"{}\" is already archived.\n", details.name()).as_bytes())?;
        return Ok(());
    }
    if !action.confirm {
        let outcome = issue_archive_delete_confirm(
            context,
            &format!(
                "Are you sure you want to {} \"{}\"?",
                mode.verb(),
                details.name()
            ),
        )?;
        if matches!(outcome, PromptOutcome::Interrupted) {
            return initiative_interrupt_status();
        }
        if !initiative_prompt_stop(outcome)? {
            context.print(
                format!(
                    "{} cancelled.\n",
                    match mode {
                        command::Mode::Archive => "Archive",
                        command::Mode::Delete => "Delete",
                    }
                )
                .as_bytes(),
            )?;
            return Ok(());
        }
    }
    let output = block_on_network(command::submit_single(&transport, &id, &details, mode))?;
    context.print(&output)?;
    Ok(())
}

struct UpdateCreateAction<'a> {
    original: &'a str,
    body: Option<&'a str>,
    file: Option<&'a str>,
    health: Option<&'a str>,
    interactive: bool,
}
fn dispatch_update_create(
    context: &Ctx,
    action: UpdateCreateAction<'_>,
    mode: crate::commands::update_create::Mode,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::commands::update_create as command;
    use crate::platform::prompt::PromptOutcome;
    let result = (|| {
        let interactive = command::attended(
            action.interactive,
            context.stdin_tty(),
            context.stdout_tty(),
            action.body,
            action.file,
            action.health,
        )?;
        // The explicit -i check runs before either client is built.
        let transport = relation_transport(context, workspace)?;
        let (id, display) = {
            let config = context.config();
            let credentials = context.credentials()?;
            let inputs = client::selection_inputs(&config.options, workspace);
            let scope = WorkspaceScope::from_selection(&inputs, credentials);
            match mode {
                command::Mode::Project => {
                    let reference = prepare_project_lookup(action.original, &scope)?;
                    (
                        block_on_network(resolve_project_with_transport(
                            &reference,
                            action.original,
                            &transport,
                        ))?,
                        None,
                    )
                }
                command::Mode::Initiative => {
                    let reference = initiative_view::prepare_reference(action.original, &scope)?;
                    let id = block_on_network(command::initiative_id(
                        &transport,
                        &reference,
                        action.original,
                    ))?;
                    let name = block_on_network(async {
                        Ok(command::initiative_name(&transport, &id, action.original).await)
                    })?;
                    (id, Some(name))
                }
            }
        };
        let env = context.config().child_env.clone();
        let fields = if interactive {
            if let Some(name) = display {
                context.print(format!("\nCreating status update for: {name}\n\n").as_bytes())?;
            }
            let mut session = crate::platform::prompt::PromptSession::stdio(context.stdout())?;
            let prompted = command::prompt(&mut session, &mut std::io::stderr(), &env, mode);
            match session.finish_result(prompted)? {
                PromptOutcome::Submitted(fields) => fields,
                PromptOutcome::Interrupted => return initiative_interrupt_status(),
                PromptOutcome::EndOfInput => {
                    return Err(Error::new(
                        "unexpected EOF while prompting for status update",
                    ));
                }
            }
        } else {
            let body = if let Some(body) = action.body.filter(|value| !value.is_empty()) {
                Some(body.to_owned())
            } else if let Some(path) = action.file.filter(|value| !value.is_empty()) {
                Some(command::file(path, mode, false)?)
            } else if !context.stdin_tty() {
                crate::commands::text_input::read_stdin(std::io::stdin().lock())?
            } else if context.stdout_tty() {
                context.print(format!("{}\n", mode.opening()).as_bytes())?;
                match command::edit(&env, &mut std::io::stderr())? {
                    PromptOutcome::Submitted(body) => {
                        if body.is_none() {
                            context.print(b"No content entered.\n")?;
                        }
                        body
                    }
                    PromptOutcome::Interrupted => return initiative_interrupt_status(),
                    PromptOutcome::EndOfInput => {
                        return Err(Error::new("editor cannot return input EOF"));
                    }
                }
            } else {
                None
            };
            command::Fields {
                body,
                health: command::Health::parse(action.health, mode)?,
            }
        };
        let pending = command::create(&transport, &id, fields, mode);
        let output = if mode == command::Mode::Project && interactive {
            block_on_network(pending)
        } else {
            document_fetch_with_spinner(context, false, pending)
        }?;
        context.print(&output)?;
        Ok(())
    })();
    result.context(mode.context())
}

fn dispatch_initiative_update(
    context: &Ctx,
    action: &cli::initiative::InitiativeUpdate,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::commands::initiative_update as command;
    use crate::platform::prompt::{PromptOutcome, PromptSession, escaped_display};
    let transport = relation_transport(context, workspace)?;
    let reference = {
        let config = context.config();
        let credentials = context.credentials()?;
        let inputs = client::selection_inputs(&config.options, workspace);
        let scope = WorkspaceScope::from_selection(&inputs, credentials);
        initiative_view::prepare_reference(&action.initiative_id, &scope)?
    };
    let id = block_on_network(command::resolve(
        &transport,
        &reference,
        &action.initiative_id,
    ))?;
    let current = block_on_network(command::details(&transport, &id, &action.initiative_id))?;
    let mut fields = command::Fields {
        name: action.name.clone(),
        description: action.description.clone(),
        status: action.status.clone(),
        owner: action.owner.clone(),
        target_date: action.target_date.clone(),
        color: action.color.clone(),
        icon: action.icon.clone(),
    };
    if fields.should_prompt(action.interactive, context.stdout_tty()) {
        context.print(
            format!(
                "\nUpdating initiative: {}\n\n",
                escaped_display(&current.name)
            )
            .as_bytes(),
        )?;
        let mut session = PromptSession::stdio_cr_or_lf(context.stdout())?;
        let prompted = command::prompt(&mut session, &current);
        fields = match session.finish_result(prompted)? {
            PromptOutcome::Submitted(fields) => fields,
            PromptOutcome::Interrupted => return initiative_interrupt_status(),
            PromptOutcome::EndOfInput => {
                return Err(Error::new("unexpected EOF while updating initiative"));
            }
        };
    }
    let owner_id = block_on_network(command::owner(&transport, fields.owner.as_deref()))?;
    if fields.empty() {
        context.print(b"No changes specified\n")?;
        return Ok(());
    }
    let output = document_fetch_with_spinner(
        context,
        false,
        command::submit(&transport, &id, fields.input(owner_id)),
    )?;
    context.print(&output)?;
    Ok(())
}

fn dispatch_issue_comment_update(
    context: &Ctx,
    action: &cli::issue::IssueCommentUpdate,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::{
        commands::issue_comment_update as command,
        platform::prompt::{PromptOutcome, PromptSession},
    };
    let result = (|| {
        let mut body = command::prepare_body(
            &action.comment_id,
            action.body.as_deref(),
            action.body_file.as_deref(),
        )?;
        let transport = relation_transport(context, workspace)?;
        if command::needs_prompt(body.as_deref()) {
            let existing =
                block_on_network(command::existing_body(&transport, &action.comment_id))?;
            if context.stdin_tty() {
                command::check_prompt_topology(true, command::stdout_is_fifo()?)?;
            }
            let mut session = PromptSession::stdin_stdio_cr_or_lf(context.stdout())?;
            let prompted = command::prompt_body(&mut session, &existing);
            body = match session.finish_result(prompted)? {
                PromptOutcome::Submitted(body) => Some(body),
                PromptOutcome::Interrupted => return initiative_interrupt_status(),
                PromptOutcome::EndOfInput => {
                    return Err(Error::new(
                        "comment prompt must convert EOF to its text-specific error",
                    ));
                }
            };
        }
        let body = body.ok_or_else(|| Error::new("comment update body absent after prompt"))?;
        let output = block_on_network(command::submit(&transport, &action.comment_id, body))?;
        context.print(&output)?;
        Ok(())
    })();
    result.context(command::CONTEXT)
}

fn dispatch_config_generate(context: &Ctx, workspace: Option<&str>) -> Result<()> {
    use crate::{
        commands::config_generate as command,
        platform::prompt::{PlainSelect, PromptOutcome, PromptSession},
    };
    let result = (|| {
        context.print(command::BANNER.as_bytes())?;
        // Borrow disjoint startup/stdout fields, not a full-context reference held by the session.
        let loaded = legacy::Loaded::new(context)?;
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
            PromptOutcome::Interrupted => return initiative_interrupt_status(),
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

// These helpers return owned data before prompts or stream output borrow the
// application context.
fn issue_read_transport(
    context: &Ctx,
    workspace: Option<&str>,
) -> Result<crate::graphql::transport::GraphQlTransport, Error> {
    let config = context.config();
    client::prepare_transport(
        &config.options,
        context.credentials()?,
        workspace,
        &config.transport_env,
    )
}
fn issue_read_team(
    context: &Ctx,
    transport: &crate::graphql::transport::GraphQlTransport,
    value: &str,
    workspace: Option<&str>,
) -> Result<crate::refs::ResolvedTeam, Error> {
    let inputs = client::selection_inputs(&context.config().options, workspace);
    let prepared = prepare_team_lookup(
        value,
        &WorkspaceScope::from_selection(&inputs, context.credentials()?),
    )?;
    block_on_network(crate::refs::resolve_team(
        &prepared,
        |request| async move { crate::commands::issue_read::exchange(transport, &request).await },
        |request| async move { crate::commands::issue_read::exchange(transport, &request).await },
    ))
}
fn issue_read_project(
    context: &Ctx,
    transport: &crate::graphql::transport::GraphQlTransport,
    value: Option<&str>,
    workspace: Option<&str>,
) -> Result<Option<String>, Error> {
    use crate::commands::issue_read as command;
    use crate::graphql::{
        envelope::GraphQlRequest,
        operations::issue_read::{GetProjectIdOptionsByName, GetProjectIdOptionsByNameVariables},
    };
    use crate::platform::prompt::{PlainOption, PlainSelect, PromptOutcome, PromptSession};
    use cynic::QueryBuilder;
    let Some(value) = value else { return Ok(None) };
    let inputs = client::selection_inputs(&context.config().options, workspace);
    let reference = prepare_project_lookup(
        value,
        &WorkspaceScope::from_selection(&inputs, context.credentials()?),
    )?;
    if let Some(id) = block_on_network(command::project_id(transport, &reference))? {
        return Ok(Some(id));
    }
    let data: GetProjectIdOptionsByName = block_on_network(command::exchange(
        transport,
        &GraphQlRequest::with_variables(GetProjectIdOptionsByName::build(
            GetProjectIdOptionsByNameVariables {
                name: value.to_owned(),
            },
        )),
    ))?;
    let mut rows = vec![];
    for row in data.projects.nodes {
        if let Some(existing) = rows
            .iter_mut()
            .find(|r: &&mut (String, String)| r.0 == row.id.inner())
        {
            existing.1 = row.name;
        } else {
            rows.push((row.id.into_inner(), row.name));
        }
    }
    if rows.is_empty() {
        return Err(Error::not_found("Project", value));
    }
    if !context.stdin_tty() {
        return Err(Error::new(format!(
            "Project \"{value}\" not found. Similar projects: {}",
            rows.iter()
                .map(|r| r.1.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    let single = rows.len() == 1;
    let message = if single {
        format!(
            "Project named {value} does not exist, but {} exists. Is this what you meant?",
            rows.first()
                .ok_or_else(|| Error::new("Empty project menu"))?
                .1
        )
    } else {
        format!(
            "Project with {value} does not exist, but the following exist. Is any of these what you meant?"
        )
    };
    let mut options = rows
        .iter()
        .enumerate()
        .map(|(index, (_, name))| PlainOption {
            label: if single {
                "yes".to_owned()
            } else {
                name.clone()
            },
            value: index.to_string(),
            script_token: index.to_string(),
        })
        .collect::<Vec<_>>();
    options.push(PlainOption {
        label: if single {
            "no".to_owned()
        } else {
            "none of the above".to_owned()
        },
        value: "none".to_owned(),
        script_token: "none".to_owned(),
    });
    command::project_menu_text(
        &message,
        &options
            .iter()
            .map(|option| option.label.as_str())
            .collect::<Vec<_>>(),
    )?;
    let mut session = PromptSession::stdin_stdio(context.stdout())?;
    let result = session.select(&PlainSelect {
        message: &message,
        options: &options,
        default_index: 0,
        default_hint: None,
    })?;
    let result = session.finish(result)?;
    match result {
        PromptOutcome::Submitted(selected) if selected == "none" => Ok(None),
        PromptOutcome::Submitted(selected) => selected
            .parse::<usize>()
            .ok()
            .and_then(|index| rows.get(index))
            .map(|r| Some(r.0.clone()))
            .ok_or_else(|| Error::new("Project menu returned unknown selection")),
        PromptOutcome::Interrupted => Err(Error::cancelled()),
        PromptOutcome::EndOfInput => Err(Error::new("unexpected EOF while selecting project")),
    }
}
fn issue_read_cycle(
    context: &Ctx,
    transport: &crate::graphql::transport::GraphQlTransport,
    value: Option<&str>,
    team_key: Option<&str>,
    team_id: Option<&str>,
    workspace: Option<&str>,
) -> Result<Option<String>, Error> {
    let Some(value) = value else { return Ok(None) };
    let inputs = client::selection_inputs(&context.config().options, workspace);
    let id = match team_id {
        Some(id) => id.to_owned(),
        None => {
            issue_read_team(
                context,
                transport,
                team_key.ok_or_else(|| Error::new("--cycle requires a single team scope"))?,
                workspace,
            )?
            .id
        }
    };
    let url = crate::refs::expect_url_kind(
        value,
        crate::refs::LinearUrlKind::Cycle,
        "a cycle URL, number, or name",
        &WorkspaceScope::from_selection(&inputs, context.credentials()?),
    )?;
    block_on_network(cycle_view::resolve_id_with(
        &id,
        value,
        url.as_ref(),
        |request| async move { crate::commands::issue_read::exchange(transport, &request).await },
    ))
    .map(Some)
}
fn issue_read_sort(context: &Ctx, sort: Option<cli::Sort>) -> Result<bool, Error> {
    use crate::config::IssueSort;
    let value = sort.map(|v| match v {
        cli::Sort::Manual => IssueSort::Manual,
        cli::Sort::Priority => IssueSort::Priority,
    });
    Ok(context.config().options.issue_sort(value).0 == IssueSort::Priority)
}
fn issue_read_output(context: &Ctx, output: &str, pager_enabled: bool) -> Result<()> {
    if !context.stdout_tty() {
        context.print(format!("{output}\n").as_bytes())?;
        return Ok(());
    }
    context.page(output, pager_enabled)
}
fn issue_read_project_conflict(project: Option<&str>, label: Option<&str>) -> Result<(), Error> {
    if project.is_some() && label.is_some() {
        return Err(Error::new("Cannot use --project and --project-label together").with_hint("Use --project to filter by a single project, or --project-label to filter by all projects with a given label."));
    }
    Ok(())
}
fn issue_read_milestone_conflict(
    milestone: Option<&str>,
    project_label: Option<&str>,
) -> Result<(), Error> {
    if milestone.is_some() && project_label.is_some() {
        return Err(
            Error::new("--milestone cannot be used with --project-label").with_hint(
                "Use --project to specify a single project when filtering by milestone.",
            ),
        );
    }
    Ok(())
}
fn dispatch_issue_mine(
    context: &Ctx,
    action: &cli::issue::IssueMine,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::commands::issue_read as command;
    if action.web || action.app {
        let Some(team) = configured_team_key(&context.config().options) else {
            context
                .eprint(b"Could not determine team id from configuration or directory name.\n")?;
            return Err(Error::reported());
        };
        let Some(workspace) = workspace.or(context
            .config()
            .options
            .workspace()
            .map(|v| v.value().as_str())
            .filter(|v| !v.is_empty()))
        else {
            context.eprint(
                b"workspace is not set via command line, configuration file, or environment.\n",
            )?;
            return Err(Error::reported());
        };
        let filter = "eyJhbmQiOlt7ImFzc2lnbmVlIjp7Im9yIjpbeyJpc01lIjp7ImVxIjp0cnVlfX1dfX1dfQ";
        crate::platform::opener::open(
            &format!("https://linear.app/{workspace}/team/{team}/active?filter={filter}"),
            action.app,
        )?;
        return Ok(());
    }
    let result = (|| {
        if action.assignee.is_some() || action.all_assignees || action.unassigned {
            let flag = if action.assignee.is_some() {
                "--assignee"
            } else if action.all_assignees {
                "--all-assignees"
            } else {
                "--unassigned"
            };
            return Err(
                Error::new(format!("{flag} has been removed from 'issue mine'")).with_hint(
                    format!("Use 'linear issue query {flag}' for assignee filtering."),
                ),
            );
        }
        if action.all_states
            && (action.state.len() != 1
                || action.state.first().map(String::as_str) != Some("unstarted"))
        {
            return Err(Error::new("Cannot use --all-states with --state flag"));
        }
        let priority = issue_read_sort(context, action.sort)?;
        if action.team.is_none() && configured_team_key(&context.config().options).is_none() {
            let inside = std::process::Command::new("git")
                .args(["rev-parse", "--is-inside-work-tree"])
                .current_dir(context.cwd())
                .stdin(std::process::Stdio::null())
                .envs(context.config().child_env.iter())
                .output()
                .is_ok_and(|o| o.status.success());
            return Err(Error::new("No default team configured and no team scope provided").with_hint(if inside { "Use --team <key, name, or ID> to specify a team, or run `linear config` to link this repository to a team." } else { "Use --team <key, name, or ID> to specify a team." }));
        }
        let transport = issue_read_transport(context, workspace)?;
        let explicit = action
            .team
            .as_deref()
            .map(|team| issue_read_team(context, &transport, team, workspace))
            .transpose()?;
        let team = explicit
            .as_ref()
            .map(|t| t.key.clone())
            .or_else(|| configured_team_key(&context.config().options));
        let team = team.ok_or_else(|| Error::new("Validated issue team scope is absent"))?;
        issue_read_project_conflict(action.project.as_deref(), action.project_label.as_deref())?;
        let project =
            issue_read_project(context, &transport, action.project.as_deref(), workspace)?;
        let cycle = issue_read_cycle(
            context,
            &transport,
            action.cycle.as_deref(),
            Some(&team),
            explicit.as_ref().map(|t| t.id.as_str()),
            workspace,
        )?;
        issue_read_milestone_conflict(
            action.milestone.as_deref(),
            action.project_label.as_deref(),
        )?;
        if action
            .milestone
            .as_deref()
            .is_some_and(|m| !crate::refs::is_linear_uuid(m))
            && project.is_none()
        {
            return Err(Error::new("--milestone requires --project to be set").with_hint("Use --project to specify which project the milestone belongs to, or pass a milestone UUID directly."));
        }
        let milestone = action
            .milestone
            .as_deref()
            .map(|m| block_on_network(command::milestone_id(&transport, m, project.as_deref())))
            .transpose()?;
        let rows = agent_session_network(context, false, async {
            let mut filter = crate::graphql::operations::issue_read::IssueFilter {
                team: Some(command::team_filter(std::slice::from_ref(&team), true)),
                state: if action.all_states {
                    None
                } else {
                    command::state_filter(
                        &transport,
                        &action.state,
                        Some(std::slice::from_ref(&team)),
                    )
                    .await?
                },
                assignee: command::assignee_filter(&transport, None, false, true).await?,
                ..Default::default()
            };
            command::entity_filters(
                &mut filter,
                project,
                action.project_label.as_deref(),
                cycle,
                milestone,
                &action.label,
            );
            command::apply_dates(
                &mut filter,
                action.created_after.as_deref(),
                action.updated_after.as_deref(),
            )?;
            command::mine(&transport, filter, priority, action.limit.0).await
        })?;
        let table = command::table(
            &rows
                .into_iter()
                .map(command::TableRow::from)
                .collect::<Vec<_>>(),
            true,
            false,
            false,
            if context.stdout_tty() {
                crate::platform::pager::stdout_size().map_or(80, |s| usize::from(s.columns))
            } else {
                120
            },
            context.color(),
            std::time::SystemTime::now(),
        )?;
        issue_read_output(context, &table, !action.no_pager)
    })();
    result.context("Failed to list issues")
}
fn dispatch_issue_query(
    context: &Ctx,
    action: &cli::issue::IssueQuery,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::commands::issue_read as command;
    use crate::graphql::operations::issue_read::IssueFilter;
    let result = (|| {
        let err = |m: &str| Error::new(m);
        if !action.team.is_empty() && action.all_teams {
            return Err(err("Cannot use both --team and --all-teams flags"));
        }
        if usize::from(action.assignee.is_some())
            + usize::from(action.all_assignees)
            + usize::from(action.unassigned)
            > 1
        {
            return Err(err(
                "Cannot specify multiple assignee filters (--assignee, --all-assignees, --unassigned)",
            ));
        }
        if action.all_states && !action.state.is_empty() {
            return Err(err("Cannot use --all-states with --state flag"));
        }
        issue_read_project_conflict(action.project.as_deref(), action.project_label.as_deref())?;
        if action
            .milestone
            .as_deref()
            .is_some_and(|m| !crate::refs::is_linear_uuid(m))
            && action.project.is_none()
        {
            return Err(err("--milestone requires --project to be set").with_hint("Use --project to specify which project the milestone belongs to, or pass a milestone UUID directly."));
        }
        issue_read_milestone_conflict(
            action.milestone.as_deref(),
            action.project_label.as_deref(),
        )?;
        if action.search_comments && action.search.is_none() {
            return Err(err("--search-comments requires --search to be set").with_hint("Use --search to provide a search term, e.g. --search \"oauth timeout\" --search-comments."));
        }
        if action.sort.is_some() && action.search.is_some() {
            return Err(err("--sort cannot be used with --search").with_hint(
                "Search results use relevance ordering. Remove --sort when using --search.",
            ));
        }
        let default_team = if !action.all_teams && action.team.is_empty() {
            let team = configured_team_key(&context.config().options).ok_or_else(||err("No default team configured and no team scope provided").with_hint("Use --team <key, name, or ID> to specify a team, or --all-teams to query the whole workspace."))?;
            if context.config().options.team_id().is_some_and(|v| {
                matches!(
                    v.source(),
                    crate::config::OptionSource::Env
                        | crate::config::OptionSource::GlobalConfig { .. }
                )
            }) {
                context.eprint(format!("Note: using default team {team}. Pass --team <key, name, or ID> or --all-teams to be explicit.\n").as_bytes())?;
            }
            Some(team)
        } else {
            None
        };
        let transport = issue_read_transport(context, workspace)?;
        let (keys, multi, explicit_id) = if action.all_teams {
            (None, true, None)
        } else if !action.team.is_empty() {
            let inputs = client::selection_inputs(&context.config().options, workspace);
            let scope = WorkspaceScope::from_selection(&inputs, context.credentials()?);
            let prepared = action
                .team
                .iter()
                .map(|t| prepare_team_lookup(t, &scope))
                .collect::<Result<Vec<_>, _>>()?;
            let resolved = block_on_network(async {
                futures_util::future::try_join_all(prepared.iter().map(|p| {
                    let transport = &transport;
                    crate::refs::resolve_team(
                        p,
                        move |r| async move { command::exchange(transport, &r).await },
                        move |r| async move { command::exchange(transport, &r).await },
                    )
                }))
                .await
            })?;
            let mut rows = vec![];
            for r in resolved {
                if !rows
                    .iter()
                    .any(|t: &crate::refs::ResolvedTeam| t.id == r.id)
                {
                    rows.push(r);
                }
            }
            let id = if rows.len() == 1 {
                rows.first().map(|t| t.id.clone())
            } else {
                None
            };
            (
                Some(rows.iter().map(|t| t.key.clone()).collect::<Vec<_>>()),
                rows.len() > 1,
                id,
            )
        } else {
            let team =
                default_team.ok_or_else(|| Error::new("default issue team was not prepared"))?;
            (Some(vec![team]), false, None)
        };
        let state = block_on_network(command::state_filter(
            &transport,
            &action.state,
            keys.as_deref(),
        ))?;
        let project =
            issue_read_project(context, &transport, action.project.as_deref(), workspace)?;
        if action.cycle.is_some() && (multi || keys.as_ref().is_none_or(|k| k.len() != 1)) {
            return Err(err("--cycle requires a single team scope").with_hint("Use --team <key, name, or ID> to specify exactly one team when filtering by cycle."));
        }
        let cycle = issue_read_cycle(
            context,
            &transport,
            action.cycle.as_deref(),
            keys.as_ref().and_then(|k| k.first()).map(String::as_str),
            explicit_id.as_deref(),
            workspace,
        )?;
        let milestone = action
            .milestone
            .as_deref()
            .map(|m| block_on_network(command::milestone_id(&transport, m, project.as_deref())))
            .transpose()?;
        let priority = if action.search.is_none() {
            issue_read_sort(context, action.sort).map(Some)
        } else {
            Ok(None)
        };
        let columns = if context.stdout_tty() {
            crate::platform::pager::stdout_size().map_or(80, |s| usize::from(s.columns))
        } else {
            120
        };
        let color = context.color();
        let output = agent_session_network(context, action.json, async {
            let priority = priority?;
            let term = action.search.as_deref().map(|s| s.trim().to_owned());
            if term.as_ref().is_some_and(String::is_empty) {
                return Err(err("--search term cannot be empty"));
            }
            let mut filter = IssueFilter {
                team: keys.as_deref().map(command::query_team_filter),
                state,
                assignee: command::assignee_filter(
                    &transport,
                    action.assignee.as_deref(),
                    action.unassigned,
                    false,
                )
                .await?,
                ..Default::default()
            };
            command::entity_filters(
                &mut filter,
                project,
                action.project_label.as_deref(),
                cycle,
                if term.is_some() { None } else { milestone },
                &action.label,
            );
            command::apply_dates(
                &mut filter,
                action.created_after.as_deref(),
                action.updated_after.as_deref(),
            )?;
            let filter = if serde_json::to_value(&filter)
                .map_err(|e| Error::new("could not inspect typed filter").with_source(e))?
                .as_object()
                .is_some_and(|m| m.is_empty())
            {
                None
            } else {
                Some(filter)
            };
            if let Some(term) = term {
                let data = command::search(
                    &transport,
                    filter,
                    term,
                    action.limit.0,
                    action.include_archived,
                    action.search_comments,
                )
                .await?;
                if action.json {
                    issue_read_json(&data)
                } else {
                    command::table(
                        &data
                            .nodes
                            .into_iter()
                            .map(command::TableRow::from)
                            .collect::<Vec<_>>(),
                        false,
                        multi,
                        action.assignee.is_none() && !action.unassigned,
                        columns,
                        color,
                        std::time::SystemTime::now(),
                    )
                }
            } else {
                let data = command::query(
                    &transport,
                    filter,
                    priority.ok_or_else(|| err("Missing issue sort"))?,
                    action.limit.0,
                    action.include_archived,
                )
                .await?;
                if action.json {
                    issue_read_json(&data)
                } else {
                    command::table(
                        &data
                            .nodes
                            .into_iter()
                            .map(command::TableRow::from)
                            .collect::<Vec<_>>(),
                        false,
                        multi,
                        action.assignee.is_none() && !action.unassigned,
                        columns,
                        color,
                        std::time::SystemTime::now(),
                    )
                }
            }
        })?;
        if action.json {
            context.print(format!("{output}\n").as_bytes())?;
            Ok(())
        } else {
            issue_read_output(context, &output, !action.no_pager)
        }
    })();
    result.context("Failed to query issues")
}
fn issue_read_json(value: &impl serde::Serialize) -> Result<String, Error> {
    serde_json::to_string_pretty(value)
        .map_err(|e| Error::new("could not serialize issue output").with_source(e))
}
fn dispatch_issue_view(
    context: &Ctx,
    action: &cli::issue::IssueView,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::commands::issue_view as command;
    use crate::platform::{markdown_assets, markdown_terminal, pager};
    if action.web || action.app {
        let id = match resolve_issue(context, action.issue_id.as_deref(), workspace) {
            Ok(id) => id,
            Err(e) if e.message() == "Could not determine issue ID" => {
                let message = match context
                    .config()
                    .options
                    .vcs()
                    .map(|v| *v.value())
                    .unwrap_or(crate::config::Vcs::Git)
                {
                    crate::config::Vcs::Git => {
                        "The current branch does not contain a valid linear issue id.\n"
                    }
                    crate::config::Vcs::Jj => {
                        "No Linear-issue trailer found in current or ancestor commits.\n"
                    }
                };
                context.eprint(message.as_bytes())?;
                return Err(Error::reported());
            }
            Err(e) => return Err(e),
        };
        let configured = workspace
            .or_else(|| {
                context
                    .config()
                    .options
                    .workspace()
                    .map(|v| v.value().as_str())
            })
            .filter(|s| !s.is_empty());
        let Some(workspace) = configured else {
            context.eprint(
                b"workspace is not set via command line, configuration file, or environment.\n",
            )?;
            return Err(Error::reported());
        };
        let url = format!("https://linear.app/{workspace}/issue/{id}");
        context.print(
            format!(
                "Opening {url} in {}\n",
                if action.app {
                    "Linear.app"
                } else {
                    "web browser"
                }
            )
            .as_bytes(),
        )?;
        crate::platform::opener::open(&url, action.app)?;
        return Ok(());
    }
    let result = (|| {
        let id = resolve_issue(context, action.issue_id.as_deref(), workspace)?;
        let transport = issue_read_transport(context, workspace)?;
        let fetched = agent_session_network(
            context,
            action.json,
            command::fetch(&transport, id, !action.no_comments),
        )?;
        if action.json {
            context.print(format!("{}\n", fetched.json()?).as_bytes())?;
            return Ok(());
        }
        let config = context.config();
        let download =
            !action.no_download && config.options.download_images().is_none_or(|v| *v.value());
        let attachments = download
            && config
                .options
                .auto_download_attachments()
                .is_none_or(|v| *v.value());
        let image_root = config.image_cache_root.clone();
        let attachment_root = config
            .options
            .attachment_dir()
            .map(|v| v.value().clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                markdown_assets::posix_join(&[
                    image_root
                        .parent()
                        .and_then(|p| p.to_str())
                        .unwrap_or("/tmp"),
                    "linear-cli-attachments",
                ])
            });
        let hyperlink = config.options.hyperlink_format().map(|v| v.value().clone());
        let mut issue = fetched.into_issue();
        if download {
            block_on_network(command::download_images(
                &transport,
                &mut issue,
                &image_root,
                |bytes| context.eprint(bytes),
            ))?;
        }
        let paths = if attachments {
            block_on_network(command::download_attachments(
                &transport,
                &issue,
                &attachment_root,
                |bytes| context.eprint(bytes),
            ))?
        } else {
            std::collections::HashMap::new()
        };
        let output = if context.stdout_tty() {
            let columns = pager::stdout_size()
                .and_then(|s| std::num::NonZeroU16::new(s.columns))
                .unwrap_or(markdown_terminal::FALLBACK_COLUMNS);
            let options = markdown_terminal::RenderOptions::for_terminal(
                columns,
                context.color(),
                hyperlink.as_deref(),
                markdown_terminal::HostSource::System,
            );
            command::terminal(
                &issue,
                &paths,
                action.show_resolved_threads,
                chrono::Utc::now(),
                &options,
                true,
            )?
        } else {
            command::markdown(
                &issue,
                &paths,
                action.show_resolved_threads,
                chrono::Utc::now(),
            )?
        };
        issue_read_output(context, &output, !action.no_pager)
    })();
    result.context("Failed to view issue")
}

fn auth_prompt_eof(phase: &str) -> Error {
    Error::new(format!("unexpected EOF while prompting for {phase}"))
}
fn auth_prompt_output<R: std::io::Read, W: std::io::Write>(
    session: &mut Option<crate::platform::prompt::PromptSession<R, W>>,
    output: &mut Option<W>,
    bytes: &[u8],
) -> Result<(), Error> {
    match session {
        Some(current) => {
            let text = std::str::from_utf8(bytes)
                .map_err(|error| Error::new("auth output must be UTF-8").with_source(error))?;
            for line in text.split_terminator('\n') {
                current.print_line(line)?;
            }
            Ok(())
        }
        None => {
            let writer = output
                .as_mut()
                .ok_or_else(|| Error::new("auth output missing"))?;
            writer
                .write_all(bytes)
                .and_then(|()| writer.flush())
                .map_err(output::write_error)
        }
    }
}
fn dispatch_auth_login(context: &Ctx, action: &cli::auth::AuthLogin) -> Result<()> {
    use crate::{
        auth::{
            keyring::NativeMutationBackend,
            mutation::{CredentialMutationState, RealCredentialMutationFileWriter},
        },
        commands::auth_login as command,
        platform::prompt::{PromptOutcome, PromptSession},
    };
    let no_color = !context.color();
    let loaded = legacy::Loaded::new(context)?;
    let mut state = CredentialMutationState::from_store(loaded.credentials);
    let config = &loaded.config;
    let path = loaded.credentials_path.as_deref();
    let backend = NativeMutationBackend {
        overlay: config.child_env.clone(),
    };
    let writer = RealCredentialMutationFileWriter;
    let mut session = None;
    let mut output = Some(context.stdout());
    let result = (|| {
        let key = match command::supplied_key(action.key.as_deref()) {
            Some(key) => key,
            None => {
                session = Some(PromptSession::stdin_stdio_cr_or_lf(
                    output
                        .take()
                        .ok_or_else(|| Error::new("auth output already owned"))?,
                )?);
                let current = session
                    .as_mut()
                    .ok_or_else(|| Error::new("secret session absent"))?;
                let key = match current.secret(command::SECRET_MESSAGE, command::SECRET_HINT)? {
                    PromptOutcome::Submitted(key) => key,
                    PromptOutcome::Interrupted => return initiative_interrupt_status(),
                    PromptOutcome::EndOfInput => return Err(auth_prompt_eof("API key")),
                };
                current.suspend()?;
                key
            }
        };
        let key = command::clean_key(key)?;
        let transport = command::prepare_transport(&config.options, &config.transport_env, &key)?;
        let viewer = block_on_network(async {
            command::authenticate(&transport)
                .await
                .map_err(|failure| failure.login())
        })?;
        let bytes = block_on_network(async {
            command::add_authenticated(
                &mut state,
                viewer,
                key,
                command::LoginSaveOptions {
                    plaintext: action.plaintext,
                    no_color,
                },
                path,
                &backend,
                &writer,
            )
            .await
            .map_err(|failure| failure.login())
        })?;
        auth_prompt_output(&mut session, &mut output, &bytes)?;
        let offer =
            block_on_network(async { Ok(command::offer_migration(&state, &backend).await) })?;
        if offer {
            auth_prompt_output(
                &mut session,
                &mut output,
                &command::migration_notice(no_color),
            )?;
            match session.as_mut() {
                Some(current) => current.resume()?,
                None => {
                    session = Some(PromptSession::stdin_stdio_cr_or_lf(
                        output
                            .take()
                            .ok_or_else(|| Error::new("migration output absent"))?,
                    )?);
                }
            }
            let current = session
                .as_mut()
                .ok_or_else(|| Error::new("migration session absent"))?;
            let migrate = match current.confirm(command::MIGRATE_MESSAGE, true)? {
                PromptOutcome::Submitted(answer) => answer,
                PromptOutcome::Interrupted => return initiative_interrupt_status(),
                PromptOutcome::EndOfInput => return Err(auth_prompt_eof("credential migration")),
            };
            current.suspend()?;
            if migrate {
                let bytes = block_on_network(async {
                    command::migrate(&mut state, path, &backend, &writer)
                        .await
                        .map_err(|failure| failure.login())
                })?;
                auth_prompt_output(&mut session, &mut output, &bytes)?;
            }
        }
        auth_prompt_output(
            &mut session,
            &mut output,
            &command::environment_warning(&config.options, no_color)?,
        )?;
        Ok(())
    })();
    let cleanup = session.as_mut().map_or(Ok(()), PromptSession::close);
    cleanup.and(result).context(command::CONTEXT)
}
fn dispatch_auth_logout(context: &Ctx, action: &cli::auth::AuthLogout) -> Result<()> {
    use crate::{
        auth::{
            keyring::NativeMutationBackend,
            mutation::{CredentialMutationState, RealCredentialMutationFileWriter},
        },
        commands::auth_logout as command,
        platform::prompt::{PlainSelect, PromptOutcome, PromptSession},
    };
    let loaded = legacy::Loaded::new(context)?;
    let mut state = CredentialMutationState::from_store(loaded.credentials);
    let backend = NativeMutationBackend {
        overlay: loaded.config.child_env.clone(),
    };
    let writer = RealCredentialMutationFileWriter;
    let path = loaded.credentials_path.as_deref();
    let mut session = None;
    let mut output = Some(context.stdout());
    let result = (|| {
        let target = command::prepare(&state, action.workspace_name.as_deref())?;
        let name = match target {
            command::LogoutTarget::Selected(name) => name,
            command::LogoutTarget::Select(options) => {
                session = Some(PromptSession::stdin_stdio_cr_or_lf(
                    output
                        .take()
                        .ok_or_else(|| Error::new("logout output absent"))?,
                )?);
                let current = session
                    .as_mut()
                    .ok_or_else(|| Error::new("logout session absent"))?;
                match current.select(&PlainSelect {
                    message: command::SELECT_MESSAGE,
                    options: &options,
                    default_index: 0,
                    default_hint: None,
                })? {
                    PromptOutcome::Submitted(name) => name,
                    PromptOutcome::Interrupted => return initiative_interrupt_status(),
                    PromptOutcome::EndOfInput => return Err(auth_prompt_eof("workspace")),
                }
            }
        };
        if !action.force {
            if session.is_none() {
                session = Some(PromptSession::stdin_stdio_cr_or_lf(
                    output
                        .take()
                        .ok_or_else(|| Error::new("logout confirmation output absent"))?,
                )?)
            }
            let current = session
                .as_mut()
                .ok_or_else(|| Error::new("logout confirmation absent"))?;
            match current.confirm(&command::confirm_message(&name), false)? {
                PromptOutcome::Submitted(true) => {}
                PromptOutcome::Submitted(false) => {
                    auth_prompt_output(&mut session, &mut output, b"Cancelled\n")?;
                    return Ok(());
                }
                PromptOutcome::Interrupted => return initiative_interrupt_status(),
                PromptOutcome::EndOfInput => return Err(auth_prompt_eof("logout confirmation")),
            }
        }
        if let Some(current) = session.as_mut() {
            current.suspend()?
        }
        let bytes = block_on_network(async {
            command::remove(&mut state, &name, path, &backend, &writer)
                .await
                .map_err(|failure| failure.outer())
        })?;
        auth_prompt_output(&mut session, &mut output, &bytes)?;
        Ok(())
    })();
    let cleanup = session.as_mut().map_or(Ok(()), PromptSession::close);
    cleanup.and(result).context(command::CONTEXT)
}
fn dispatch_auth_migrate(context: &Ctx) -> Result<()> {
    use crate::{
        auth::{
            keyring::NativeMutationBackend,
            mutation::{CredentialMutationState, RealCredentialMutationFileWriter},
        },
        commands::auth_migrate as command,
    };
    let loaded = legacy::Loaded::new(context)?;
    let mut state = CredentialMutationState::from_store(loaded.credentials);
    let backend = NativeMutationBackend {
        overlay: loaded.config.child_env.clone(),
    };
    let writer = RealCredentialMutationFileWriter;
    let bytes = block_on_network(async {
        command::run(
            &mut state,
            loaded.credentials_path.as_deref(),
            &backend,
            &writer,
        )
        .await
        .map_err(|failure| failure.outer())
    })
    .context(command::CONTEXT)?;
    context.print(&bytes)?;
    Ok(())
}

// Resolve the issue from the argument or the VCS, running jj/git with explicit stdin and environment.
fn resolve_script_issue(
    context: &Ctx,
    input: Option<&str>,
    workspace: Option<&str>,
    runner: &mut impl crate::platform::vcs_script::ProcessRunner,
) -> Result<String, Error> {
    let config = context.config();
    let reference = match input {
        None => crate::refs::IssueReference::Inferred,
        Some(_) => {
            let inputs = client::selection_inputs(&config.options, workspace);
            let team = configured_team_key(&config.options);
            crate::refs::prepare_issue_reference(
                input,
                team.as_deref(),
                &WorkspaceScope::from_selection(&inputs, context.credentials()?),
            )?
        }
    };
    let identifier = match reference {
        crate::refs::IssueReference::Identifier(identifier) => Some(identifier),
        crate::refs::IssueReference::Unresolved => None,
        crate::refs::IssueReference::Inferred => crate::platform::vcs_script::infer_issue(
            runner,
            config
                .options
                .vcs()
                .map(|value| *value.value())
                .unwrap_or(crate::config::Vcs::Git),
            context.cwd(),
            &config.child_env,
        )?,
    };
    identifier.ok_or_else(|| issue_details::unresolved(false))
}
fn dispatch_issue_commits(
    context: &Ctx,
    action: &cli::issue::IssueCommits,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::{commands::issue_commits as command, platform::vcs_script::NativeProcessRunner};
    let result = (|| {
        let config = context.config();
        command::check_vcs(
            config
                .options
                .vcs()
                .map(|value| *value.value())
                .unwrap_or(crate::config::Vcs::Git),
        )?;
        let mut runner = NativeProcessRunner;
        // Gate then inference; missing key must NOT preempt the inference child.
        let identifier =
            resolve_script_issue(context, action.issue_id.as_deref(), workspace, &mut runner)?;
        let config = context.config();
        let transport = client::prepare_transport(
            &config.options,
            context.credentials()?,
            workspace,
            &config.transport_env,
        )?;
        block_on_network(command::lookup(&transport, &identifier))?;
        // The final child writes actual inherited descriptors, not AppContext.
        command::show(&mut runner, &identifier, context.cwd(), &config.child_env)
    })();
    result.context(command::CONTEXT)
}
fn dispatch_issue_describe(
    context: &Ctx,
    action: &cli::issue::IssueDescribe,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::{commands::issue_describe as command, platform::vcs_script::NativeProcessRunner};
    let result: Result<()> = (|| {
        let identifier = resolve_script_issue(
            context,
            action.issue_id.as_deref(),
            workspace,
            &mut NativeProcessRunner,
        )?;
        let enabled = spinner::enabled(false, context.stdout_tty(), true);
        if enabled {
            context.print(spinner::frame(0).as_bytes())?;
        }
        // The spinner starts before the client is built, so it also covers a missing key.
        let fetched = (|| {
            let transport = relation_transport(context, workspace)?;
            project_ticks(context, command::fetch(&transport, &identifier), enabled)
        })();
        if enabled {
            context.print(spinner::CLEAR)?;
        }
        let detail = fetched?;
        let bytes = command::format(&identifier, &detail.title, &detail.url, action.references);
        context.print(&bytes)?;
        Ok(())
    })();
    result.context(command::CONTEXT)
}

// Anchored addition to app.rs; no old app whole-file replacement.
// Prompt session owns stdin/buffer across the picker, details read and branch menu.
fn start_snapshot(
    context: &Ctx,
) -> Result<(&crate::config::StartupConfig, &crate::auth::CredentialStore)> {
    Ok((context.config(), context.credentials()?))
}
struct StartPromptOutput<'a> {
    plain: Option<StdoutWriter<'a>>,
    session: Option<crate::platform::prompt::PromptSession<std::io::Stdin, StdoutWriter<'a>>>,
}
impl<'a> StartPromptOutput<'a> {
    fn new(writer: StdoutWriter<'a>) -> Self {
        Self {
            plain: Some(writer),
            session: None,
        }
    }
    fn writer(&mut self) -> Result<&mut StdoutWriter<'a>, Error> {
        match self.session.as_mut() {
            Some(session) => session.suspended_output(),
            None => self
                .plain
                .as_mut()
                .ok_or_else(|| Error::new("start output has no owner")),
        }
    }
    fn prompt(
        &mut self,
    ) -> Result<&mut crate::platform::prompt::PromptSession<std::io::Stdin, StdoutWriter<'a>>, Error>
    {
        match self.session.as_mut() {
            Some(session) => session.resume()?,
            None => {
                let writer = self
                    .plain
                    .take()
                    .ok_or_else(|| Error::new("start prompt output was already moved"))?;
                self.session =
                    Some(crate::platform::prompt::PromptSession::stdin_stdio_cr_or_lf(writer)?);
            }
        }
        self.session
            .as_mut()
            .ok_or_else(|| Error::new("start prompt construction vanished"))
    }
    fn suspend(&mut self) -> Result<(), Error> {
        self.session
            .as_mut()
            .ok_or_else(|| Error::new("start prompt absent"))?
            .suspend()
    }
    fn close(&mut self) -> Result<(), Error> {
        if let Some(session) = self.session.take() {
            self.plain = Some(session.into_output()?);
        }
        Ok(())
    }
}
fn script_status(writer: &mut dyn std::io::Write, bytes: &[u8]) -> Result<(), Error> {
    writer
        .write_all(bytes)
        .and_then(|()| writer.flush())
        .map_err(output::write_error)
}
fn start_details(
    writer: &mut dyn std::io::Write,
    config: &crate::config::StartupConfig,
    credentials: &crate::auth::CredentialStore,
    workspace: Option<&str>,
    identifier: &str,
    spin: bool,
) -> Result<crate::graphql::operations::issue_details::IssueDetails, Error> {
    if spin {
        script_status(writer, spinner::frame(0).as_bytes())?;
    }
    let fetched = (|| {
        // The spinner starts before the client is built on each details read.
        let transport = client::prepare_transport(
            &config.options,
            credentials,
            workspace,
            &config.transport_env,
        )?;
        block_on_network(async {
            let pending = crate::commands::issue_describe::fetch(&transport, identifier);
            if !spin {
                return pending.await;
            }
            tokio::pin!(pending);
            let mut ticks = tokio::time::interval(spinner::TICK_INTERVAL);
            ticks.tick().await;
            let mut frame = 1_usize;
            loop {
                tokio::select! { biased;
                    result = &mut pending => break result,
                    _ = ticks.tick() => {
                        script_status(writer, spinner::frame(frame).as_bytes())?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        })
    })();
    if spin {
        script_status(writer, spinner::CLEAR)?;
    }
    fetched
}
/// Composed post-create/start entry: the already resolved ID is opaque.
#[allow(clippy::too_many_arguments)]
fn start_work_on_created_issue(
    stderr: &mut dyn std::io::Write,
    config: &crate::config::StartupConfig,
    credentials: &crate::auth::CredentialStore,
    workspace: Option<&str>,
    identifier: &str,
    team_reference: &str,
    output: &mut StartPromptOutput<'_>,
    cwd: &std::path::Path,
    spin: bool,
    stdin_tty: bool,
    branch_override: Option<&str>,
    from_ref: Option<&str>,
) -> Result<()> {
    use crate::{
        commands::issue_start as command,
        platform::{prompt::PromptOutcome, vcs_script::NativeProcessRunner},
    };
    let details = start_details(
        output.writer()?,
        config,
        credentials,
        workspace,
        identifier,
        spin,
    )?;
    let mut runner = NativeProcessRunner;
    match config
        .options
        .vcs()
        .map(|value| *value.value())
        .unwrap_or(crate::config::Vcs::Git)
    {
        crate::config::Vcs::Git => {
            let branch = command::branch_name(branch_override, &details.branch_name);
            let exists = command::verify(&mut runner, branch, cwd, &config.child_env)?;
            let choice = if exists {
                let choices = command::branch_options();
                let message = format!("Branch {branch} already exists. What would you like to do?");
                // Escape display controls only; raw branch remains semantic argv.
                let message = crate::platform::prompt::escaped_display(&message);
                if stdin_tty {
                    command::check_prompt_topology(true, command::stdout_is_fifo()?)?;
                }
                let answer = output
                    .prompt()?
                    .select(&command::branch_menu(&message, &choices))?;
                let answer = command::stage(answer, "existing branch action")?;
                output.suspend()?;
                match answer {
                    PromptOutcome::Submitted(value) => Some(command::existing_branch(&value)?),
                    PromptOutcome::Interrupted => return initiative_interrupt_status(),
                    PromptOutcome::EndOfInput => {
                        return Err(Error::new("branch EOF conversion absent"));
                    }
                }
            } else {
                None
            };
            output.close()?;
            let bytes = match choice {
                Some(choice) => command::existing_git(
                    &mut runner,
                    choice,
                    branch,
                    from_ref,
                    cwd,
                    &config.child_env,
                )?,
                None => {
                    command::create_branch(&mut runner, branch, from_ref, cwd, &config.child_env)?
                }
            };
            script_status(output.writer()?, &bytes)?;
        }
        crate::config::Vcs::Jj => {
            output.close()?;
            command::prepare_jj(&mut runner, cwd, &config.child_env, &mut *stderr)?;
            let second = start_details(
                output.writer()?,
                config,
                credentials,
                workspace,
                identifier,
                spin,
            )?;
            let bytes = command::describe_jj(
                &mut runner,
                identifier,
                &second.title,
                &second.url,
                cwd,
                &config.child_env,
                &mut *stderr,
            )?;
            script_status(output.writer()?, &bytes)?;
        }
    }
    // The entire post-VCS block is best effort. A malformed success response
    // may already have affected state; never retry or roll back local work.
    let updated = (|| {
        let transport = client::prepare_transport(
            &config.options,
            credentials,
            workspace,
            &config.transport_env,
        )
        .map_err(|error| Error::new(format!("Error: {error}")))?;
        block_on_network(async {
            command::update_state(&transport, team_reference, identifier)
                .await
                .map_err(Error::new)
        })
    })();
    match updated {
        Ok(bytes) => script_status(output.writer()?, &bytes)?,
        Err(error) => output::eprint(
            format!("Failed to update issue state: {}\n", error.message()).as_bytes(),
        )?,
    }
    Ok(())
}
fn dispatch_issue_start(
    context: &Ctx,
    action: &cli::issue::IssueStart,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::{commands::issue_start as command, platform::prompt::PromptOutcome};
    let result = (|| {
        let (config, credentials) = start_snapshot(context)?;
        let team = configured_team_key(&config.options);
        let team =
            command::team_and_flags(team.as_deref(), action.all_assignees, action.unassigned)?
                .to_owned();
        // Start never infers from VCS. Falsey/unresolved input enters the picker.
        let identifier = match action.issue_id.as_deref().filter(|value| !value.is_empty()) {
            Some(input) => {
                let inputs = client::selection_inputs(&config.options, workspace);
                match crate::refs::prepare_issue_reference(
                    Some(input),
                    Some(&team),
                    &WorkspaceScope::from_selection(&inputs, credentials),
                )? {
                    crate::refs::IssueReference::Identifier(id) => Some(id),
                    crate::refs::IssueReference::Unresolved => None,
                    crate::refs::IssueReference::Inferred => {
                        return Err(Error::new("supplied start input inferred VCS"));
                    }
                }
            }
            None => None,
        };
        let stdin_tty = context.stdin_tty();
        let spin = spinner::enabled(false, context.stdout_tty(), true);
        let cwd = context.cwd().to_path_buf();
        let mut output = StartPromptOutput::new(context.stdout());
        let action_result = (|| {
            let identifier = match identifier {
                Some(identifier) => identifier,
                None => {
                    let priority =
                        config.options.issue_sort(None).0 == crate::config::IssueSort::Priority;
                    // Sort validation precedes transport construction, after team/conflict.
                    let transport = client::prepare_transport(
                        &config.options,
                        credentials,
                        workspace,
                        &config.transport_env,
                    )?;
                    let issues = block_on_network(command::list(
                        &transport,
                        command::filter(&team, action.all_assignees, action.unassigned),
                        priority,
                    ))?;
                    let options = command::choices(&issues, &team)?;
                    if stdin_tty {
                        command::check_prompt_topology(true, command::stdout_is_fifo()?)?;
                    }
                    let picked = output.prompt()?.searchable_select_with_no_match(
                        "Select an issue to start:",
                        "Search issues",
                        &options,
                        "no issues match submitted search query",
                    );
                    let picked = command::stage(picked?, "issue to start")?;
                    output.suspend()?;
                    match picked {
                        PromptOutcome::Submitted(identifier) => identifier,
                        PromptOutcome::Interrupted => return initiative_interrupt_status(),
                        PromptOutcome::EndOfInput => {
                            return Err(Error::new("start picker EOF conversion absent"));
                        }
                    }
                }
            };
            start_work_on_created_issue(
                &mut std::io::stderr(),
                config,
                credentials,
                workspace,
                &identifier,
                &team,
                &mut output,
                &cwd,
                spin,
                stdin_tty,
                action.branch.as_deref(),
                action.from_ref.as_deref(),
            )
        })();
        // Restore on answer, error, EOF and interruption before returning to app.
        output.close()?;
        action_result
    })();
    result.context(command::CONTEXT)
}
fn dispatch_issue_pull_request(
    context: &Ctx,
    action: &cli::issue::IssuePullRequest,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::{
        commands::issue_pull_request as command,
        platform::{gh_script::NativeGhRunner, vcs_script::NativeProcessRunner},
    };
    let result: Result<()> = (|| {
        let (config, credentials) = start_snapshot(context)?;
        let selected = if action.no_template {
            crate::config::PrTemplateCli::Disabled
        } else if let Some(path) = action.template.as_deref() {
            crate::config::PrTemplateCli::Path(path)
        } else {
            crate::config::PrTemplateCli::Unset
        };
        let path = config.options.pr_template(selected)?;
        let contents = path
            .as_ref()
            .map(|path| command::read_template(path.path()))
            .transpose()?;
        let identifier = resolve_script_issue(
            context,
            action.issue_id.as_deref(),
            workspace,
            &mut NativeProcessRunner,
        )?;
        let spin = spinner::enabled(false, context.stdout_tty(), true);
        let details = start_details(
            &mut context.stdout(),
            config,
            credentials,
            workspace,
            &identifier,
            spin,
        )?;
        let args = command::args(
            &identifier,
            &details.title,
            &details.url,
            contents.as_deref(),
            command::Options {
                title: action.title.as_deref(),
                base: action.base.as_deref(),
                head: action.head.as_deref(),
                draft: action.draft,
                web: action.web,
            },
        );
        command::create(&mut NativeGhRunner, &args, context.cwd(), &config.child_env)?;
        Ok(())
    })();
    result.context(command::CONTEXT)
}
