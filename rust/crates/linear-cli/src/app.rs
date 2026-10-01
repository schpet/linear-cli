use std::error::Error;
use std::ffi::OsString;
use std::future::Future;
use std::io;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use crate::auth::{ApiKeyInput, CredentialSelectionInputs};
use crate::cli;
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
use crate::config::{NoColor, StartupConfig};
use crate::error::{AppError, AppErrorKind, ExitStatus};
use crate::platform::output::{Output, OutputOutcome, OutputPolicy, Stream, failed_stream};
use crate::platform::spinner;
use crate::refs::{
    InitiativeReference, ProjectReference, WorkspaceScope, prepare_initiative_lookup,
    prepare_project_lookup, prepare_team_lookup, resolve_document_reference,
    resolve_initiative_with_transport, resolve_project_with_transport, resolve_team_with_transport,
};
use crate::startup::{AppStartupReport, render_startup_diagnostic};

/// Lazily run one network action on a current-thread IO runtime. The action
/// owns its inputs and returns before its caller writes to the CLI streams.
pub fn block_on_network<T, F>(future: F) -> Result<T, AppError>
where
    F: Future<Output = Result<T, AppError>>,
{
    block_on_network_with(future, || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
    })
}

fn block_on_network_with<T, F>(
    future: F,
    build: impl FnOnce() -> io::Result<tokio::runtime::Runtime>,
) -> Result<T, AppError>
where
    F: Future<Output = Result<T, AppError>>,
{
    let runtime = build().map_err(|error| {
        AppError::new(AppErrorKind::IoProcess, "could not start network runtime").with_source(error)
    })?;
    let result = runtime.block_on(future);
    runtime.shutdown_timeout(Duration::from_millis(500));
    result
}

pub struct AppContext<'a> {
    pub startup: AppStartupReport,
    pub cwd: PathBuf,
    pub stdout: &'a mut dyn Write,
    pub stderr: &'a mut dyn Write,
    pub stdin_tty: bool,
    pub stdout_tty: bool,
    pub stderr_tty: bool,
    pub stdout_finalization: Option<(OutputPolicy, OutputOutcome)>,
}

impl AppContext<'_> {
    pub fn debug_enabled(&self) -> bool {
        self.startup.settings.debug
    }

    pub fn no_color(&self) -> bool {
        self.startup.settings.no_color()
    }

    pub fn handled_color(&self) -> bool {
        self.stderr_tty && !self.no_color()
    }

    pub fn help_color(&self) -> bool {
        self.startup.settings.help_color()
    }

    pub fn config(&self) -> Result<&StartupConfig, AppError> {
        self.startup
            .result
            .as_ref()
            .map(|loaded| &loaded.config)
            .map_err(|_| {
                AppError::new(
                    AppErrorKind::Invariant,
                    "config was requested after startup failed",
                )
            })
    }

    pub fn credentials(&self) -> Result<&crate::auth::CredentialStore, AppError> {
        self.startup
            .result
            .as_ref()
            .map(|loaded| &loaded.credentials)
            .map_err(|_| {
                AppError::new(
                    AppErrorKind::Invariant,
                    "credentials requested after startup failed",
                )
            })
    }

    fn write_stdout(&mut self, bytes: &[u8]) -> Result<(), AppError> {
        self.write_stdout_with_policy(bytes, OutputPolicy::Strict)
    }

    fn write_stdout_with_policy(
        &mut self,
        bytes: &[u8],
        policy: OutputPolicy,
    ) -> Result<(), AppError> {
        self.stdout_finalization = None;
        let outcome =
            Output::new(&mut *self.stdout, Stream::Stdout).write_with_policy(bytes, policy)?;
        self.stdout_finalization = Some((policy, outcome));
        Ok(())
    }

    fn write_stderr(&mut self, bytes: &[u8]) -> Result<(), AppError> {
        Output::new(&mut *self.stderr, Stream::Stderr).write(bytes)
    }

    fn flush_all(&mut self) -> Result<(), AppError> {
        let policy = match self.stdout_finalization.take() {
            Some((OutputPolicy::ConsoleLike, OutputOutcome::QuietBrokenPipe)) => {
                OutputPolicy::ConsoleLike
            }
            Some(_) | None => OutputPolicy::Strict,
        };
        Output::new(&mut *self.stdout, Stream::Stdout).flush_with_policy(policy)?;
        Output::new(&mut *self.stderr, Stream::Stderr).flush()
    }
}

fn write_stdout(context: &mut AppContext<'_>, bytes: &[u8]) -> Result<(), AppError> {
    context.write_stdout(bytes)
}

fn write_stderr(context: &mut AppContext<'_>, bytes: &[u8]) -> Result<(), AppError> {
    context.write_stderr(bytes)
}

/// Emit one bootstrap diagnostic, including its flush. The caller returns status 1
/// even when this diagnostic cannot be written.
pub fn report_bootstrap_error(stderr: &mut dyn Write, error: &AppError) -> Result<(), AppError> {
    Output::new(stderr, Stream::Stderr).write(format!("✗ {error}\n").as_bytes())
}

fn report_output_failure(context: &mut AppContext<'_>, error: &AppError) {
    if failed_stream(error) == Some(Stream::Stdout) {
        let _ = report_bootstrap_error(context.stderr, error);
    }
}

/// Resolve route output and final stream flushes before the process chooses an exit code.
/// A write or flush failure always wins over the route status, including usage/child codes.
pub fn finalize(
    result: Result<ExitStatus, AppError>,
    context: &mut AppContext<'_>,
) -> Result<ExitStatus, AppError> {
    let (status, route_io_error, output_diagnostic_attempted) = match result {
        Ok(status) => (status, None, false),
        Err(error) => {
            // A failed stderr write already was the diagnostic attempt.
            if failed_stream(&error) == Some(Stream::Stderr) {
                return Err(error);
            }
            match write_final_error(context, &error) {
                Ok(status) => {
                    let output_diagnostic_attempted = failed_stream(&error) == Some(Stream::Stdout);
                    let io_error = (error.kind == AppErrorKind::IoProcess).then_some(error);
                    (status, io_error, output_diagnostic_attempted)
                }
                Err(write_error) => {
                    report_output_failure(context, &write_error);
                    return Err(write_error);
                }
            }
        }
    };
    if let Err(flush_error) = context.flush_all() {
        if !output_diagnostic_attempted {
            report_output_failure(context, &flush_error);
        }
        return Err(flush_error);
    }
    match route_io_error {
        Some(error) => Err(error),
        None => Ok(status),
    }
}

pub fn run(argv: &[String], context: &mut AppContext<'_>) -> Result<ExitStatus, AppError> {
    context.stdout_finalization = None;
    for diagnostic in context.startup.diagnostics.clone() {
        let rendered = render_startup_diagnostic(&diagnostic, context.help_color());
        write_stderr(context, rendered.as_bytes())?;
    }
    if let Err(error) = &context.startup.result {
        return Err(error.app_error());
    }
    let os_argv = argv.iter().map(OsString::from).collect::<Vec<_>>();
    dispatch(cli::parse(&os_argv)?, context)
}

fn dispatch(cli: cli::Cli, context: &mut AppContext<'_>) -> Result<ExitStatus, AppError> {
    let workspace = cli.workspace.as_deref();
    match cli.command {
        None => {
            context.write_stdout_with_policy(
                b"Use --help to see available commands\n",
                OutputPolicy::ConsoleLike,
            )?;
            Ok(ExitStatus::Success)
        }
        Some(cli::RootCommand::Auth(action)) => match action.command {
            None => parent_help(context, "linear auth"),
            Some(cli::auth::AuthCommand::Login(_)) => unsupported("linear auth login"),
            Some(cli::auth::AuthCommand::Logout(_)) => unsupported("linear auth logout"),
            Some(cli::auth::AuthCommand::List(action)) => {
                dispatch_auth_list(context, &action, workspace)
            }
            Some(cli::auth::AuthCommand::Default(action)) => {
                dispatch_auth_default(context, &action)
            }
            Some(cli::auth::AuthCommand::Token(_)) => dispatch_auth_token(context, workspace),
            Some(cli::auth::AuthCommand::Whoami(action)) => {
                dispatch_auth_whoami(context, &action, workspace)
            }
            Some(cli::auth::AuthCommand::Migrate(_)) => unsupported("linear auth migrate"),
        },
        Some(cli::RootCommand::Issue(action)) => match action.command {
            None => parent_help(context, "linear issue"),
            Some(cli::issue::IssueCommand::Id(_)) => dispatch_issue_id(context),
            Some(cli::issue::IssueCommand::Mine(_)) => unsupported("linear issue mine"),
            Some(cli::issue::IssueCommand::Query(_)) => unsupported("linear issue query"),
            Some(cli::issue::IssueCommand::Title(action)) => dispatch_issue_detail(
                context,
                action.issue_id.as_deref(),
                cli.workspace.as_deref(),
                IssueDetailField::Title,
            ),
            Some(cli::issue::IssueCommand::Start(_)) => unsupported("linear issue start"),
            Some(cli::issue::IssueCommand::View(_)) => unsupported("linear issue view"),
            Some(cli::issue::IssueCommand::Url(action)) => dispatch_issue_detail(
                context,
                action.issue_id.as_deref(),
                cli.workspace.as_deref(),
                IssueDetailField::Url,
            ),
            Some(cli::issue::IssueCommand::Describe(_)) => unsupported("linear issue describe"),
            Some(cli::issue::IssueCommand::Commits(_)) => unsupported("linear issue commits"),
            Some(cli::issue::IssueCommand::PullRequest(_)) => {
                unsupported("linear issue pull-request")
            }
            Some(cli::issue::IssueCommand::Archive(action)) => dispatch_issue_archive_delete(
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
            .map_err(|error| error.with_context("Failed to archive issue")),
            Some(cli::issue::IssueCommand::Delete(action)) => dispatch_issue_archive_delete(
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
            .map_err(|error| error.with_context("Failed to delete issue")),
            Some(cli::issue::IssueCommand::Create(_)) => unsupported("linear issue create"),
            Some(cli::issue::IssueCommand::Update(_)) => unsupported("linear issue update"),
            Some(cli::issue::IssueCommand::Comment(action)) => match action.command {
                None => parent_help(context, "linear issue comment"),
                Some(cli::issue::IssueCommentCommand::Add(action)) => {
                    dispatch_issue_comment_add(context, &action, workspace)
                }
                Some(cli::issue::IssueCommentCommand::Delete(action)) => {
                    dispatch_issue_comment_delete(context, &action, workspace)
                }
                Some(cli::issue::IssueCommentCommand::Update(action)) => {
                    dispatch_issue_comment_update(context, &action, workspace)
                }
                Some(cli::issue::IssueCommentCommand::List(action)) => {
                    dispatch_issue_comment_list(context, &action, workspace)
                }
            },
            Some(cli::issue::IssueCommand::Attach(action)) => {
                dispatch_issue_attach(context, &action, workspace)
            }
            Some(cli::issue::IssueCommand::Link(action)) => {
                dispatch_issue_link(context, &action, workspace)
            }
            Some(cli::issue::IssueCommand::Relation(action)) => match action.command {
                None => parent_help(context, "linear issue relation"),
                Some(cli::issue::IssueRelationCommand::Add(action)) => {
                    dispatch_issue_relation_add(context, &action, workspace)
                }
                Some(cli::issue::IssueRelationCommand::Delete(action)) => {
                    dispatch_issue_relation_delete(context, &action, workspace)
                }
                Some(cli::issue::IssueRelationCommand::List(action)) => {
                    dispatch_issue_relation_list(context, &action, workspace)
                }
            },
            Some(cli::issue::IssueCommand::AgentSession(action)) => match action.command {
                None => parent_help(context, "linear issue agent-session"),
                Some(cli::issue::IssueAgentSessionCommand::List(action)) => {
                    dispatch_agent_session_list(context, &action, workspace)
                }
                Some(cli::issue::IssueAgentSessionCommand::View(action)) => {
                    dispatch_agent_session_view(context, &action, workspace)
                }
            },
        },
        Some(cli::RootCommand::Team(action)) => match action.command {
            None => parent_help(context, "linear team"),
            Some(cli::team::TeamCommand::Create(action)) => {
                dispatch_team_create(context, &action, workspace)
            }
            Some(cli::team::TeamCommand::Delete(action)) => {
                dispatch_team_delete(context, &action, workspace)
            }
            Some(cli::team::TeamCommand::List(action)) => {
                dispatch_team_list(context, &action, workspace)
            }
            Some(cli::team::TeamCommand::Id(action)) => {
                dispatch_team_id(context, &action, workspace)
            }
            Some(cli::team::TeamCommand::Autolinks(_)) => {
                crate::commands::team_autolinks::execute(context.config()?, &context.cwd)?;
                Ok(ExitStatus::Success)
            }
            Some(cli::team::TeamCommand::Members(action)) => {
                dispatch_team_members(context, &action, workspace)
            }
            Some(cli::team::TeamCommand::States(action)) => {
                dispatch_team_states(context, &action, workspace)
            }
        },
        Some(cli::RootCommand::User(action)) => match action.command {
            None => parent_help(context, "linear user"),
            Some(cli::user::UserCommand::List(action)) => {
                dispatch_user_list(context, &action, workspace)
            }
        },
        Some(cli::RootCommand::Project(action)) => match action.command {
            None => parent_help(context, "linear project"),
            Some(cli::project::ProjectCommand::List(action)) => {
                dispatch_project_list(context, &action, workspace)
            }
            Some(cli::project::ProjectCommand::View(action)) => {
                dispatch_project_view(context, &action, workspace)
            }
            Some(cli::project::ProjectCommand::Create(action)) => {
                dispatch_project_create(context, &action, workspace)
            }
            Some(cli::project::ProjectCommand::Update(action)) => {
                dispatch_project_update(context, &action, workspace)
            }
            Some(cli::project::ProjectCommand::Delete(action)) => {
                dispatch_project_delete(context, &action, workspace)
            }
            Some(cli::project::ProjectCommand::Comment(action)) => match action.command {
                None => parent_help(context, "linear project comment"),
                Some(cli::project::ProjectCommentCommand::Add(action)) => {
                    dispatch_project_comment_add(context, &action, workspace)
                }
                Some(cli::project::ProjectCommentCommand::List(action)) => {
                    dispatch_project_comment_list(context, &action, workspace)
                }
            },
        },
        Some(cli::RootCommand::ProjectUpdate(action)) => match action.command {
            None => parent_help(context, "linear project-update"),
            Some(cli::project_update::ProjectUpdateCommand::Create(action)) => {
                dispatch_update_create(
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
                )
            }
            Some(cli::project_update::ProjectUpdateCommand::List(action)) => {
                dispatch_project_update_list(context, &action, workspace)
            }
        },
        Some(cli::RootCommand::Cycle(action)) => match action.command {
            None => parent_help(context, "linear cycle"),
            Some(cli::cycle::CycleCommand::List(action)) => {
                dispatch_cycle_list(context, &action, workspace)
            }
            Some(cli::cycle::CycleCommand::View(action)) => {
                dispatch_cycle_view(context, &action, workspace)
            }
        },
        Some(cli::RootCommand::Milestone(action)) => match action.command {
            None => parent_help(context, "linear milestone"),
            Some(cli::milestone::MilestoneCommand::List(action)) => {
                dispatch_milestone_list(context, &action, workspace)
            }
            Some(cli::milestone::MilestoneCommand::View(action)) => {
                dispatch_milestone_view(context, &action, workspace)
            }
            Some(cli::milestone::MilestoneCommand::Create(action)) => {
                dispatch_milestone_create(context, &action, workspace)
            }
            Some(cli::milestone::MilestoneCommand::Update(action)) => {
                dispatch_milestone_update(context, &action, workspace)
            }
            Some(cli::milestone::MilestoneCommand::Delete(action)) => {
                dispatch_milestone_delete(context, &action, workspace)
            }
        },
        Some(cli::RootCommand::Initiative(action)) => match action.command {
            None => parent_help(context, "linear initiative"),
            Some(cli::initiative::InitiativeCommand::List(action)) => {
                dispatch_initiative_list(context, &action, workspace)
            }
            Some(cli::initiative::InitiativeCommand::View(action)) => {
                dispatch_initiative_view(context, &action, workspace)
            }
            Some(cli::initiative::InitiativeCommand::Create(action)) => {
                dispatch_initiative_create(context, &action, workspace)
            }
            Some(cli::initiative::InitiativeCommand::Archive(action)) => dispatch_initiative_bulk(
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
            Some(cli::initiative::InitiativeCommand::Update(action)) => {
                dispatch_initiative_update(context, &action, workspace)
            }
            Some(cli::initiative::InitiativeCommand::Unarchive(action)) => {
                dispatch_initiative_unarchive(context, &action, workspace)
            }
            Some(cli::initiative::InitiativeCommand::Delete(action)) => dispatch_initiative_bulk(
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
            Some(cli::initiative::InitiativeCommand::AddProject(action)) => {
                dispatch_initiative_projects(
                    context,
                    &action.initiative,
                    &action.project,
                    action.sort_order,
                    true,
                    initiative_projects::Mode::Add,
                    workspace,
                )
            }
            Some(cli::initiative::InitiativeCommand::RemoveProject(action)) => {
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
            Some(cli::initiative::InitiativeCommand::Comment(action)) => match action.command {
                None => parent_help(context, "linear initiative comment"),
                Some(cli::initiative::InitiativeCommentCommand::Add(action)) => {
                    dispatch_initiative_comment_add(context, &action, workspace)
                }
                Some(cli::initiative::InitiativeCommentCommand::List(action)) => {
                    dispatch_initiative_comment_list(context, &action, workspace)
                }
            },
        },
        Some(cli::RootCommand::InitiativeUpdate(action)) => match action.command {
            None => parent_help(context, "linear initiative-update"),
            Some(cli::initiative_update::InitiativeUpdateCommand::Create(action)) => {
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
            Some(cli::initiative_update::InitiativeUpdateCommand::List(action)) => {
                dispatch_initiative_update_list(context, &action, workspace)
            }
        },
        Some(cli::RootCommand::Label(action)) => match action.command {
            None => parent_help(context, "linear label"),
            Some(cli::label::LabelCommand::List(action)) => {
                dispatch_label_list(context, &action, workspace)
            }
            Some(cli::label::LabelCommand::Create(action)) => {
                dispatch_label_create(context, &action, workspace)
            }
            Some(cli::label::LabelCommand::Delete(action)) => {
                dispatch_label_delete(context, &action, workspace)
            }
        },
        Some(cli::RootCommand::Template(action)) => match action.command {
            None => parent_help(context, "linear template"),
            Some(cli::template::TemplateCommand::List(action)) => {
                dispatch_template_list(context, &action, workspace)
            }
            Some(cli::template::TemplateCommand::View(action)) => {
                dispatch_template_view(context, &action, workspace)
            }
        },
        Some(cli::RootCommand::Document(action)) => match action.command {
            None => document_hint(context),
            Some(cli::document::DocumentCommand::List(action)) => {
                dispatch_document_list(context, &action, workspace)
            }
            Some(cli::document::DocumentCommand::View(action)) => {
                dispatch_document_view(context, &action, workspace)
            }
            Some(cli::document::DocumentCommand::Create(action)) => {
                dispatch_document_create(context, &action, workspace)
            }
            Some(cli::document::DocumentCommand::Update(action)) => {
                dispatch_document_update(context, &action, workspace)
            }
            Some(cli::document::DocumentCommand::Delete(action)) => {
                dispatch_document_delete(context, &action, workspace)
            }
            Some(cli::document::DocumentCommand::Comment(action)) => match action.command {
                None => parent_help(context, "linear document comment"),
                Some(cli::document::DocumentCommentCommand::Add(action)) => {
                    dispatch_document_comment_add(context, &action, workspace)
                }
                Some(cli::document::DocumentCommentCommand::List(action)) => {
                    dispatch_document_comment_list(context, &action, workspace)
                }
            },
        },
        Some(cli::RootCommand::Completions(action)) => match action.command {
            None => parent_help(context, "linear completions"),
            Some(cli::completions::CompletionsCommand::Bash(action)) => {
                write_completion_script(context, CompletionShell::Bash, action.name.as_deref())
            }
            Some(cli::completions::CompletionsCommand::Fish(action)) => {
                write_completion_script(context, CompletionShell::Fish, action.name.as_deref())
            }
            Some(cli::completions::CompletionsCommand::Zsh(action)) => {
                write_completion_script(context, CompletionShell::Zsh, action.name.as_deref())
            }
            Some(cli::completions::CompletionsCommand::Complete(action)) => {
                write_complete(context, &action)
            }
        },
        Some(cli::RootCommand::Config(_)) => dispatch_config_generate(context, workspace),
        Some(cli::RootCommand::Schema(_)) => unsupported("linear schema"),
        Some(cli::RootCommand::Api(_)) => unsupported("linear api"),
        Some(cli::RootCommand::Markdown(_)) => markdown(context),
    }
}
fn unsupported(path: &str) -> Result<ExitStatus, AppError> {
    Err(AppError::new(
        AppErrorKind::Unimplemented,
        format!("{path} is registered, but this action is not implemented yet"),
    ))
}

fn parent_help(context: &mut AppContext<'_>, path: &str) -> Result<ExitStatus, AppError> {
    let mut command = cli::command();
    command.build();
    let mut selected = &mut command;
    for word in path.split_whitespace().skip(1) {
        selected = selected.find_subcommand_mut(word).ok_or_else(|| {
            AppError::new(
                AppErrorKind::Invariant,
                format!("native command path missing: {path}"),
            )
        })?;
    }
    context.write_stdout_with_policy(
        selected.render_long_help().to_string().as_bytes(),
        OutputPolicy::ConsoleLike,
    )?;
    Ok(ExitStatus::Success)
}

fn document_hint(context: &mut AppContext<'_>) -> Result<ExitStatus, AppError> {
    write_stdout(context, b"Use --help to see available subcommands\n")?;
    Ok(ExitStatus::Success)
}

fn markdown(context: &mut AppContext<'_>) -> Result<ExitStatus, AppError> {
    context.write_stdout_with_policy("Linear-flavored Markdown: mentions and collapsible sections\n\nThese rules apply to comment bodies, issue descriptions, document content,\nproject overviews, and status update bodies.\n\nMENTIONS\n\nA resource's plain Linear URL becomes a linked mention. A literal `@name`, an\n`@[Name](id)`, or a Markdown link such as `[Name](url)` does not — it stays\nplain text and notifies nobody. Put the bare URL in the body:\n\nhttps://linear.app/acme/profiles/someuser can you take a look?\n\nRESOLVING PEOPLE\n\nLook the person up in the relevant team first. The team can usually be\ninferred from the issue identifier or the current directory:\n\nlinear team members ENG --json\n\nPaste the selected member's `url` field verbatim. If the intended person is\nnot a member of that team, stop and confirm before searching the whole\nworkspace with `linear user list --json`; mentioning someone outside the team\nis likely accidental.\n\nTo mention an issue, use its URL the same way:\n\nlinear issue url ENG-123\n\nCOLLAPSIBLE SECTIONS\n\nOpen a section with `+++ [title]` and close it with `+++`:\n\n+++ [Server log]\n\nMarkdown content that is initially hidden.\n\n+++\n\nThe square brackets around the title and the closing `+++` are both required.\n".as_bytes(), OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn write_complete(
    context: &mut AppContext<'_>,
    action: &cli::completions::CompletionsComplete,
) -> Result<ExitStatus, AppError> {
    let output = completions::complete(action)?;
    if !output.is_empty() {
        context.write_stdout_with_policy(&output, OutputPolicy::Strict)?;
    }
    Ok(ExitStatus::Success)
}

fn missing_team_key() -> AppError {
    AppError::new(
        AppErrorKind::Validation,
        "Could not determine team key from directory name",
    )
    .with_suggestion("Please specify a team key, name, or ID as an argument.")
}

fn dispatch_project_view(
    context: &mut AppContext<'_>,
    action: &cli::project::ProjectView,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    use crate::platform::{markdown_terminal::HostSource, pager, selector};
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
            return Err(AppError::new(AppErrorKind::Validation, "A project is required with --json")
                .with_suggestion("Pass a project UUID, slug ID, or exact name, or drop --json to pick one from a list.")
                .with_context(project_view::CONTEXT));
        }
        let interactive = {
            let config = context.config()?;
            selector::interactive_allowed(
                context.stdin_tty,
                context.stdout_tty,
                config.ci.as_deref(),
            )
        };
        if !interactive {
            return Err(AppError::new(AppErrorKind::Validation, "No project specified")
                .with_suggestion("Pass a project UUID, slug ID, or exact name. Running `linear project view` with no argument picks from a list, but only on a terminal.")
                .with_context(project_view::CONTEXT));
        }
        let config = context.config()?;
        let team_key = configured_team_key(&config.options);
        let transport = client::prepare_transport(
            &config.options,
            context.credentials()?,
            cli_workspace,
            &config.transport_env,
        )
        .map_err(|error| error.with_context(project_view::CONTEXT))?;
        let projects =
            block_on_network(project_view::fetch_picker(&transport, team_key.as_deref()))
                .map_err(|error| error.with_context(project_view::CONTEXT))?;
        let options = project_view::picker_options(&projects)
            .map_err(|error| error.with_context(project_view::CONTEXT))?;
        let ci = context.config()?.ci.clone();
        let selection = selector::run(
            &options,
            &selector::PromptLabels {
                message: "Select a project",
                search_label: "Search projects",
                max_rows: 10,
            },
            ci.as_deref(),
            context.stdout,
        )
        .map_err(|error| error.with_context(project_view::CONTEXT))?;
        match selection {
            selector::Selection::Selected(id) => id,
            selector::Selection::Interrupted => return Ok(ExitStatus::HandledFailure),
            selector::Selection::EndOfInput => {
                return Err(AppError::new(
                    AppErrorKind::Validation,
                    "Project selection ended before a project was chosen",
                )
                .with_context(project_view::CONTEXT));
            }
        }
    };

    // A UUID browser reference needs neither credential selection nor GraphQL.
    let (resolved_id, transport) =
        if explicit.is_some() && is_linear_uuid(&original) && (web || app) {
            (original.clone(), None)
        } else {
            let config = context.config()?;
            let credentials = context.credentials()?;
            let inputs = client::selection_inputs(&config.options, cli_workspace)
                .map_err(|error| error.with_context(project_view::CONTEXT))?;
            let reference = if explicit.is_some() {
                Some(
                    prepare_project_lookup(
                        &original,
                        &WorkspaceScope::from_selection(&inputs, credentials),
                    )
                    .map_err(|error| error.with_context(project_view::CONTEXT))?,
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
            .map_err(|error| error.with_context(project_view::CONTEXT))?;
            let id = match reference {
                Some(reference) => block_on_network(resolve_project_with_transport(
                    &reference, &original, &transport,
                ))
                .map_err(|error| error.with_context(project_view::CONTEXT))?,
                None => original.clone(),
            };
            (id, Some(transport))
        };

    if web || app {
        let workspace = context
            .config()?
            .options
            .workspace()
            .map(|value| value.value().clone())
            .filter(|value| !value.is_empty());
        let Some(workspace) = workspace else {
            context.write_stderr(
                b"workspace is not set via command line, configuration file, or environment.\n",
            )?;
            return Ok(ExitStatus::HandledFailure);
        };
        let url = format!("https://linear.app/{workspace}/project/{resolved_id}");
        let destination = if app { "Linear.app" } else { "web browser" };
        context.write_stdout_with_policy(
            format!("Opening {url} in {destination}\n").as_bytes(),
            OutputPolicy::ConsoleLike,
        )?;
        project_list::open(&url, app).map_err(|mut error| {
            error.context = None;
            error.with_context(project_view::CONTEXT)
        })?;
        return Ok(ExitStatus::Success);
    }

    let transport = transport
        .ok_or_else(|| AppError::new(AppErrorKind::Invariant, "project transport missing"))?;
    let show_spinner = spinner::enabled(
        json,
        context.stdout_tty,
        context.startup.settings.no_color == NoColor::Absent,
    );
    if show_spinner {
        context
            .write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
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
                        context.write_stdout_with_policy(spinner::frame(frame).as_bytes(), OutputPolicy::ConsoleLike)?;
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
        context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
    }
    let project = result.map_err(|error| error.with_context(project_view::CONTEXT))?;
    if json {
        let bytes = project_view::json(&project)
            .map_err(|error| error.with_context(project_view::CONTEXT))?;
        context.write_stdout_with_policy(&bytes, OutputPolicy::ConsoleLike)?;
        return Ok(ExitStatus::Success);
    }
    let markdown = project_view::markdown(&project, chrono::Utc::now(), &chrono::Local)
        .map_err(|error| error.with_context(project_view::CONTEXT))?;
    if !context.stdout_tty {
        context.write_stdout_with_policy(
            format!("{markdown}\n").as_bytes(),
            OutputPolicy::ConsoleLike,
        )?;
        return Ok(ExitStatus::Success);
    }
    let config = context.config()?;
    let pager_value = config.pager.clone();
    let no_color = context.startup.settings.no_color;
    let hyperlink_format = config
        .options
        .hyperlink_format()
        .map(|value| value.value().clone());
    let mut runner = pager::ProcessPagerRunner::inheriting(config.child_env.iter());
    let request = pager::PagerRequest {
        enabled: pager_enabled,
        stdout_tty: context.stdout_tty,
        size: pager::stdout_size(),
        pager: pager_value.as_deref(),
        os: pager::HOST_OS,
    };
    pager::render_and_show(
        &markdown,
        &request,
        no_color,
        hyperlink_format.as_deref(),
        HostSource::System,
        &mut runner,
        context.stdout,
    )
    .map_err(|error| error.with_context(project_view::CONTEXT))?;
    Ok(ExitStatus::Success)
}

fn dispatch_milestone_view(
    context: &mut AppContext<'_>,
    action: &cli::milestone::MilestoneView,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    let json = action.json;
    let all = action.all;
    let original = action.milestone.clone();
    let project = action.project.clone();
    let show_spinner = spinner::enabled(
        json,
        context.stdout_tty,
        context.startup.settings.no_color == NoColor::Absent,
    );
    if show_spinner {
        context
            .write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
    }
    let prepared = (|| {
        let config = context.config()?;
        let credentials = context.credentials()?;

        let inputs = client::selection_inputs(&config.options, workspace)?;
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
        Ok::<_, AppError>((reference, transport))
    })();
    let (reference, transport) = match prepared {
        Ok(value) => value,
        Err(error) => {
            if show_spinner {
                context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
            }
            return Err(error.with_context(milestone_view::CONTEXT));
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
                return Err(AppError::new(
                    AppErrorKind::Invariant,
                    "project reference mismatch",
                ));
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
                        context.write_stdout_with_policy(spinner::frame(frame).as_bytes(), OutputPolicy::ConsoleLike)?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        })
    } else {
        block_on_network(fetch)
    };
    if show_spinner {
        context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
    }
    let milestone = result.map_err(|error| error.with_context(milestone_view::CONTEXT))?;
    let output = if json {
        milestone_view::json(&milestone)
            .map_err(|error| error.with_context(milestone_view::CONTEXT))?
    } else {
        let markdown =
            milestone_view::markdown(&milestone, all, chrono::Utc::now(), &chrono::Local);
        let rendered = if context.stdout_tty {
            use std::num::NonZeroU16;
            let columns = u16::try_from(table::stdout_columns(true))
                .ok()
                .and_then(NonZeroU16::new)
                .unwrap_or(crate::platform::markdown_terminal::FALLBACK_COLUMNS);
            let options = crate::platform::markdown_terminal::RenderOptions::for_terminal(
                columns,
                context.startup.settings.no_color,
                true,
                None,
                crate::platform::markdown_terminal::HostSource::System,
            );
            crate::platform::markdown_terminal::render(&markdown, &options)
                .map_err(|error| error.with_context(milestone_view::CONTEXT))?
        } else {
            markdown
        };
        format!("{rendered}\n").into_bytes()
    };
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn dispatch_project_update_list(
    context: &mut AppContext<'_>,
    action: &cli::project_update::ProjectUpdateList,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    let json = action.json;
    let first = project_update_list::graphql_int(action.limit)?;
    let original = action.project_id.clone();
    let show_spinner = spinner::enabled(
        json,
        context.stdout_tty,
        context.startup.settings.no_color == NoColor::Absent,
    );
    if show_spinner {
        context
            .write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
    }
    let result = (|| {
        let config = context.config()?;
        let credentials = context.credentials()?;

        let inputs = client::selection_inputs(&config.options, workspace)?;
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
        let columns = table::stdout_columns(context.stdout_tty);
        let color = project_update_list::output_color(context.stdout_tty, context.no_color());
        block_on_network(async {
            let id = resolve_project_with_transport(&reference, &original, &transport).await?;
            project_update_list::run(&transport, &original, &id, first, json, columns, color).await
        })
    })();
    if show_spinner {
        context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
    }
    let output = result.map_err(|error| {
        if error.context.is_some() {
            error
        } else {
            error.with_context(project_update_list::CONTEXT)
        }
    })?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn dispatch_initiative_create(
    context: &mut AppContext<'_>,
    action: &cli::initiative::InitiativeCreate,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
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
    let config = context.config()?;
    let credentials = context.credentials()?;
    let cli_workspace = workspace;
    let inputs = client::selection_inputs(&config.options, cli_workspace)
        .map_err(|error| error.with_context(initiative_create::CREATE_CONTEXT))?;
    let transport = client::prepare_transport_with_inputs(
        &config.options,
        credentials,
        &inputs,
        &config.transport_env,
    )
    .map_err(|error| error.with_context(initiative_create::CREATE_CONTEXT))?;
    if initiative_create::should_prompt(&options, context.stdout_tty) {
        context.write_stdout_with_policy(
            b"\nCreate a new initiative\n\n",
            OutputPolicy::ConsoleLike,
        )?;
        let mut session = crate::platform::prompt::PromptSession::stdio(&mut *context.stdout)?;
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
                        cleanup.message.push_str(&format!(
                            "; prompt also failed: {}",
                            error.display_message()
                        ));
                        cleanup
                    }
                });
            }
        };
        match result {
            initiative_create::PromptResult::Complete => {}
            initiative_create::PromptResult::Interrupted => {
                return Ok(ExitStatus::ChildCode(
                    std::num::NonZeroU8::new(130).ok_or_else(|| {
                        AppError::new(AppErrorKind::Invariant, "exit code 130 must be nonzero")
                    })?,
                ));
            }
            initiative_create::PromptResult::EndOfInput => {
                return Err(AppError::new(
                    AppErrorKind::Validation,
                    "unexpected EOF while prompting for initiative",
                ));
            }
        }
    }
    let status = initiative_create::validate(&options)?;
    let owner_id = block_on_network(initiative_create::resolve_owner(
        &transport,
        options.owner.as_deref(),
    ))
    .map_err(|error| error.with_context(initiative_create::CREATE_CONTEXT))?;
    let show_spinner = spinner::enabled(
        false,
        context.stdout_tty,
        context.startup.settings.no_color == NoColor::Absent,
    );
    if show_spinner {
        context
            .write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
    }
    let result = block_on_network(initiative_create::submit_create(
        &transport, options, status, owner_id,
    ));
    if show_spinner {
        context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
    }
    let result = result.map_err(|error| error.with_context(initiative_create::CREATE_CONTEXT))?;
    context.write_stdout_with_policy(&result, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn dispatch_initiative_update_list(
    context: &mut AppContext<'_>,
    action: &cli::initiative_update::InitiativeUpdateList,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    let json = action.json;
    let first = initiative_update_list::graphql_int(action.limit)?;
    let original = &action.initiative_id;
    let show_spinner = spinner::enabled(
        json,
        context.stdout_tty,
        context.startup.settings.no_color == NoColor::Absent,
    );
    if show_spinner {
        context
            .write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
    }
    let result = (|| {
        let config = context.config()?;
        let credentials = context.credentials()?;

        let inputs = client::selection_inputs(&config.options, workspace)?;
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
        let columns = table::stdout_columns(context.stdout_tty);
        let color = context.stdout_tty && !context.no_color();
        block_on_network(async {
            let id = initiative_view::resolve_reference(&transport, &reference, original)
                .await
                .map_err(|mut error| {
                    error.context = Some(initiative_update_list::CONTEXT.to_owned());
                    error
                })?;
            initiative_update_list::run(&transport, original, &id, first, json, columns, color)
                .await
        })
    })();
    if show_spinner {
        context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
    }
    let output = result.map_err(|error| {
        if error.context.is_some() {
            error
        } else {
            error.with_context(initiative_update_list::CONTEXT)
        }
    })?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn dispatch_initiative_projects(
    context: &mut AppContext<'_>,
    initiative_arg: &str,
    project_arg: &str,
    sort_order: Option<f64>,
    force: bool,
    mode: initiative_projects::Mode,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    use crate::platform::prompt::{PromptOutcome, PromptSession};
    let config = context.config()?;
    let credentials = context.credentials()?;
    let inputs = client::selection_inputs(&config.options, workspace)?;
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
                context.write_stdout_with_policy(
                    format!(
                        "Project \"{}\" is not linked to initiative \"{}\"\n",
                        project.name, initiative.name
                    )
                    .as_bytes(),
                    OutputPolicy::ConsoleLike,
                )?;
                return Ok(ExitStatus::Success);
            }
            if !force {
                if !context.stdin_tty {
                    return Err(AppError::new(
                        AppErrorKind::Validation,
                        "Interactive confirmation required. Use --force to skip.",
                    ));
                }
                let outcome = {
                    let mut session = PromptSession::confirmation_stdio(&mut *context.stdout)?;
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
                        context.write_stdout_with_policy(
                            b"Removal cancelled.\n",
                            OutputPolicy::ConsoleLike,
                        )?;
                        return Ok(ExitStatus::Success);
                    }
                    PromptOutcome::Interrupted => {
                        return Ok(ExitStatus::ChildCode(
                            std::num::NonZeroU8::new(130).ok_or_else(|| {
                                AppError::new(
                                    AppErrorKind::Invariant,
                                    "exit code 130 must be nonzero",
                                )
                            })?,
                        ));
                    }
                    PromptOutcome::EndOfInput => {
                        return Err(AppError::new(
                            AppErrorKind::Validation,
                            "unexpected EOF while prompting for confirmation",
                        ));
                    }
                }
            }
            link
        }
    };
    let show_spinner = spinner::enabled(
        false,
        context.stdout_tty,
        context.startup.settings.no_color == NoColor::Absent,
    );
    if show_spinner {
        context
            .write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
    }
    let result = match mode {
        initiative_projects::Mode::Add => block_on_network(initiative_projects::add(
            &transport,
            &initiative,
            &project,
            sort_order,
        )),
        initiative_projects::Mode::Remove => {
            let link_id = link.ok_or_else(|| {
                AppError::new(
                    AppErrorKind::Invariant,
                    "confirmed removal requires a link ID",
                )
            })?;
            block_on_network(initiative_projects::remove(
                &transport,
                &link_id,
                &initiative,
                &project,
            ))
        }
    };
    if show_spinner {
        context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
    }
    context.write_stdout_with_policy(&result?, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn dispatch_initiative_unarchive(
    context: &mut AppContext<'_>,
    action: &cli::initiative::InitiativeUnarchive,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    use crate::platform::prompt::{PromptOutcome, PromptSession};
    let original = &action.initiative_id;
    let config = context.config()?;
    let credentials = context.credentials()?;
    let inputs = client::selection_inputs(&config.options, workspace)
        .map_err(|error| error.with_context(initiative_view::RESOLVE_CONTEXT))?;
    let transport = client::prepare_transport_with_inputs(
        &config.options,
        credentials,
        &inputs,
        &config.transport_env,
    )
    .map_err(|error| error.with_context(initiative_view::RESOLVE_CONTEXT))?;
    let reference = initiative_view::prepare_reference(
        original,
        &WorkspaceScope::from_selection(&inputs, credentials),
    )
    .map_err(|error| error.with_context(initiative_view::RESOLVE_CONTEXT))?;
    let id = block_on_network(initiative_unarchive::resolve_reference(
        &transport, &reference, original,
    ))?;
    let detail = block_on_network(initiative_unarchive::fetch_details(
        &transport, &id, original,
    ))?;
    if let Some(output) = initiative_unarchive::active_output(&detail) {
        context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
        return Ok(ExitStatus::Success);
    }
    if !action.force {
        if !context.stdin_tty {
            return Err(AppError::new(
                AppErrorKind::Validation,
                "Interactive confirmation required. Use --force to skip.",
            ));
        }
        let outcome = {
            let mut session = PromptSession::confirmation_stdio(&mut *context.stdout)?;
            let result = session.confirm(
                &format!("Are you sure you want to unarchive \"{}\"?", detail.name),
                true,
            );
            session.finish_result(result)?
        };
        match outcome {
            PromptOutcome::Submitted(true) => {}
            PromptOutcome::Submitted(false) => {
                context.write_stdout_with_policy(
                    b"Unarchive cancelled.\n",
                    OutputPolicy::ConsoleLike,
                )?;
                return Ok(ExitStatus::Success);
            }
            PromptOutcome::Interrupted => {
                return Ok(ExitStatus::ChildCode(
                    std::num::NonZeroU8::new(130).ok_or_else(|| {
                        AppError::new(AppErrorKind::Invariant, "exit code 130 must be nonzero")
                    })?,
                ));
            }
            PromptOutcome::EndOfInput => {
                return Err(AppError::new(
                    AppErrorKind::Validation,
                    "unexpected EOF while prompting for confirmation",
                ));
            }
        }
    }
    let show_spinner = spinner::enabled(
        false,
        context.stdout_tty,
        context.startup.settings.no_color == NoColor::Absent,
    );
    if show_spinner {
        context
            .write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
    }
    let result = block_on_network(initiative_unarchive::submit(&transport, &id));
    if show_spinner {
        context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
    }
    let output = result?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn dispatch_initiative_view(
    context: &mut AppContext<'_>,
    action: &cli::initiative::InitiativeView,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    let original = &action.initiative_id;
    let app = action.app;
    let web = action.web;
    let json = action.json;
    let config = context.config()?;
    let credentials = context.credentials()?;
    let cli_workspace = workspace;
    let inputs = client::selection_inputs(&config.options, cli_workspace)
        .map_err(|error| error.with_context(initiative_view::RESOLVE_CONTEXT))?;
    let transport = client::prepare_transport_with_inputs(
        &config.options,
        credentials,
        &inputs,
        &config.transport_env,
    )
    .map_err(|error| error.with_context(initiative_view::RESOLVE_CONTEXT))?;
    let scope = WorkspaceScope::from_selection(&inputs, credentials);
    let reference = initiative_view::prepare_reference(original, &scope)
        .map_err(|error| error.with_context(initiative_view::RESOLVE_CONTEXT))?;
    let id = block_on_network(initiative_view::resolve_reference(
        &transport, &reference, original,
    ))?;
    let show_spinner = !(app || web)
        && spinner::enabled(
            json,
            context.stdout_tty,
            context.startup.settings.no_color == NoColor::Absent,
        );
    if show_spinner {
        context
            .write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
    }
    let result = block_on_network(initiative_view::fetch_details(&transport, id, original));
    if show_spinner {
        context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
    }
    let detail = result?;
    if app || web {
        if detail.url.is_empty() {
            return Err(AppError::not_found("Initiative", original)
                .with_context(initiative_view::FETCH_CONTEXT));
        }
        context.write_stdout_with_policy(
            &initiative_view::opening(&detail, app),
            OutputPolicy::ConsoleLike,
        )?;
        crate::platform::opener::open(&detail.url, app)
            .map_err(|error| error.with_context(initiative_view::OPEN_CONTEXT))?;
        return Ok(ExitStatus::Success);
    }
    let output = if json {
        initiative_view::render_json(&detail)
    } else {
        let columns = std::num::NonZeroU16::new(
            u16::try_from(table::stdout_columns(context.stdout_tty)).unwrap_or(80),
        )
        .unwrap_or(crate::platform::markdown_terminal::FALLBACK_COLUMNS);
        initiative_view::render_text(
            &detail,
            context.stdout_tty,
            columns,
            context.startup.settings.no_color,
        )
    }
    .map_err(|error| error.with_context(initiative_view::FETCH_CONTEXT))?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn dispatch_initiative_list(
    context: &mut AppContext<'_>,
    action: &cli::initiative::InitiativeList,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
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
        let config = context.config()?;
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
                    .map_err(|error| error.with_context(initiative_list::OPEN_CONTEXT))?;
                let inputs = client::selection_inputs(&config.options, cli_workspace)
                    .map_err(|error| error.with_context(initiative_list::OPEN_CONTEXT))?;
                let transport = client::prepare_transport_with_inputs(
                    &config.options,
                    credentials,
                    &inputs,
                    &config.transport_env,
                )
                .map_err(|error| error.with_context(initiative_list::OPEN_CONTEXT))?;
                block_on_network(initiative_list::viewer_workspace(&transport))
                    .map_err(|error| error.with_context(initiative_list::OPEN_CONTEXT))?
            }
        };
        let (url, opening) = initiative_list::opening(&workspace, options.app);
        context.write_stdout_with_policy(&opening, OutputPolicy::ConsoleLike)?;
        initiative_list::open(&url, options.app)?;
        return Ok(ExitStatus::Success);
    }

    let show_spinner = spinner::enabled(
        options.json,
        context.stdout_tty,
        context.startup.settings.no_color == NoColor::Absent,
    );
    if show_spinner {
        context
            .write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
    }
    let result = (|| {
        let status =
            initiative_list::status_filter(options.status.as_deref(), options.all_statuses)?;
        initiative_list::validate_owner(options.owner.as_deref())?;
        let config = context.config()?;
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
            table::stdout_columns(context.stdout_tty),
            context.stdout_tty && !context.no_color(),
        ))
    })();
    if show_spinner {
        context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
    }
    let output = result.map_err(|error| error.with_context(initiative_list::FETCH_CONTEXT))?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn dispatch_project_comment_list(
    context: &mut AppContext<'_>,
    action: &cli::project::ProjectCommentList,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    let json = action.json;
    let original = &action.project;
    let config = context.config()?;
    let credentials = context.credentials()?;

    let inputs = client::selection_inputs(&config.options, workspace)
        .map_err(|error| error.with_context(project_comment_list::CONTEXT))?;
    let reference = prepare_project_lookup(
        original,
        &WorkspaceScope::from_selection(&inputs, credentials),
    )
    .map_err(|error| error.with_context(project_comment_list::CONTEXT))?;
    let transport = client::prepare_transport_with_inputs(
        &config.options,
        credentials,
        &inputs,
        &config.transport_env,
    )
    .map_err(|error| error.with_context(project_comment_list::CONTEXT))?;
    let color = !context.no_color();
    let output = block_on_network(async {
        let id = resolve_project_with_transport(&reference, original, &transport)
            .await
            .map_err(|error| error.with_context(project_comment_list::CONTEXT))?;
        project_comment_list::run(&transport, original, &id, json, color).await
    })?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn dispatch_milestone_list(
    context: &mut AppContext<'_>,
    action: &cli::milestone::MilestoneList,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    let json = action.json;
    let original = action.project.clone();
    // Deno starts this spinner before config, credential and URL preparation,
    // and its catch path stops it before reporting any action error.
    let show_spinner = spinner::enabled(
        json,
        context.stdout_tty,
        context.startup.settings.no_color == NoColor::Absent,
    );
    if show_spinner {
        context
            .write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
    }
    let prepared = (|| {
        let config = context.config()?;
        let credentials = context.credentials()?;

        let inputs = client::selection_inputs(&config.options, workspace)?;
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
        Ok::<_, AppError>((reference, transport))
    })();
    let (reference, transport) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            if show_spinner {
                context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
            }
            return Err(error.with_context(milestone_list::CONTEXT));
        }
    };
    let columns = table::stdout_columns(context.stdout_tty);
    let color = context.stdout_tty && !context.no_color();
    // Resolver errors gain the context here; `milestone_list::run` already
    // applies it to every page, cursor and rendering error.
    let fetch = async {
        let project_id = resolve_project_with_transport(&reference, &original, &transport)
            .await
            .map_err(|error| error.with_context(milestone_list::CONTEXT))?;
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
                        context.write_stdout_with_policy(
                            spinner::frame(frame).as_bytes(),
                            OutputPolicy::ConsoleLike,
                        )?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        })
    } else {
        block_on_network(fetch)
    };
    if show_spinner {
        context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
    }
    let output = output_result?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn dispatch_milestone_create(
    context: &mut AppContext<'_>,
    action: &cli::milestone::MilestoneCreate,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    let original = action.project.clone();
    let options = milestone_create::Options {
        name: action.name.clone(),
        description: action.description.clone(),
        target_date: action.target_date.clone(),
    };
    // Deno starts this spinner before config, credential and URL preparation,
    // and its catch path stops it before reporting any action error.
    let show_spinner = spinner::enabled(
        false,
        context.stdout_tty,
        context.startup.settings.no_color == NoColor::Absent,
    );
    if show_spinner {
        context
            .write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
    }
    let prepared = (|| {
        let config = context.config()?;
        let credentials = context.credentials()?;

        let inputs = client::selection_inputs(&config.options, workspace)?;
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
        Ok::<_, AppError>((reference, transport))
    })();
    let (reference, transport) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            if show_spinner {
                context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
            }
            return Err(error.with_context(milestone_create::CONTEXT));
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
                        context.write_stdout_with_policy(
                            spinner::frame(frame).as_bytes(),
                            OutputPolicy::ConsoleLike,
                        )?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        })
    } else {
        block_on_network(create)
    };
    if show_spinner {
        context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
    }
    let output = result.map_err(|error| error.with_context(milestone_create::CONTEXT))?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn dispatch_milestone_update(
    context: &mut AppContext<'_>,
    action: &cli::milestone::MilestoneUpdate,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    let id = &action.id;
    crate::refs::reject_linear_url(id, "a milestone UUID")
        .map_err(|error| error.with_context(milestone_update::CONTEXT))?;
    let sort_order = action.sort_order;
    let mut options = milestone_update::Options {
        name: action.name.clone(),
        description: action.description.clone(),
        target_date: action.target_date.clone(),
        sort_order,
        project_id: action.project.clone(),
    };
    // The source throws this outside its catch, before spinner/config/client.
    options.require_update()?;
    let show_spinner = spinner::enabled(
        false,
        context.stdout_tty,
        context.startup.settings.no_color == NoColor::Absent,
    );
    if show_spinner {
        context
            .write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
    }
    let prepared = (|| {
        let config = context.config()?;
        let credentials = context.credentials()?;

        let inputs = client::selection_inputs(&config.options, workspace)?;
        // Source constructs the client before validating an optional project.
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
        Ok::<_, AppError>((reference, transport))
    })();
    let (reference, transport) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            if show_spinner {
                context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
            }
            return Err(error.with_context(milestone_update::CONTEXT));
        }
    };
    let update = async {
        if let Some(reference) = reference {
            let original = options.project_id.as_ref().ok_or_else(|| {
                AppError::new(
                    AppErrorKind::Invariant,
                    "prepared project has no original reference",
                )
            })?;
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
                        context.write_stdout_with_policy(
                            spinner::frame(frame).as_bytes(),
                            OutputPolicy::ConsoleLike,
                        )?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        })
    } else {
        block_on_network(update)
    };
    if show_spinner {
        context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
    }
    let output = result.map_err(|error| error.with_context(milestone_update::CONTEXT))?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

/// None means the user confirmed; every other result is a completed command.
fn confirm_deletion(
    context: &mut AppContext<'_>,
    force: bool,
    message: &str,
) -> Result<Option<ExitStatus>, AppError> {
    use crate::platform::prompt::{PromptOutcome, PromptSession};
    if force {
        return Ok(None);
    }
    if !context.stdin_tty {
        return Err(AppError::new(
            AppErrorKind::Validation,
            "Interactive confirmation required",
        )
        .with_suggestion("Use --force to skip confirmation."));
    }
    let outcome = {
        let mut session = PromptSession::confirmation_stdio(&mut *context.stdout)?;
        let result = session.confirm(message, false);
        session.finish_result(result)?
    };
    match outcome {
        PromptOutcome::Submitted(true) => Ok(None),
        PromptOutcome::Submitted(false) => {
            context.write_stdout_with_policy(b"Deletion canceled\n", OutputPolicy::ConsoleLike)?;
            Ok(Some(ExitStatus::Success))
        }
        PromptOutcome::Interrupted => Ok(Some(ExitStatus::ChildCode(
            std::num::NonZeroU8::new(130).ok_or_else(|| {
                AppError::new(AppErrorKind::Invariant, "exit code 130 must be nonzero")
            })?,
        ))),
        PromptOutcome::EndOfInput => Err(AppError::new(
            AppErrorKind::Validation,
            "unexpected EOF while prompting for confirmation",
        )),
    }
}

fn dispatch_label_delete(
    context: &mut AppContext<'_>,
    action: &cli::label::LabelDelete,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    use crate::platform::prompt::{PromptOutcome, PromptSession};
    let result = (|| {
        let config = context.config()?;
        let credentials = context.credentials()?;
        let inputs = client::selection_inputs(&config.options, workspace)?;
        // Source creates the client before parsing/resolving an explicit team.
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
                if !context.stdin_tty {
                    return Err(AppError::new(
                        AppErrorKind::Validation,
                        format!("Multiple labels named \"{}\" found", action.name_or_id),
                    )
                    .with_suggestion("Use --team to disambiguate."));
                }
                let outcome = {
                    let mut session = PromptSession::stdin_stdio(&mut *context.stdout)?;
                    let result = label_delete::choose(&mut session, &action.name_or_id, &labels);
                    session.finish_result(result)?
                };
                match outcome {
                    PromptOutcome::Submitted(label) => label,
                    PromptOutcome::Interrupted => {
                        return Ok(ExitStatus::ChildCode(
                            std::num::NonZeroU8::new(130).ok_or_else(|| {
                                AppError::new(
                                    AppErrorKind::Invariant,
                                    "exit code 130 must be nonzero",
                                )
                            })?,
                        ));
                    }
                    PromptOutcome::EndOfInput => {
                        return Err(AppError::new(
                            AppErrorKind::Validation,
                            "unexpected EOF while selecting a label",
                        ));
                    }
                }
            }
        };
        if let Some(status) = confirm_deletion(
            context,
            action.force,
            &format!(
                "Are you sure you want to delete label \"{}\"?",
                label_delete::display(&label)
            ),
        )? {
            return Ok(status);
        }
        let show_spinner = spinner::enabled(
            false,
            context.stdout_tty,
            context.startup.settings.no_color == NoColor::Absent,
        );
        if show_spinner {
            context.write_stdout_with_policy(
                spinner::frame(0).as_bytes(),
                OutputPolicy::ConsoleLike,
            )?;
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
                        context.write_stdout_with_policy(spinner::frame(frame).as_bytes(), OutputPolicy::ConsoleLike)?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        });
        if show_spinner {
            context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
        }
        context.write_stdout_with_policy(&result?, OutputPolicy::ConsoleLike)?;
        Ok(ExitStatus::Success)
    })();
    result.map_err(|error| error.with_context(label_delete::CONTEXT))
}

fn dispatch_project_delete(
    context: &mut AppContext<'_>,
    action: &cli::project::ProjectDelete,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    let original = &action.project_id;
    if let Some(status) = confirm_deletion(
        context,
        action.force,
        &format!("Are you sure you want to delete project {original}?"),
    )? {
        return Ok(status);
    }
    let show_spinner = spinner::enabled(
        false,
        context.stdout_tty,
        context.startup.settings.no_color == NoColor::Absent,
    );
    if show_spinner {
        context
            .write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
    }
    let result = (|| {
        let config = context.config()?;
        let credentials = context.credentials()?;
        let inputs = client::selection_inputs(&config.options, workspace)?;
        // Source constructs its client before parsing any project URL.
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
        context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
    }
    let output = result.map_err(|error| error.with_context(project_delete::CONTEXT))?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn dispatch_milestone_delete(
    context: &mut AppContext<'_>,
    action: &cli::milestone::MilestoneDelete,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    let id = &action.id;
    crate::refs::reject_linear_url(id, "a milestone UUID")
        .map_err(|error| error.with_context(milestone_delete::CONTEXT))?;
    if let Some(status) = confirm_deletion(
        context,
        action.force,
        &format!("Are you sure you want to delete milestone {id}?"),
    )? {
        return Ok(status);
    }
    let show_spinner = spinner::enabled(
        false,
        context.stdout_tty,
        context.startup.settings.no_color == NoColor::Absent,
    );
    if show_spinner {
        context
            .write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
    }
    let result = (|| {
        let config = context.config()?;
        let credentials = context.credentials()?;
        let inputs = client::selection_inputs(&config.options, workspace)?;
        let transport = client::prepare_transport_with_inputs(
            &config.options,
            credentials,
            &inputs,
            &config.transport_env,
        )?;
        block_on_network(milestone_delete::submit(&transport, id))
    })();
    if show_spinner {
        context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
    }
    let output = result.map_err(|error| error.with_context(milestone_delete::CONTEXT))?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

/// These leaves have distinct unresolved diagnostics, while sharing the
/// maintained reference and real VCS inference implementation.
fn resolve_relation_reference(
    context: &AppContext<'_>,
    input: Option<&str>,
    workspace: Option<&str>,
    unresolved: impl FnOnce() -> AppError,
) -> Result<String, AppError> {
    let reference = match input {
        None => crate::refs::IssueReference::Inferred,
        Some(_) => {
            let config = context.config()?;
            let credentials = context.credentials()?;
            let inputs = client::selection_inputs(&config.options, workspace)?;
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
    context: &AppContext<'_>,
    workspace: Option<&str>,
) -> Result<crate::graphql::transport::GraphQlTransport, AppError> {
    let config = context.config()?;
    client::prepare_transport(
        &config.options,
        context.credentials()?,
        workspace,
        &config.transport_env,
    )
}

/// Source relations use a spinner; URL links do not. Clear it on every network
/// result before displaying the result or returning its contextual error.
fn relation_network(
    context: &mut AppContext<'_>,
    pending: impl std::future::Future<Output = Result<Vec<u8>, AppError>>,
) -> Result<Vec<u8>, AppError> {
    let enabled = spinner::enabled(
        false,
        context.stdout_tty,
        context.startup.settings.no_color == NoColor::Absent,
    );
    if !enabled {
        return block_on_network(pending);
    }
    context.write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
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
                    context.write_stdout_with_policy(spinner::frame(frame).as_bytes(), OutputPolicy::ConsoleLike)?;
                    frame = frame.wrapping_add(1);
                }
            }
        }
    });
    context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
    result
}

fn dispatch_issue_relation_list(
    context: &mut AppContext<'_>,
    action: &cli::issue::IssueRelationList,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    use crate::commands::issue_relations;
    let output = (|| {
        let identifier =
            resolve_relation_reference(context, action.issue_id.as_deref(), workspace, || {
                issue_details::unresolved(false)
            })?;
        let transport = relation_transport(context, workspace)?;
        relation_network(context, issue_relations::list(&transport, &identifier))
    })()
    .map_err(|error| error.with_context(issue_relations::LIST_CONTEXT))?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn prepare_relation_pair(
    context: &AppContext<'_>,
    a: &str,
    b: &str,
    workspace: Option<&str>,
) -> Result<(String, String), AppError> {
    let resolve = |input| {
        resolve_relation_reference(context, Some(input), workspace, || {
            AppError::new(
                AppErrorKind::Validation,
                format!("Could not resolve issue identifier: {input}"),
            )
        })
    };
    // Validate both references before creating the transport or looking up A.
    let a = resolve(a)?;
    let b = resolve(b)?;
    Ok((a, b))
}
fn dispatch_issue_relation_add(
    context: &mut AppContext<'_>,
    action: &cli::issue::IssueRelationAdd,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
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
    .map_err(|error| error.with_context(issue_relations::ADD_CONTEXT))?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}
fn dispatch_issue_relation_delete(
    context: &mut AppContext<'_>,
    action: &cli::issue::IssueRelationDelete,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
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
    .map_err(|error| error.with_context(issue_relations::DELETE_CONTEXT))?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}
fn dispatch_issue_link(
    context: &mut AppContext<'_>,
    action: &cli::issue::IssueLink,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    use crate::commands::issue_link;
    let output = (|| {
        let (input, url) = issue_link::inputs(&action.url_or_issue_id, action.url.as_deref())?;
        let identifier = resolve_relation_reference(context, input, workspace, ||
            AppError::new(AppErrorKind::Validation, "Could not determine issue ID").with_suggestion(
                "Please provide an issue ID like 'ENG-123', or run from a branch that contains an issue identifier."))?;
        let transport = relation_transport(context, workspace)?;
        block_on_network(issue_link::submit(&transport, &identifier, url, action.title.as_deref()))
    })().map_err(|error| error.with_context(issue_link::CONTEXT))?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn dispatch_issue_comment_list(
    context: &mut AppContext<'_>,
    action: &cli::issue::IssueCommentList,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    let id = resolve_issue(context, action.issue_id.as_deref(), workspace)
        .map_err(|error| error.with_context(issue_comment_list::CONTEXT))?;
    let config = context.config()?;
    let transport = client::prepare_transport(
        &config.options,
        context.credentials()?,
        workspace,
        &config.transport_env,
    )
    .map_err(|error| error.with_context(issue_comment_list::CONTEXT))?;
    let output = block_on_network(issue_comment_list::run(
        &transport,
        &id,
        &id,
        action.json,
        !context.no_color(),
    ))?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn dispatch_issue_comment_delete(
    context: &mut AppContext<'_>,
    action: &cli::issue::IssueCommentDelete,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    let id = &action.comment_id;
    // Both URL checks precede config and credentials, as in the source.
    crate::refs::reject_comment_url(id)
        .and_then(|()| crate::refs::reject_linear_url(id, "a comment UUID"))
        .map_err(|error| error.with_context(issue_comment_delete::CONTEXT))?;
    let transport = (|| {
        let config = context.config()?;
        let credentials = context.credentials()?;
        let inputs = client::selection_inputs(&config.options, workspace)?;
        client::prepare_transport_with_inputs(
            &config.options,
            credentials,
            &inputs,
            &config.transport_env,
        )
    })()
    .map_err(|error| error.with_context(issue_comment_delete::CONTEXT))?;
    let output = block_on_network(issue_comment_delete::submit(&transport, id))
        .map_err(|error| error.with_context(issue_comment_delete::CONTEXT))?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn write_completion_script(
    context: &mut AppContext<'_>,
    shell: CompletionShell,
    name: Option<&str>,
) -> Result<ExitStatus, AppError> {
    let script = completions::script(shell, name)?;
    context.write_stdout_with_policy(&script, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

pub fn write_final_error(
    context: &mut AppContext<'_>,
    error: &AppError,
) -> Result<ExitStatus, AppError> {
    if let Some(native) = error.native_parser_error() {
        let use_stderr = native.use_stderr();
        // clap was built without its color feature. Preserve its native
        // rendering rather than applying our handled-error ANSI wrapper.
        let text = native.render().to_string();
        if use_stderr {
            write_stderr(context, text.as_bytes())?;
        } else {
            write_stdout(context, text.as_bytes())?;
        }
        let code = u8::try_from(native.exit_code()).map_err(|error| {
            AppError::new(
                AppErrorKind::Invariant,
                "clap exit code does not fit process status",
            )
            .with_source(error)
        })?;
        return Ok(match code {
            0 => ExitStatus::Success,
            2 => ExitStatus::UsageFailure,
            _ => ExitStatus::ChildCode(std::num::NonZeroU8::new(code).ok_or_else(|| {
                AppError::new(
                    AppErrorKind::Invariant,
                    "nonzero clap exit code became zero",
                )
            })?),
        });
    }
    let color = context.handled_color();
    let line = format!("✗ {}", error.display_message());
    if color {
        write_stderr(context, format!("\x1b[31m{line}\x1b[39m\n").as_bytes())?;
    } else {
        write_stderr(context, format!("{line}\n").as_bytes())?;
    }
    if let Some(suggestion) = &error.suggestion {
        if color {
            write_stderr(
                context,
                format!("\x1b[90m  {suggestion}\x1b[39m\n").as_bytes(),
            )?;
        } else {
            write_stderr(context, format!("  {suggestion}\n").as_bytes())?;
        }
    }
    if context.debug_enabled()
        && matches!(error.kind, AppErrorKind::GraphQl | AppErrorKind::Transport)
    {
        if let Some(detail) = error.debug_detail() {
            write_stderr(context, format!("  debug: {detail}\n").as_bytes())?;
        }
        let mut source = error.source();
        while let Some(cause) = source {
            write_stderr(context, format!("  caused by: {cause}\n").as_bytes())?;
            source = cause.source();
        }
    }
    Ok(ExitStatus::HandledFailure)
}

/// Source order: body flags, project reference (a UUID needs no client), the
/// omitted-body prompt, then client construction before parent validation.
fn dispatch_project_comment_add(
    context: &mut AppContext<'_>,
    action: &cli::project::ProjectCommentAdd,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    let original = &action.project;
    let result = (|| {
        let body = comment_add::resolve_body(action.body.as_deref(), action.body_file.as_deref())?;
        let project_id = {
            let config = context.config()?;
            let credentials = context.credentials()?;
            let inputs = client::selection_inputs(&config.options, workspace)?;
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
            None => match prompt_comment_body(context)? {
                Ok(body) => body,
                Err(status) => return Ok(Err(status)),
            },
        };
        submit_comment(
            context,
            workspace,
            comment_add::CommentTarget::Project { project_id },
            body,
            action.parent.as_deref(),
        )
        .map(|comment| Ok(comment_add::output("project", original, &comment)))
    })();
    finish_comment_add(context, result)
}

/// Source order: body flags, initiative reference (a UUID needs no client),
/// the omitted-body prompt, then client construction before parent validation.
fn dispatch_initiative_comment_add(
    context: &mut AppContext<'_>,
    action: &cli::initiative::InitiativeCommentAdd,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    let original = &action.initiative;
    let result = (|| {
        let body = comment_add::resolve_body(action.body.as_deref(), action.body_file.as_deref())?;
        let initiative_id = {
            let config = context.config()?;
            let credentials = context.credentials()?;
            let inputs = client::selection_inputs(&config.options, workspace)?;
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
            None => match prompt_comment_body(context)? {
                Ok(body) => body,
                Err(status) => return Ok(Err(status)),
            },
        };
        submit_comment(
            context,
            workspace,
            comment_add::CommentTarget::Initiative { initiative_id },
            body,
            action.parent.as_deref(),
        )
        .map(|comment| Ok(comment_add::output("initiative", original, &comment)))
    })();
    finish_comment_add(context, result)
}

/// Source order: local document URL reduction, body flags, then the content
/// record lookup for every reference (UUIDs included), the omitted-body
/// prompt, and a second client before parent validation.
fn dispatch_document_comment_add(
    context: &mut AppContext<'_>,
    action: &cli::document::DocumentCommentAdd,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    let result = (|| {
        let document = {
            let config = context.config()?;
            let credentials = context.credentials()?;
            let inputs = client::selection_inputs(&config.options, workspace)?;
            resolve_document_reference(
                &action.document,
                &WorkspaceScope::from_selection(&inputs, credentials),
            )?
        };
        let body = comment_add::resolve_body(action.body.as_deref(), action.body_file.as_deref())?;
        let document_content_id = {
            let config = context.config()?;
            let credentials = context.credentials()?;
            let inputs = client::selection_inputs(&config.options, workspace)?;
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
            None => match prompt_comment_body(context)? {
                Ok(body) => body,
                Err(status) => return Ok(Err(status)),
            },
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
        .map(|comment| Ok(comment_add::output("document", &document, &comment)))
    })();
    finish_comment_add(context, result)
}

/// The source always prompts for an omitted body. Terminal cleanup completes
/// before any outcome, and a blank answer fails after submission.
fn prompt_comment_body(
    context: &mut AppContext<'_>,
) -> Result<Result<String, ExitStatus>, AppError> {
    use crate::platform::prompt::{PromptOutcome, PromptSession};
    let outcome = {
        let mut session = PromptSession::stdin_stdio(&mut *context.stdout)?;
        let result = comment_add::prompt_body(&mut session);
        session.finish_result(result)?
    };
    match outcome {
        PromptOutcome::Submitted(body) => comment_add::require_prompted(body).map(Ok),
        PromptOutcome::Interrupted => Ok(Err(ExitStatus::ChildCode(
            std::num::NonZeroU8::new(130).ok_or_else(|| {
                AppError::new(AppErrorKind::Invariant, "exit code 130 must be nonzero")
            })?,
        ))),
        PromptOutcome::EndOfInput => Err(AppError::new(
            AppErrorKind::Validation,
            "unexpected EOF while prompting for comment body",
        )),
    }
}

/// `createComment` constructs its client before building the input.
fn submit_comment(
    context: &AppContext<'_>,
    workspace: Option<&str>,
    target: comment_add::CommentTarget,
    body: String,
    parent: Option<&str>,
) -> Result<crate::graphql::operations::comment_create::CreatedComment, AppError> {
    let config = context.config()?;
    let credentials = context.credentials()?;
    let inputs = client::selection_inputs(&config.options, workspace)?;
    let transport = client::prepare_transport_with_inputs(
        &config.options,
        credentials,
        &inputs,
        &config.transport_env,
    )?;
    let input = comment_add::build_input(target, body, parent, None)?;
    block_on_network(comment_add::create(&transport, input))
}

fn finish_comment_add(
    context: &mut AppContext<'_>,
    result: Result<Result<Vec<u8>, ExitStatus>, AppError>,
) -> Result<ExitStatus, AppError> {
    match result.map_err(|error| error.with_context(comment_add::CONTEXT))? {
        Ok(output) => {
            context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
            Ok(ExitStatus::Success)
        }
        Err(status) => Ok(status),
    }
}

fn dispatch_initiative_comment_list(
    context: &mut AppContext<'_>,
    action: &cli::initiative::InitiativeCommentList,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    let json = action.json;
    let original = &action.initiative;
    let config = context.config()?;
    let credentials = context.credentials()?;

    let inputs = client::selection_inputs(&config.options, workspace)
        .map_err(|error| error.with_context(initiative_comment_list::CONTEXT))?;
    let reference = prepare_initiative_lookup(
        original,
        &WorkspaceScope::from_selection(&inputs, credentials),
    )
    .map_err(|error| error.with_context(initiative_comment_list::CONTEXT))?;
    let transport = client::prepare_transport_with_inputs(
        &config.options,
        credentials,
        &inputs,
        &config.transport_env,
    )
    .map_err(|error| error.with_context(initiative_comment_list::CONTEXT))?;
    let color = !context.no_color();
    let output = block_on_network(async {
        let id = resolve_initiative_with_transport(&reference, original, &transport)
            .await
            .map_err(|error| error.with_context(initiative_comment_list::CONTEXT))?;
        initiative_comment_list::run(&transport, original, &id, json, color).await
    })?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn dispatch_document_comment_list(
    context: &mut AppContext<'_>,
    action: &cli::document::DocumentCommentList,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    let json = action.json;
    let original = &action.document;
    let config = context.config()?;
    let credentials = context.credentials()?;

    let inputs = client::selection_inputs(&config.options, workspace)
        .map_err(|error| error.with_context(document_comment_list::CONTEXT))?;
    let reference = resolve_document_reference(
        original,
        &WorkspaceScope::from_selection(&inputs, credentials),
    )
    .map_err(|error| error.with_context(document_comment_list::CONTEXT))?;
    let transport = client::prepare_transport_with_inputs(
        &config.options,
        credentials,
        &inputs,
        &config.transport_env,
    )
    .map_err(|error| error.with_context(document_comment_list::CONTEXT))?;
    let color = !context.no_color();
    let output = block_on_network(async {
        document_comment_list::run(&transport, &reference, &reference, json, color).await
    })?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn dispatch_auth_token(
    context: &mut AppContext<'_>,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    let output = auth_token::run(
        &context.config()?.options,
        context.credentials()?,
        workspace,
    )?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn dispatch_auth_default(
    context: &mut AppContext<'_>,
    action: &cli::auth::AuthDefault,
) -> Result<ExitStatus, AppError> {
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
                if !io::stdin().is_terminal() {
                    return Err(auth_default::non_tty_error());
                }
                let outcome = {
                    let mut session = PromptSession::stdin_stdio(&mut *context.stdout)?;
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
                        return Ok(ExitStatus::ChildCode(
                            std::num::NonZeroU8::new(130).ok_or_else(|| {
                                AppError::new(
                                    AppErrorKind::Invariant,
                                    "exit code 130 must be nonzero",
                                )
                            })?,
                        ));
                    }
                    PromptOutcome::EndOfInput => {
                        return Err(AppError::new(
                            AppErrorKind::Validation,
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
                let startup = context.startup.result.as_ref().map_err(|_| {
                    AppError::new(AppErrorKind::Invariant, "default save after failed startup")
                })?;
                auth_default::save(
                    &startup.credentials,
                    &workspace,
                    startup.credentials_path.as_deref(),
                    &RealCredentialFileWriter,
                )?
            }
            DefaultAction::Select(_) => {
                return Err(AppError::new(
                    AppErrorKind::Invariant,
                    "submitted workspace did not resolve to a default action",
                ));
            }
        };
        context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
        Ok(ExitStatus::Success)
    })();
    result.map_err(|error| error.with_context(auth_default::CONTEXT))
}

fn dispatch_auth_list(
    context: &mut AppContext<'_>,
    _action: &cli::auth::AuthList,
    _workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    let config = context.config()?;
    let rows = auth_list::classify(context.credentials()?);
    let output = if rows.is_empty() {
        auth_list::EMPTY_OUTPUT.as_bytes().to_vec()
    } else {
        let prepared = auth_list::prepare_transports(
            rows,
            config.options.endpoint().value(),
            &config.transport_env,
        )
        .map_err(|error| error.with_context(auth_list::CONTEXT))?;
        let listed = block_on_network(auth_list::fetch(prepared))
            .map_err(|error| error.with_context(auth_list::CONTEXT))?;
        auth_list::render(&listed, context.stdout_tty && !context.no_color())
    };
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn dispatch_auth_whoami(
    context: &mut AppContext<'_>,
    _action: &cli::auth::AuthWhoami,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    let config = context.config()?;
    let credentials = context.credentials()?;

    let transport = auth_whoami::prepare_transport(
        &config.options,
        credentials,
        workspace,
        &config.transport_env,
    )?;
    let output = block_on_network(async move { auth_whoami::run(&transport).await })?;
    write_stdout(context, &output)?;
    Ok(ExitStatus::Success)
}

fn dispatch_team_list(
    context: &mut AppContext<'_>,
    action: &cli::team::TeamList,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    let flags = team_list::Options {
        json: action.json,
        web: action.web,
        app: action.app,
    };
    if flags.web || flags.app {
        let (url, opening) = team_list::web_opening(&context.config()?.options, flags.app)?;
        context.write_stdout_with_policy(&opening, OutputPolicy::ConsoleLike)?;
        team_list::open(&url, flags.app)?;
        return Ok(ExitStatus::Success);
    }
    let spinner = spinner::enabled(
        flags.json,
        context.stdout_tty,
        context.startup.settings.no_color == NoColor::Absent,
    );
    if spinner {
        context
            .write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
    }
    let prepared = (|| {
        let config = context.config()?;
        let credentials = context.credentials()?;

        client::prepare_transport(
            &config.options,
            credentials,
            workspace,
            &config.transport_env,
        )
        .map_err(|error| error.with_context("Failed to fetch teams"))
    })();
    let transport = match prepared {
        Ok(transport) => transport,
        Err(error) => {
            if spinner {
                context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
            }
            return Err(error);
        }
    };
    let columns = table::stdout_columns(context.stdout_tty);
    let color = context.stdout_tty && !context.no_color();
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
                        context.write_stdout_with_policy(
                            spinner::frame(frame).as_bytes(),
                            OutputPolicy::ConsoleLike,
                        )?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        })
    } else {
        block_on_network(async { team_list::run(&transport, flags.json, columns, color).await })
    };
    if spinner {
        context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
    }
    let output = output_result.map_err(|error| {
        if error.context.is_none() {
            error.with_context("Failed to fetch teams")
        } else {
            error
        }
    })?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn dispatch_label_create(
    context: &mut AppContext<'_>,
    action: &cli::label::LabelCreate,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    // Source builds the transport even before validating required fields.
    let transport = (|| {
        let config = context.config()?;
        client::prepare_transport(
            &config.options,
            context.credentials()?,
            workspace,
            &config.transport_env,
        )
    })()
    .map_err(|error| error.with_context(label_create::CONTEXT))?;
    let mut options = label_create::Options {
        name: action.name.clone(),
        color: action.color.clone(),
        description: action.description.clone(),
        team: action.team.clone(),
        interactive: action.interactive,
    };
    if label_create::should_prompt(&options, context.stdout_tty) {
        context.write_stdout_with_policy(label_create::PROMPT_HEADER, OutputPolicy::ConsoleLike)?;
        let configured_key = configured_team_key(&context.config()?.options);
        let mut session = crate::platform::prompt::PromptSession::stdio(&mut *context.stdout)?;
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
                        .map_err(|error| error.with_context(label_create::CONTEXT))?;
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
                return Ok(ExitStatus::ChildCode(
                    std::num::NonZeroU8::new(130).ok_or_else(|| {
                        AppError::new(AppErrorKind::Invariant, "exit code 130 must be nonzero")
                    })?,
                ));
            }
            crate::platform::prompt::PromptOutcome::EndOfInput => {
                return Err(AppError::new(
                    AppErrorKind::Validation,
                    "unexpected EOF while prompting for label",
                ));
            }
        }
    }
    label_create::validate(&options).map_err(|error| error.with_context(label_create::CONTEXT))?;
    let team_id = match options.team.as_deref().filter(|team| !team.is_empty()) {
        None => None,
        Some(team) => {
            let config = context.config()?;
            let inputs = client::selection_inputs(&config.options, workspace)?;
            let prepared = prepare_team_lookup(
                team,
                &WorkspaceScope::from_selection(&inputs, context.credentials()?),
            )
            .map_err(|error| error.with_context(label_create::CONTEXT))?;
            Some(
                block_on_network(resolve_team_with_transport(&prepared, &transport))
                    .map_err(|error| error.with_context(label_create::CONTEXT))?
                    .id,
            )
        }
    };
    let show_spinner = spinner::enabled(
        false,
        context.stdout_tty,
        context.startup.settings.no_color == NoColor::Absent,
    );
    if show_spinner {
        context
            .write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
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
                        context.write_stdout_with_policy(spinner::frame(frame).as_bytes(), OutputPolicy::ConsoleLike)?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        })
    } else {
        block_on_network(create)
    };
    if show_spinner {
        context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
    }
    let output = result.map_err(|error| error.with_context(label_create::CONTEXT))?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn dispatch_team_create(
    context: &mut AppContext<'_>,
    action: &cli::team::TeamCreate,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    let mut options = team_create::Options {
        name: action.name.clone(),
        description: action.description.clone(),
        key: action.key.clone(),
        private: action.private,
    };
    let interactive = team_create::interactive(action.no_interactive, context.stdout_tty);
    let mode = team_create::mode(&options, interactive);
    if mode == team_create::Mode::Prompt {
        context.write_stdout_with_policy(team_create::PROMPT_HEADER, OutputPolicy::ConsoleLike)?;
        let mut session = crate::platform::prompt::PromptSession::stdio(&mut *context.stdout)?;
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
                        cleanup.message.push_str(&format!(
                            "; prompt also failed: {}",
                            error.display_message()
                        ));
                        cleanup
                    }
                });
            }
        };
        match result {
            team_create::PromptResult::Complete => {}
            team_create::PromptResult::Interrupted => {
                return Ok(ExitStatus::ChildCode(
                    std::num::NonZeroU8::new(130).ok_or_else(|| {
                        AppError::new(AppErrorKind::Invariant, "exit code 130 must be nonzero")
                    })?,
                ));
            }
            team_create::PromptResult::EndOfInput => {
                return Err(AppError::new(
                    AppErrorKind::Validation,
                    "unexpected EOF while prompting for team",
                ));
            }
        }
    }
    let announcement = team_create::required_name(&options)
        .map(|name| team_create::announcement(name, mode))
        .map_err(|error| error.with_context(team_create::CONTEXT))?;
    context.write_stdout_with_policy(&announcement, OutputPolicy::ConsoleLike)?;
    // Only flag mode starts the spinner, after its progress line and before
    // the client is built; the catch path stops it before any error.
    let show_spinner = mode == team_create::Mode::Flags
        && interactive
        && spinner::enabled(
            false,
            context.stdout_tty,
            context.startup.settings.no_color == NoColor::Absent,
        );
    if show_spinner {
        context
            .write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
    }
    let prepared = (|| {
        let config = context.config()?;
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
                context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
            }
            return Err(error.with_context(team_create::CONTEXT));
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
                        context.write_stdout_with_policy(
                            spinner::frame(frame).as_bytes(),
                            OutputPolicy::ConsoleLike,
                        )?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        })
    } else {
        block_on_network(create)
    };
    if show_spinner {
        context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
    }
    let output = result.map_err(|error| error.with_context(team_create::CONTEXT))?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn dispatch_team_id(
    context: &mut AppContext<'_>,
    _action: &cli::team::TeamId,
    _workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    let text = team_id::render(context)?;
    context.write_stdout_with_policy(text.as_bytes(), OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn dispatch_team_members(
    context: &mut AppContext<'_>,
    action: &cli::team::TeamMembers,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    let flags = team_members::Options {
        all: action.all,
        json: action.json,
    };
    let explicit = action.team.as_ref().filter(|value| !value.is_empty());

    let show_spinner = spinner::enabled(
        flags.json,
        context.stdout_tty,
        context.startup.settings.no_color == NoColor::Absent,
    );
    let mut spinner_started = false;
    let fallback_key = if explicit.is_none() {
        Some(
            crate::commands::team_key::configured_team_key(&context.config()?.options)
                .ok_or_else(|| missing_team_key().with_context(team_members::CONTEXT))?,
        )
    } else {
        None
    };
    if show_spinner && fallback_key.is_some() {
        context
            .write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
        spinner_started = true;
    }

    // A missing local key fails before credential selection. Explicit
    // references are locally prepared before transport, and resolved
    // before the member-query spinner begins.
    let selected = (|| {
        let config = context.config()?;
        if let Some(reference) = explicit {
            let credentials = context.credentials()?;
            let inputs = client::selection_inputs(&config.options, workspace)?;
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
            let key = fallback_key.ok_or_else(|| {
                AppError::new(AppErrorKind::Invariant, "configured team key was lost")
            })?;
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
    let selected = selected.map_err(|error: AppError| {
        if error.context.is_none() {
            error.with_context(team_members::CONTEXT)
        } else {
            error
        }
    });
    if selected.is_err() && spinner_started {
        context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
    }
    let (team_key, transport) = selected?;
    if show_spinner && !spinner_started {
        context
            .write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
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
                        context.write_stdout_with_policy(
                            spinner::frame(frame).as_bytes(),
                            OutputPolicy::ConsoleLike,
                        )?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        })
    } else {
        block_on_network(team_members::run(&transport, &team_key, flags))
    };
    if spinner_started {
        context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
    }
    context.write_stdout_with_policy(&output_result?, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn dispatch_team_states(
    context: &mut AppContext<'_>,
    action: &cli::team::TeamStates,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    let json = action.json;
    let cli_workspace = workspace;
    let explicit = action.team.as_ref().filter(|value| !value.is_empty());
    let prepared = if let Some(reference) = explicit {
        let config = context.config()?;
        let api_key = ApiKeyInput::from_options(&config.options).map_err(|error| {
            AppError::new(AppErrorKind::Invariant, error.to_string())
                .with_context(team_states::CONTEXT)
        })?;
        let inputs = CredentialSelectionInputs {
            api_key,
            cli_workspace,
            sourced_workspace: config
                .options
                .workspace()
                .map(|resolved| (resolved.value().as_str(), resolved.source().clone())),
        };
        let scope = WorkspaceScope::from_selection(&inputs, context.credentials()?);
        Some(
            prepare_team_lookup(reference, &scope)
                .map_err(|error| error.with_context(team_states::CONTEXT))?,
        )
    } else {
        None
    };
    let configured_key = if prepared.is_none() {
        Some(
            configured_team_key(&context.config()?.options).ok_or_else(|| {
                AppError::new(
                    AppErrorKind::Validation,
                    "Could not determine team key from directory name",
                )
                .with_suggestion("Please specify a team key, name, or ID as an argument.")
                .with_context(team_states::CONTEXT)
            })?,
        )
    } else {
        None
    };
    let spinner = spinner::enabled(
        json,
        context.stdout_tty,
        context.startup.settings.no_color == NoColor::Absent,
    );
    if spinner && prepared.is_none() {
        context
            .write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
    }
    let transport = match (|| {
        let config = context.config()?;
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
                context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
            }
            return Err(error.with_context(team_states::CONTEXT));
        }
    };
    let team_key = match prepared {
        Some(prepared) => {
            block_on_network(async { resolve_team_with_transport(&prepared, &transport).await })
                .map_err(|error| error.with_context(team_states::CONTEXT))?
                .key
        }
        None => configured_key.ok_or_else(|| {
            AppError::new(AppErrorKind::Invariant, "configured team key disappeared")
        })?,
    };
    if spinner && explicit.is_some() {
        context
            .write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
    }
    let color = context.stdout_tty && !context.no_color();
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
                        context.write_stdout_with_policy(
                            spinner::frame(frame).as_bytes(),
                            OutputPolicy::ConsoleLike,
                        )?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        })
    } else {
        block_on_network(async { team_states::run(&transport, team_key, json, color).await })
    };
    if spinner {
        context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
    }
    let output = output_result.map_err(|error| error.with_context(team_states::CONTEXT))?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn dispatch_user_list(
    context: &mut AppContext<'_>,
    action: &cli::user::UserList,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    let include_disabled = action.all;
    let json = action.json;
    let show_spinner = spinner::enabled(
        json,
        context.stdout_tty,
        context.startup.settings.no_color == NoColor::Absent,
    );
    if show_spinner {
        context
            .write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
    }
    let prepared = (|| {
        let config = context.config()?;
        let credentials = context.credentials()?;

        client::prepare_transport(
            &config.options,
            credentials,
            workspace,
            &config.transport_env,
        )
        .map_err(|error| error.with_context(user_list::CONTEXT))
    })();
    let transport = match prepared {
        Ok(transport) => transport,
        Err(error) => {
            if show_spinner {
                context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
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
                        context.write_stdout_with_policy(
                            spinner::frame(frame).as_bytes(),
                            OutputPolicy::ConsoleLike,
                        )?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        })
    } else {
        block_on_network(async { user_list::run(&transport, include_disabled, json).await })
    };
    if show_spinner {
        context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
    }
    let output = output_result.map_err(|error| {
        if error.context.is_none() {
            error.with_context(user_list::CONTEXT)
        } else {
            error
        }
    })?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn dispatch_project_list(
    context: &mut AppContext<'_>,
    action: &cli::project::ProjectList,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
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
        let config = context.config()?;
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
            let inputs = client::selection_inputs(&config.options, cli_workspace)
                .map_err(|error| error.with_context(project_list::OPEN_CONTEXT))?;
            let transport = client::prepare_transport_with_inputs(
                &config.options,
                credentials,
                &inputs,
                &config.transport_env,
            )
            .map_err(|error| error.with_context(project_list::OPEN_CONTEXT))?;
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
            .map_err(|error| error.with_context(project_list::OPEN_CONTEXT))?
        } else {
            let workspace = configured_workspace.ok_or_else(|| {
                AppError::new(
                    AppErrorKind::Invariant,
                    "project browser workspace was not resolved",
                )
            })?;
            (workspace, configured_team)
        };
        let (url, line) = project_list::opening(&workspace, team_key.as_deref(), options.app);
        context.write_stdout_with_policy(&line, OutputPolicy::ConsoleLike)?;
        project_list::open(&url, options.app)?;
        return Ok(ExitStatus::Success);
    }

    let show_spinner = spinner::enabled(
        options.json,
        context.stdout_tty,
        context.startup.settings.no_color == NoColor::Absent,
    );
    if show_spinner {
        context
            .write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
    }
    let prepared = (|| {
        project_list::check_conflicting_flags(&options)?;
        let config = context.config()?;
        let credentials = context.credentials()?;
        let inputs = client::selection_inputs(&config.options, cli_workspace)?;
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
        Ok::<_, AppError>((team_lookup, transport))
    })();
    let (team_lookup, transport) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            if show_spinner {
                context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
            }
            return Err(error.with_context(project_list::FETCH_CONTEXT));
        }
    };
    let columns = crate::commands::table::stdout_columns(context.stdout_tty);
    let color = context.stdout_tty && !context.no_color();
    let configured_team = if options.all_teams {
        None
    } else {
        configured_team_key(&context.config()?.options)
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
                        context.write_stdout_with_policy(spinner::frame(frame).as_bytes(), OutputPolicy::ConsoleLike)?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        })
    } else {
        block_on_network(pending)
    };
    if show_spinner {
        context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
    }
    let output = result.map_err(|error| {
        if error.context.is_some() {
            error
        } else {
            error.with_context(project_list::FETCH_CONTEXT)
        }
    })?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn dispatch_cycle_list(
    context: &mut AppContext<'_>,
    action: &cli::cycle::CycleList,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    let json = action.json;
    let team_reference = match action.team.clone() {
        Some(explicit) => explicit,
        None => configured_team_key(&context.config()?.options).ok_or_else(|| {
            AppError::new(
                AppErrorKind::Validation,
                "Could not determine team key from directory name or team flag",
            )
            .with_context(cycle_list::CONTEXT)
        })?,
    };
    let selected = (|| {
        let config = context.config()?;
        let credentials = context.credentials()?;

        let inputs = client::selection_inputs(&config.options, workspace)?;
        let scope = WorkspaceScope::from_selection(&inputs, credentials);
        let prepared = prepare_team_lookup(&team_reference, &scope)?;
        let transport = client::prepare_transport_with_inputs(
            &config.options,
            credentials,
            &inputs,
            &config.transport_env,
        )?;
        Ok::<_, AppError>((prepared, transport))
    })()
    .map_err(|error| error.with_context(cycle_list::CONTEXT))?;
    let (prepared, transport) = selected;
    let team = block_on_network(async { resolve_team_with_transport(&prepared, &transport).await })
        .map_err(|error| error.with_context(cycle_list::CONTEXT))?;

    // Deno starts this spinner after the team lookup, and its catch
    // path leaves the last frame visible on a cycle-fetch error.
    let show_spinner = spinner::enabled(
        json,
        context.stdout_tty,
        context.startup.settings.no_color == NoColor::Absent,
    );
    if show_spinner {
        context
            .write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
    }
    let columns = table::stdout_columns(context.stdout_tty);
    let color = !context.no_color();
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
                        context.write_stdout_with_policy(
                            spinner::frame(frame).as_bytes(),
                            OutputPolicy::ConsoleLike,
                        )?;
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
        if error.context.is_none() {
            error.with_context(cycle_list::CONTEXT)
        } else {
            error
        }
    })?;
    if show_spinner {
        context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
    }
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn dispatch_cycle_view(
    context: &mut AppContext<'_>,
    action: &cli::cycle::CycleView,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    let reference = action.cycle_ref.clone();
    let json = action.json;
    let explicit_team = action.team.clone();
    let selected = (|| {
        let config = context.config()?;
        let credentials = context.credentials()?;

        let inputs = client::selection_inputs(&config.options, workspace)?;
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
                return Err(AppError::new(AppErrorKind::Invariant, "expected cycle URL"));
            }
            None => None,
        };
        let team_reference = explicit_team
            .or(url_team)
            .or_else(|| configured_team_key(&config.options))
            .ok_or_else(|| {
                AppError::new(
                    AppErrorKind::Validation,
                    "Could not determine team key from directory name or team flag",
                )
            })?;
        let prepared = prepare_team_lookup(&team_reference, &scope)?;
        let transport = client::prepare_transport_with_inputs(
            &config.options,
            credentials,
            &inputs,
            &config.transport_env,
        )?;
        Ok::<_, AppError>((url, prepared, transport))
    })()
    .map_err(|error| error.with_context(cycle_view::CONTEXT))?;
    let (url, prepared, transport) = selected;
    let team = block_on_network(async { resolve_team_with_transport(&prepared, &transport).await })
        .map_err(|error| error.with_context(cycle_view::CONTEXT))?;
    let cycle_id = block_on_network(async {
        cycle_view::resolve_id(&transport, &team.id, &reference, url.as_ref()).await
    })
    .map_err(|error| error.with_context(cycle_view::CONTEXT))?;
    let show_spinner = spinner::enabled(
        json,
        context.stdout_tty,
        context.startup.settings.no_color == NoColor::Absent,
    );
    if show_spinner {
        context
            .write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
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
                    result = &mut pending => break result.map_err(AppError::from),
                    _ = ticks.tick() => {
                        context.write_stdout_with_policy(
                            spinner::frame(frame).as_bytes(), OutputPolicy::ConsoleLike,
                        )?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        })
    } else {
        block_on_network(async {
            transport
                .send_request(&request)
                .await
                .map_err(AppError::from)
        })
    }
    .map_err(|error| error.with_context(cycle_view::CONTEXT))?;
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
        context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
    }
    let details =
        details.map_err(|error| AppError::from(error).with_context(cycle_view::CONTEXT))?;
    let cycle = details.cycle.ok_or_else(|| {
        AppError::not_found("Cycle", &reference).with_context(cycle_view::CONTEXT)
    })?;
    let output = if json {
        cycle_view::json(&cycle).map_err(|error| error.with_context(cycle_view::CONTEXT))?
    } else {
        let markdown = cycle_view::markdown(&cycle, chrono::Utc::now(), &chrono::Local)
            .map_err(|error| error.with_context(cycle_view::CONTEXT))?;
        let rendered = if context.stdout_tty {
            use std::num::NonZeroU16;
            let columns = u16::try_from(table::stdout_columns(true))
                .ok()
                .and_then(NonZeroU16::new)
                .unwrap_or(crate::platform::markdown_terminal::FALLBACK_COLUMNS);
            let options = crate::platform::markdown_terminal::RenderOptions::for_terminal(
                columns,
                context.startup.settings.no_color,
                true,
                None,
                crate::platform::markdown_terminal::HostSource::System,
            );
            crate::platform::markdown_terminal::render(&markdown, &options)
                .map_err(|error| error.with_context(cycle_view::CONTEXT))?
        } else {
            markdown
        };
        format!("{rendered}\n").into_bytes()
    };
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn dispatch_label_list(
    context: &mut AppContext<'_>,
    action: &cli::label::LabelList,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    let flags = label_list::Options {
        team: action.team.clone(),
        workspace_only: action.workspace_only,
        all: action.all,
        json: action.json,
    };
    let cli_workspace = workspace;
    let show_spinner = spinner::enabled(
        flags.json,
        context.stdout_tty,
        context.startup.settings.no_color == NoColor::Absent,
    );
    if show_spinner {
        context
            .write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
    }
    let prepared = (|| {
        let config = context.config()?;
        let credentials = context.credentials()?;
        let inputs = client::selection_inputs(&config.options, cli_workspace)?;
        let transport = client::prepare_transport_with_inputs(
            &config.options,
            credentials,
            &inputs,
            &config.transport_env,
        )?;
        let scope = WorkspaceScope::from_selection(&inputs, credentials);
        let configured_team = configured_team_key(&config.options);
        let selection = label_list::select(&flags, configured_team.as_deref(), &scope)?;
        Ok::<_, AppError>((transport, selection))
    })();
    let (transport, selection) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            if show_spinner {
                context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
            }
            return Err(if error.context.is_none() {
                error.with_context(label_list::CONTEXT)
            } else {
                error
            });
        }
    };
    let columns = table::stdout_columns(context.stdout_tty);
    let color = context.stdout_tty && !context.no_color();
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
                        context.write_stdout_with_policy(
                            spinner::frame(frame).as_bytes(),
                            OutputPolicy::ConsoleLike,
                        )?;
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
        context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
    }
    let output = output_result.map_err(|error| {
        if error.context.is_none() {
            error.with_context(label_list::CONTEXT)
        } else {
            error
        }
    })?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn dispatch_template_list(
    context: &mut AppContext<'_>,
    action: &cli::template::TemplateList,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    let json = action.json;
    let template_type = action.r#type.map(|value| match value {
        cli::TemplateType::Issue => template_list::TemplateType::Issue,
        cli::TemplateType::Project => template_list::TemplateType::Project,
        cli::TemplateType::Document => template_list::TemplateType::Document,
    });
    let team_reference = action.team.as_deref();
    let show_spinner = spinner::enabled(
        json,
        context.stdout_tty,
        context.startup.settings.no_color == NoColor::Absent,
    );
    if show_spinner {
        context
            .write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
    }
    let prepared = (|| {
        let config = context.config()?;
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
                context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
            }
            return Err(error);
        }
    };
    let columns = table::stdout_columns(context.stdout_tty);
    let color = context.stdout_tty && !context.no_color();
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
                        context.write_stdout_with_policy(
                            spinner::frame(frame).as_bytes(),
                            OutputPolicy::ConsoleLike,
                        )?;
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
        context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
    }
    let output = output_result.map_err(|error| {
        if error.context.is_none() {
            error.with_context(template_list::CONTEXT)
        } else {
            error
        }
    })?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn dispatch_template_view(
    context: &mut AppContext<'_>,
    action: &cli::template::TemplateView,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    let json = action.json;
    let reference = &action.template;
    // Deno starts this spinner before the URL check and credential
    // selection, and stops it before reporting either failure.
    let show_spinner = spinner::enabled(
        json,
        context.stdout_tty,
        context.startup.settings.no_color == NoColor::Absent,
    );
    if show_spinner {
        context
            .write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
    }
    let prepared = (|| {
        let config = context.config()?;
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
                context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
            }
            return Err(if error.context.is_none() {
                error.with_context(template_view::CONTEXT)
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
                        context.write_stdout_with_policy(
                            spinner::frame(frame).as_bytes(),
                            OutputPolicy::ConsoleLike,
                        )?;
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
        context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
    }
    let output = output_result.map_err(|error| {
        if error.context.is_none() {
            error.with_context(template_view::CONTEXT)
        } else {
            error
        }
    })?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn inferred_issue(context: &AppContext<'_>) -> Result<Option<String>, AppError> {
    let vcs = context
        .config()?
        .options
        .vcs()
        .map(|v| *v.value())
        .unwrap_or(crate::config::Vcs::Git);
    crate::platform::vcs::infer_issue(vcs, &context.cwd)
}

fn dispatch_issue_id(context: &mut AppContext<'_>) -> Result<ExitStatus, AppError> {
    let id = inferred_issue(context)
        .and_then(|id| id.ok_or_else(|| issue_details::unresolved(true)))
        .map_err(|e| e.with_context("Failed to get issue ID"))?;
    context.write_stdout_with_policy(format!("{id}\n").as_bytes(), OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
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
    context: &AppContext<'_>,
    input: Option<&str>,
    workspace: Option<&str>,
) -> Result<String, AppError> {
    let reference = if input.is_none() {
        crate::refs::IssueReference::Inferred
    } else {
        let config = context.config()?;
        let credentials = context.credentials()?;
        let inputs = client::selection_inputs(&config.options, workspace)?;
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
    context: &mut AppContext<'_>,
    input: Option<&str>,
    workspace: Option<&str>,
    field: IssueDetailField,
) -> Result<ExitStatus, AppError> {
    let id =
        resolve_issue(context, input, workspace).map_err(|e| e.with_context(field.context()))?;
    let show_spinner = spinner::enabled(
        false,
        context.stdout_tty,
        context.startup.settings.no_color == NoColor::Absent,
    );
    if show_spinner {
        context
            .write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
    }
    let result = (|| {
        let config = context.config()?;
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
                        context.write_stdout_with_policy(spinner::frame(frame).as_bytes(), OutputPolicy::ConsoleLike)?;
                        frame = frame.wrapping_add(1);
                    }
                }
            }
        })
    })();
    if show_spinner {
        context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
    }
    let detail = result.map_err(|e| e.with_context(field.context()))?;
    let value = match field {
        IssueDetailField::Title => detail.title,
        IssueDetailField::Url => detail.url,
    };
    context.write_stdout_with_policy(format!("{value}\n").as_bytes(), OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn agent_session_network<T>(
    context: &mut AppContext<'_>,
    json: bool,
    pending: impl Future<Output = Result<T, AppError>>,
) -> Result<T, AppError> {
    let enabled = spinner::enabled(
        json,
        context.stdout_tty,
        context.startup.settings.no_color == NoColor::Absent,
    );
    if !enabled {
        return block_on_network(pending);
    }
    context.write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
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
                    context.write_stdout_with_policy(spinner::frame(frame).as_bytes(), OutputPolicy::ConsoleLike)?;
                    frame = frame.wrapping_add(1);
                }
            }
        }
    });
    context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
    result
}

fn dispatch_agent_session_view(
    context: &mut AppContext<'_>,
    action: &cli::issue::IssueAgentSessionView,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
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
        let rendered = if context.stdout_tty {
            use std::num::NonZeroU16;
            let columns = u16::try_from(table::stdout_columns(true))
                .ok()
                .and_then(NonZeroU16::new)
                .unwrap_or(crate::platform::markdown_terminal::FALLBACK_COLUMNS);
            let options = crate::platform::markdown_terminal::RenderOptions::for_terminal(
                columns,
                context.startup.settings.no_color,
                true,
                None,
                crate::platform::markdown_terminal::HostSource::System,
            );
            crate::platform::markdown_terminal::render(&markdown, &options)?
        } else {
            markdown
        };
        Ok(format!("{rendered}\n").into_bytes())
    })()
    .map_err(|error: AppError| error.with_context(agent_session::VIEW_CONTEXT))?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn dispatch_agent_session_list(
    context: &mut AppContext<'_>,
    action: &cli::issue::IssueAgentSessionList,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
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
            table::stdout_columns(context.stdout_tty),
            !context.no_color(),
        ))
    })()
    .map_err(|error: AppError| error.with_context(agent_session::LIST_CONTEXT))?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

struct InitiativeAction<'a> {
    target: Option<&'a str>,
    force: bool,
    bulk: initiative_bulk::BulkInput<'a>,
}

fn initiative_prompt_confirm(
    context: &mut AppContext<'_>,
    message: &str,
    default: bool,
) -> Result<crate::platform::prompt::PromptOutcome<bool>, AppError> {
    use crate::platform::prompt::PromptSession;
    if !context.stdin_tty {
        return Err(AppError::new(
            AppErrorKind::Validation,
            "Interactive confirmation required. Use --force to skip.",
        ));
    }
    let mut session = PromptSession::confirmation_stdio(&mut *context.stdout)?;
    let result = session.confirm(message, default);
    session.finish_result(result)
}

fn initiative_prompt_stop<T>(
    outcome: crate::platform::prompt::PromptOutcome<T>,
) -> Result<T, AppError> {
    use crate::platform::prompt::PromptOutcome;
    match outcome {
        PromptOutcome::Submitted(value) => Ok(value),
        PromptOutcome::Interrupted => Err(AppError::new(AppErrorKind::Cancellation, "Interrupted")),
        PromptOutcome::EndOfInput => Err(AppError::new(
            AppErrorKind::Validation,
            "unexpected EOF while prompting for confirmation",
        )),
    }
}

fn initiative_interrupt_status() -> Result<ExitStatus, AppError> {
    Ok(ExitStatus::ChildCode(
        std::num::NonZeroU8::new(130).ok_or_else(|| {
            AppError::new(AppErrorKind::Invariant, "exit code 130 must be nonzero")
        })?,
    ))
}

fn dispatch_initiative_bulk(
    context: &mut AppContext<'_>,
    action: InitiativeAction<'_>,
    mode: initiative_bulk::Mode,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    use crate::platform::prompt::{PromptOutcome, PromptSession};
    // Source constructs the client before reading or validating collected IDs.
    let transport = {
        let config = context.config()?;
        let credentials = context.credentials()?;
        let inputs = client::selection_inputs(&config.options, workspace)?;
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
            return Err(AppError::new(
                AppErrorKind::Validation,
                format!("No initiative IDs provided for bulk {}.", mode.verb()),
            ));
        }
        context.write_stdout_with_policy(
            format!("Found {} initiative(s) to {}.\n", ids.len(), mode.verb()).as_bytes(),
            OutputPolicy::ConsoleLike,
        )?;
        if mode == initiative_bulk::Mode::Delete {
            context.write_stdout_with_policy(
                "\n⚠️  This action is PERMANENT and cannot be undone.\n\n".as_bytes(),
                OutputPolicy::ConsoleLike,
            )?;
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
                context
                    .write_stdout_with_policy(mode.bulk_cancelled(), OutputPolicy::ConsoleLike)?;
                return Ok(ExitStatus::Success);
            }
        }
        let targets = {
            let config = context.config()?;
            let credentials = context.credentials()?;
            let inputs = client::selection_inputs(&config.options, workspace)?;
            let scope = WorkspaceScope::from_selection(&inputs, credentials);
            ids.into_iter()
                .map(|id| initiative_bulk::Target::prepare(id, &scope))
                .collect()
        };
        let progress_enabled = spinner::enabled(
            false,
            context.stdout_tty,
            context.startup.settings.no_color == NoColor::Absent,
        );
        let results = block_on_network(initiative_bulk::execute(
            &transport,
            targets,
            mode,
            |progress| {
                if progress_enabled {
                    context
                        .write_stdout_with_policy(&progress.render(), OutputPolicy::ConsoleLike)?;
                }
                Ok(())
            },
        ))?;
        if progress_enabled {
            context.write_stdout_with_policy(
                initiative_bulk::PROGRESS_CLEAR,
                OutputPolicy::ConsoleLike,
            )?;
        }
        let (output, failed) = initiative_bulk::summary(&results, mode);
        context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
        return Ok(if failed {
            ExitStatus::HandledFailure
        } else {
            ExitStatus::Success
        });
    }
    let original = action
        .target
        .filter(|target| !target.is_empty())
        .ok_or_else(|| {
            AppError::new(
                AppErrorKind::Validation,
                "Initiative ID required. Use --bulk for multiple initiatives.",
            )
        })?;
    let target = {
        let config = context.config()?;
        let credentials = context.credentials()?;
        let inputs = client::selection_inputs(&config.options, workspace)?;
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
    .ok_or_else(|| AppError::not_found("Initiative", original))?;
    let detail = block_on_network(initiative_bulk::fetch_single(&transport, &id, mode))?
        .ok_or_else(|| AppError::not_found("Initiative", original))?;
    if detail.already_archived() {
        context.write_stdout_with_policy(
            format!("Initiative \"{}\" is already archived.\n", detail.name()).as_bytes(),
            OutputPolicy::ConsoleLike,
        )?;
        return Ok(ExitStatus::Success);
    }
    if let Some(warning) = detail.linked_warning() {
        context.write_stdout_with_policy(&warning, OutputPolicy::ConsoleLike)?;
    }
    if !action.force {
        // In source the nonTTY gate precedes the extra permanent warning.
        if !context.stdin_tty {
            return Err(AppError::new(
                AppErrorKind::Validation,
                "Interactive confirmation required. Use --force to skip.",
            ));
        }
        let (message, default) = match mode {
            initiative_bulk::Mode::Archive => {
                (format!("Archive initiative \"{}\"?", detail.name()), true)
            }
            initiative_bulk::Mode::Delete => {
                context.write_stdout_with_policy(
                    "\n⚠️  This action is PERMANENT and cannot be undone.\n\n".as_bytes(),
                    OutputPolicy::ConsoleLike,
                )?;
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
            context.write_stdout_with_policy(mode.single_cancelled(), OutputPolicy::ConsoleLike)?;
            return Ok(ExitStatus::Success);
        }
        if mode == initiative_bulk::Mode::Delete {
            // Keep raw input for JS trim semantics (FEFF yes, U+0085 no); the maintained
            // prompt owns terminal handling and its native rendering.
            let raw = std::cell::RefCell::new(String::new());
            let outcome = {
                let mut session = PromptSession::confirmation_stdio(&mut *context.stdout)?;
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
            if raw.into_inner().trim_matches(crate::text::js_space) != detail.name() {
                context.write_stdout_with_policy(
                    b"Name does not match. Delete cancelled.\n",
                    OutputPolicy::ConsoleLike,
                )?;
                return Ok(ExitStatus::Success);
            }
        }
    }
    let show_spinner = spinner::enabled(
        false,
        context.stdout_tty,
        context.startup.settings.no_color == NoColor::Absent,
    );
    if show_spinner {
        context
            .write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
    }
    let result = block_on_network(initiative_bulk::submit_single(
        &transport,
        &id,
        detail.name(),
        mode,
    ));
    if show_spinner {
        context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
    }
    context.write_stdout_with_policy(&result?, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

use crate::commands::{issue_upload, upload};
/// Issue source order: hidden id, body flags, identifier, all-file prevalidation,
/// sequential uploads with immediate output, line prompt only with no links,
/// client then parent validation then AddComment. No pre-target API lookup.
fn dispatch_issue_comment_add(
    context: &mut AppContext<'_>,
    action: &cli::issue::IssueCommentAdd,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
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
            return Err(AppError::new(
                AppErrorKind::Validation,
                "--public requires at least one --attach",
            )
            .with_suggestion("Add --attach <file> to upload, or remove --public."));
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
            match prompt_comment_body(context)? {
                Ok(body) => Some(body),
                Err(status) => return Ok(Err(status)),
            }
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
        Ok(Ok(issue_upload::comment_output(&identifier, &comment.url)))
    })();
    finish_comment_add(context, result)
}
fn dispatch_issue_attach(
    context: &mut AppContext<'_>,
    action: &cli::issue::IssueAttach,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    let output = (|| {
        let identifier = resolve_relation_reference(
            context,
            Some(&action.issue_id),
            workspace,
            issue_upload::unresolved,
        )?;
        upload::validate_file(std::path::Path::new(&action.filepath))?;
        let transport = relation_transport(context, workspace)?;
        let issue_uuid = block_on_network(issue_upload::lookup(&transport, &identifier))?;
        // Source public eligibility and size checks occur AFTER the UUID lookup.
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
    .map_err(|error: AppError| error.with_context(issue_upload::ATTACH_CONTEXT))?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}
/// File metadata/public validation before client/spinner; spinner only encloses
/// FileUpload+PUT. Every completed file is printed before later failures.
fn upload_issue_file(
    context: &mut AppContext<'_>,
    workspace: Option<&str>,
    path: &str,
    public: bool,
    transport_slot: &mut Option<crate::graphql::transport::GraphQlTransport>,
) -> Result<upload::UploadedFile, AppError> {
    let path = std::path::Path::new(path);
    let file = upload::prepare(path, public)?;
    if transport_slot.is_none() {
        *transport_slot = Some(relation_transport(context, workspace)?);
    }
    let Some(transport) = transport_slot.as_ref() else {
        unreachable!("transport initialized after metadata checks");
    };
    let show_spinner = context.stdout_tty && context.startup.settings.no_color == NoColor::Absent;
    let filename = file.filename.clone();
    let pending = upload::upload(transport, path, file);
    let uploaded = if show_spinner {
        // Source frames clear the line and reset color before the message.
        let frame = |tick: usize| format!("{}Uploading {filename}...", spinner::frame(tick));
        context.write_stdout_with_policy(frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
        let result = block_on_network(async {
            tokio::pin!(pending);
            let mut ticks = tokio::time::interval(spinner::TICK_INTERVAL);
            ticks.tick().await;
            let mut tick = 1;
            loop {
                tokio::select! {biased;result=&mut pending=>break result,_=ticks.tick()=>{context.write_stdout_with_policy(frame(tick).as_bytes(),OutputPolicy::ConsoleLike)?;tick=tick.wrapping_add(1);}}
            }
        });
        context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
        result?
    } else {
        block_on_network(pending)?
    };
    context.write_stdout_with_policy(&upload::output(&uploaded), OutputPolicy::ConsoleLike)?;
    if let Some(warning) = upload::warning(&uploaded) {
        write_stderr(context, &warning)?;
    }
    Ok(uploaded)
}

fn dispatch_document_list(
    context: &mut AppContext<'_>,
    action: &cli::document::DocumentList,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    use crate::commands::{document_list, document_target};
    let result = (|| {
        let config = context.config()?;
        let credentials = context.credentials()?;
        let inputs = client::selection_inputs(&config.options, workspace)?;
        let team = configured_team_key(&config.options);
        let target = document_target::prepare(
            action,
            &WorkspaceScope::from_selection(&inputs, credentials),
            team.as_deref(),
        )?;
        let first = i32::try_from(action.limit.get()).map_err(|error| {
            AppError::new(
                AppErrorKind::Validation,
                "Document limit exceeds GraphQL's signed integer range",
            )
            .with_source(error)
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
                context.stdout_tty && !context.no_color(),
                std::time::SystemTime::now(),
            )
            .into_bytes()
        };
        context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
        Ok(ExitStatus::Success)
    })();
    result.map_err(|error: AppError| error.with_context(document_list::CONTEXT))
}

fn dispatch_document_view(
    context: &mut AppContext<'_>,
    action: &cli::document::DocumentView,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    use crate::commands::document_view;
    use crate::platform::{markdown_assets, markdown_ast, markdown_serializer, markdown_terminal};
    let result = (|| {
        let config = context.config()?;
        let credentials = context.credentials()?;
        let inputs = client::selection_inputs(&config.options, workspace)?;
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
            context.write_stdout_with_policy(
                format!("Opening {} in web browser\n", document.url()).as_bytes(),
                OutputPolicy::ConsoleLike,
            )?;
            crate::platform::opener::open(document.url(), false)?;
            return Ok(ExitStatus::Success);
        }
        if action.json {
            context.write_stdout_with_policy(&document.json()?, OutputPolicy::ConsoleLike)?;
            return Ok(ExitStatus::Success);
        }
        let document = match document {
            document_view::DocumentResult::Body(document) => document,
            document_view::DocumentResult::WithComments(_) => {
                return Err(AppError::new(
                    AppErrorKind::Invariant,
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
                |bytes| context.write_stderr(bytes),
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
        let output = if action.raw || !context.stdout_tty {
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
                context.startup.settings.no_color,
                context.stdout_tty,
                hyperlink.as_deref(),
                markdown_terminal::HostSource::System,
            );
            format!("{}\n", markdown_terminal::render(&markdown, &options)?).into_bytes()
        };
        context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
        Ok(ExitStatus::Success)
    })();
    result.map_err(|error: AppError| error.with_context(document_view::CONTEXT))
}

fn document_fetch_with_spinner<T>(
    context: &mut AppContext<'_>,
    json: bool,
    pending: impl std::future::Future<Output = Result<T, AppError>>,
) -> Result<T, AppError> {
    let enabled = spinner::enabled(
        json,
        context.stdout_tty,
        context.startup.settings.no_color == NoColor::Absent,
    );
    if !enabled {
        return block_on_network(pending);
    }
    context.write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
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
                    context.write_stdout_with_policy(spinner::frame(frame).as_bytes(), OutputPolicy::ConsoleLike)?;
                    frame = frame.wrapping_add(1);
                }
            }
        }
    });
    context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
    result
}

fn delete_confirmation(
    context: &mut AppContext<'_>,
    message: &str,
    flag: &str,
) -> Result<crate::platform::prompt::PromptOutcome<bool>, AppError> {
    if !context.stdin_tty {
        return Err(AppError::new(
            AppErrorKind::Validation,
            "Interactive confirmation required",
        )
        .with_suggestion(format!("Use --{flag} to skip.")));
    }
    let mut session =
        crate::platform::prompt::PromptSession::confirmation_stdio(&mut *context.stdout)?;
    let outcome = session.confirm(message, false);
    session.finish_result(outcome)
}
fn dispatch_team_delete(
    context: &mut AppContext<'_>,
    action: &cli::team::TeamDelete,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    use crate::{
        commands::team_delete,
        graphql::operations::team_delete::GetTeamDetails,
        platform::prompt::{PlainOption, PlainSelect, PromptOutcome, PromptSession},
    };
    let result = (|| {
        let transport = relation_transport(context, workspace)?;
        let prepared = {
            let inputs = client::selection_inputs(&context.config()?.options, workspace)?;
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
                .map_err(AppError::from)
        })?;
        let team = details
            .team
            .ok_or_else(|| AppError::not_found("Team", &action.team))?;
        let count = team.issues.nodes.len();
        if count > 0 {
            let target = match action.move_issues.as_deref().filter(|s| !s.is_empty()) {
                Some(reference) => {
                    let prepared = {
                        let inputs =
                            client::selection_inputs(&context.config()?.options, workspace)?;
                        prepare_team_lookup(
                            reference,
                            &WorkspaceScope::from_selection(&inputs, context.credentials()?),
                        )?
                    };
                    let target =
                        block_on_network(resolve_team_with_transport(&prepared, &transport))?;
                    if target.id == source.id {
                        return Err(AppError::new(
                            AppErrorKind::Validation,
                            "Cannot move issues to the same team",
                        ));
                    }
                    target.id
                }
                None => {
                    context.write_stdout_with_policy(
                        &team_delete::warning(&team),
                        OutputPolicy::ConsoleLike,
                    )?;
                    if !context.stdin_tty {
                        return Err(AppError::new(
                            AppErrorKind::Validation,
                            "Interactive selection required",
                        )
                        .with_suggestion("Use --move-issues <teamKey> to specify target team."));
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
                        return Err(AppError::new(
                            AppErrorKind::GraphQl,
                            "No other teams available to move issues to",
                        ));
                    }
                    let outcome = {
                        let mut session = PromptSession::confirmation_stdio(&mut *context.stdout)?;
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
                                return Err(AppError::new(
                                    AppErrorKind::Invariant,
                                    "selected target team is missing",
                                ));
                            }
                            id
                        }
                        PromptOutcome::Interrupted => return initiative_interrupt_status(),
                        PromptOutcome::EndOfInput => {
                            return Err(AppError::new(
                                AppErrorKind::Validation,
                                "unexpected EOF while selecting a team",
                            ));
                        }
                    }
                }
            };
            team_delete_moves(context, &transport, &source.id, &target, count)
                .map_err(|e| e.with_context(team_delete::MOVE_CONTEXT))?;
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
                    context.write_stdout_with_policy(
                        b"Delete cancelled.\n",
                        OutputPolicy::ConsoleLike,
                    )?;
                    return Ok(ExitStatus::Success);
                }
                PromptOutcome::Interrupted => return initiative_interrupt_status(),
                PromptOutcome::EndOfInput => {
                    return Err(AppError::new(
                        AppErrorKind::Validation,
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
                    .map_err(AppError::from)
            })?;
        if !result.team_delete.success {
            return Err(AppError::new(
                AppErrorKind::GraphQl,
                "Failed to delete team",
            ));
        }
        context
            .write_stdout_with_policy(&team_delete::deleted(&team), OutputPolicy::ConsoleLike)?;
        Ok(ExitStatus::Success)
    })();
    result.map_err(|error: AppError| {
        if error.context.as_deref() == Some(team_delete::MOVE_CONTEXT) {
            error
        } else {
            error.with_context(team_delete::CONTEXT)
        }
    })
}
fn team_delete_moves(
    context: &mut AppContext<'_>,
    transport: &crate::graphql::transport::GraphQlTransport,
    source: &str,
    target: &str,
    count: usize,
) -> Result<(), AppError> {
    use crate::commands::team_delete;
    let show = spinner::enabled(
        false,
        context.stdout_tty,
        context.startup.settings.no_color == NoColor::Absent,
    );
    let message = std::cell::RefCell::new(format!("Moving {count} issue(s) to target team..."));
    let pending = async {
        let issues = team_delete::all_issues(source, |request| async move {
            transport.execute(&request).await.map_err(AppError::from)
        })
        .await?;
        team_delete::move_all(
            &issues,
            target,
            |request| async move { transport.execute(&request).await.map_err(AppError::from) },
            |moved, total| {
                *message.borrow_mut() = format!("Moving issues... ({moved}/{total})");
                Ok(())
            },
        )
        .await
    };
    let result = if show {
        context.write_stdout_with_policy(
            format!("{}{}", spinner::frame(0), message.borrow()).as_bytes(),
            OutputPolicy::ConsoleLike,
        )?;
        block_on_network(async {
            tokio::pin!(pending);
            let mut ticks = tokio::time::interval(spinner::TICK_INTERVAL);
            ticks.tick().await;
            let mut frame = 1;
            loop {
                tokio::select! {biased;result=&mut pending=>break result,_=ticks.tick()=>{context.write_stdout_with_policy(format!("{}{}",spinner::frame(frame),message.borrow()).as_bytes(),OutputPolicy::ConsoleLike)?;frame=frame.wrapping_add(1);}}
            }
        })
    } else {
        block_on_network(pending)
    };
    if show {
        context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?
    }
    let moved = result?;
    context.write_stdout_with_policy(
        format!("✓ Moved {moved} issue(s) to target team\n").as_bytes(),
        OutputPolicy::ConsoleLike,
    )
}
fn dispatch_document_delete(
    context: &mut AppContext<'_>,
    action: &cli::document::DocumentDelete,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
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
                return Err(AppError::new(
                    AppErrorKind::Validation,
                    "No document IDs provided for bulk delete",
                ));
            }
            context.write_stdout_with_policy(
                format!("Found {} document(s) to delete.\n", ids.len()).as_bytes(),
                OutputPolicy::ConsoleLike,
            )?;
            if !action.yes {
                match delete_confirmation(
                    context,
                    &format!("Delete {} document(s)?", ids.len()),
                    "yes",
                )? {
                    PromptOutcome::Submitted(true) => {}
                    PromptOutcome::Submitted(false) => {
                        context.write_stdout_with_policy(
                            b"Bulk delete cancelled.\n",
                            OutputPolicy::ConsoleLike,
                        )?;
                        return Ok(ExitStatus::Success);
                    }
                    PromptOutcome::Interrupted => return initiative_interrupt_status(),
                    PromptOutcome::EndOfInput => {
                        return Err(AppError::new(
                            AppErrorKind::Validation,
                            "unexpected EOF while prompting for confirmation",
                        ));
                    }
                }
            }
            let targets = {
                let inputs = client::selection_inputs(&context.config()?.options, workspace)?;
                let scope = WorkspaceScope::from_selection(&inputs, context.credentials()?);
                ids.into_iter()
                    .map(|id| command::Target::prepare(id, &scope))
                    .collect()
            };
            let show = spinner::enabled(
                false,
                context.stdout_tty,
                context.startup.settings.no_color == NoColor::Absent,
            );
            let results = block_on_network(command::execute(&transport, targets, |progress| {
                if show {
                    context
                        .write_stdout_with_policy(&progress.render(), OutputPolicy::ConsoleLike)?
                }
                Ok(())
            }));
            if show {
                context.write_stdout_with_policy(
                    initiative_bulk::PROGRESS_CLEAR,
                    OutputPolicy::ConsoleLike,
                )?
            }
            let (output, failed) = command::summary(&results?);
            context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
            return Ok(if failed {
                ExitStatus::HandledFailure
            } else {
                ExitStatus::Success
            });
        }
        let original = action
            .document_id
            .as_deref()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| {
                AppError::new(AppErrorKind::Validation, "Document ID required")
                    .with_suggestion("Use --bulk for multiple documents.")
            })?;
        let id = {
            let inputs = client::selection_inputs(&context.config()?.options, workspace)?;
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
                    context.write_stdout_with_policy(
                        b"Delete cancelled.\n",
                        OutputPolicy::ConsoleLike,
                    )?;
                    return Ok(ExitStatus::Success);
                }
                PromptOutcome::Interrupted => return initiative_interrupt_status(),
                PromptOutcome::EndOfInput => {
                    return Err(AppError::new(
                        AppErrorKind::Validation,
                        "unexpected EOF while prompting for confirmation",
                    ));
                }
            }
        }
        let output = block_on_network(command::submit_single(&transport, &document))?;
        context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
        Ok(ExitStatus::Success)
    })();
    result.map_err(|error: AppError| error.with_context(command::CONTEXT))
}

fn document_write_target(
    context: &AppContext<'_>,
    target: crate::commands::document_target::TargetOptions<'_>,
    workspace: Option<&str>,
) -> Result<
    (
        crate::graphql::transport::GraphQlTransport,
        Option<crate::commands::document_target::PreparedTarget>,
    ),
    AppError,
> {
    let config = context.config()?;
    let credentials = context.credentials()?;
    let inputs = client::selection_inputs(&config.options, workspace)?;
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
) -> Result<Result<crate::commands::document_write::Fields, ExitStatus>, AppError> {
    use crate::platform::prompt::PromptOutcome;
    match outcome {
        PromptOutcome::Submitted(fields) => Ok(Ok(fields)),
        PromptOutcome::Interrupted => Ok(Err(ExitStatus::ChildCode(
            std::num::NonZeroU8::new(130).ok_or_else(|| {
                AppError::new(AppErrorKind::Invariant, "exit code 130 must be nonzero")
            })?,
        ))),
        PromptOutcome::EndOfInput => Err(AppError::new(
            AppErrorKind::Validation,
            "unexpected EOF while prompting for document",
        )),
    }
}
fn dispatch_document_create(
    context: &mut AppContext<'_>,
    action: &cli::document::DocumentCreate,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    use crate::commands::{
        document_content, document_target::TargetOptions, document_write as command,
    };
    let result = (|| {
        let target = TargetOptions {
            project: action.project.as_deref(),
            issue: action.issue.as_deref(),
            initiative: action.initiative.as_deref(),
            team: action.team.as_deref(),
            cycle: action.cycle.as_deref(),
            release: action.release.as_deref(),
        };
        let interactive = context.stdout_tty
            && (action.interactive
                || (action.title.is_none()
                    && action.content.is_none()
                    && action.content_file.is_none()
                    && action.icon.is_none()
                    && !target.any()));
        let root = std::env::temp_dir();
        let fields = if interactive {
            if target.any() {
                return Err(AppError::new(AppErrorKind::Validation,"Attachment target flags cannot be combined with interactive mode").with_suggestion("Drop the target flags to choose the attachment interactively, or drop -i/--interactive to use the flags."));
            }
            let config = context.config()?;
            let env = config.child_env.clone();
            let default_team = configured_team_key(&config.options);
            crate::platform::prompt_text::TextOptions {
                minimum_utf16_length: 0,
                default: default_team.as_deref(),
            }
            .preflight()
            .map_err(|reason| AppError::new(AppErrorKind::Validation, reason))?;
            let mut session = crate::platform::prompt::PromptSession::stdio(&mut *context.stdout)?;
            let prompted = command::prompt(
                &mut session,
                &mut *context.stderr,
                command::PromptSettings {
                    env: &env,
                    temp_root: &root,
                    default_team: default_team.as_deref(),
                },
            );
            let outcome = session.finish_result(prompted)?;
            match document_prompt_exit(outcome)? {
                Ok(fields) => fields,
                Err(exit) => return Ok(exit),
            }
        } else {
            let title = action.title.clone().ok_or_else(|| {
                AppError::new(AppErrorKind::Validation, "Title is required")
                    .with_suggestion("Use --title or run with -i for interactive mode.")
            })?;
            target.cardinality(true)?;
            let content = if let Some(content) = &action.content {
                Some(content.clone())
            } else if let Some(path) = &action.content_file {
                Some(command::file(path, false)?)
            } else if !context.stdin_tty {
                document_content::optional_stdin(std::io::stdin())?
            } else if context.stdout_tty {
                context.write_stdout_with_policy(
                    b"Opening editor for document content...\n",
                    OutputPolicy::ConsoleLike,
                )?;
                let env = context.config()?.child_env.clone();
                let content = command::optional_editor(&env, &root, &mut *context.stderr)?;
                if content.is_none() {
                    context.write_stdout_with_policy(
                        b"No content entered. Creating document without content.\n",
                        OutputPolicy::ConsoleLike,
                    )?;
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
                .ok_or_else(|| AppError::new(AppErrorKind::Validation, "Title is required"))?;
            command::create(&transport, title, input).await
        })?;
        context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
        Ok(ExitStatus::Success)
    })();
    result.map_err(|error: AppError| error.with_context("Failed to create document"))
}
fn dispatch_document_update(
    context: &mut AppContext<'_>,
    action: &cli::document::DocumentUpdate,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    use crate::commands::{
        document_content, document_target::TargetOptions, document_write as command,
    };
    let result = (|| {
        let config = context.config()?;
        let credentials = context.credentials()?;
        let inputs = client::selection_inputs(&config.options, workspace)?;
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
            context.write_stdout_with_policy(
                format!("Opening {} in editor...\n", document.title).as_bytes(),
                OutputPolicy::ConsoleLike,
            )?;
            let content = command::required_editor(
                &context.config()?.child_env,
                &std::env::temp_dir(),
                &seed,
            )?;
            let Some(content) = content else {
                context.write_stdout_with_policy(
                    b"No changes made, update cancelled.\n",
                    OutputPolicy::ConsoleLike,
                )?;
                return Ok(ExitStatus::Success);
            };
            if content == seed {
                context.write_stdout_with_policy(
                    b"No changes detected, update cancelled.\n",
                    OutputPolicy::ConsoleLike,
                )?;
                return Ok(ExitStatus::Success);
            }
            Some(content)
        } else if !context.stdin_tty && !command::has_fields(&input) {
            document_content::optional_stdin(std::io::stdin())?
        } else {
            None
        };
        if !command::has_fields(&input) {
            return Err(AppError::new(AppErrorKind::Validation,"No update fields provided").with_suggestion("Use --title, --content, --content-file, --icon, --edit, or re-point the attachment with --project, --issue, --initiative, --team, --cycle, or --release."));
        }
        let output = block_on_network(async {
            if input.content.is_some() && !action.force {
                command::guard(&transport, &id).await?;
            }
            command::update(&transport, &id, input).await
        })?;
        context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
        Ok(ExitStatus::Success)
    })();
    result.map_err(|error: AppError| error.with_context("Failed to update document"))
}

fn project_ticks<T>(
    context: &mut AppContext<'_>,
    pending: impl std::future::Future<Output = Result<T, AppError>>,
    enabled: bool,
) -> Result<T, AppError> {
    if !enabled {
        return block_on_network(pending);
    }
    block_on_network(async {
        tokio::pin!(pending);
        let mut ticks = tokio::time::interval(spinner::TICK_INTERVAL);
        ticks.tick().await;
        let mut frame = 1_usize;
        loop {
            tokio::select! {biased;result=&mut pending=>break result,_=ticks.tick()=>{context.write_stdout_with_policy(spinner::frame(frame).as_bytes(),OutputPolicy::ConsoleLike)?;frame=frame.wrapping_add(1);}}
        }
    })
}
fn dispatch_project_create(
    context: &mut AppContext<'_>,
    action: &cli::project::ProjectCreate,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    use crate::commands::project_create as command;
    let result = (|| {
        // Original content/priority validation before authentication.
        let mut fields = command::local(action)?;
        let (options, default_workspace, transport) = {
            let config = context.config()?;
            let credentials = context.credentials()?;
            let options = config.options.clone();
            let inputs = client::selection_inputs(&options, workspace)?;
            let transport = client::prepare_transport_with_inputs(
                &options,
                credentials,
                &inputs,
                &config.transport_env,
            )?;
            (options, credentials.default().map(str::to_owned), transport)
        };
        let inputs = client::selection_inputs(&options, workspace)?;
        let scope = WorkspaceScope {
            cli_workspace: inputs.cli_workspace,
            sourced_workspace: inputs.sourced_workspace.as_ref().map(|(value, _)| *value),
            default_workspace: default_workspace.as_deref(),
            api_key: &inputs.api_key,
        };
        let default_team = configured_team_key(&options);
        if command::interactive(&fields, action.interactive, context.stdout_tty) {
            // Stdout-only source gate. Genuine stdin-pipe qualification/refusal remains
            // pending; never inherit doc's body-stdin/editor branches or CI gate.
            context.write_stdout_with_policy(
                b"\nCreate a new project\n\n",
                OutputPolicy::ConsoleLike,
            )?;
            let mut session = crate::platform::prompt::PromptSession::stdio(&mut *context.stdout)?;
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
                    return Err(AppError::new(
                        AppErrorKind::Validation,
                        "unexpected EOF while prompting for project",
                    ));
                }
            };
        }
        let input = block_on_network(command::input(
            &transport,
            &scope,
            &fields,
            default_team.as_deref(),
        ))?;
        let enabled = spinner::enabled(
            action.json,
            context.stdout_tty,
            context.startup.settings.no_color == NoColor::Absent,
        );
        if enabled {
            context.write_stdout_with_policy(
                spinner::frame(0).as_bytes(),
                OutputPolicy::ConsoleLike,
            )?;
        }
        let created = project_ticks(context, command::submit(&transport, input), enabled);
        if enabled {
            context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
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
        context.write_stderr(&output.stderr)?;
        context.write_stdout_with_policy(&output.stdout, OutputPolicy::ConsoleLike)?;
        Ok(ExitStatus::Success)
    })();
    result.map_err(|error: AppError| error.with_context("Failed to create project"))
}
fn dispatch_project_update(
    context: &mut AppContext<'_>,
    action: &cli::project::ProjectUpdate,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    use crate::commands::project_update as command;
    let options = command::Options::from_cli(action);
    // Local input/files/date checks before spinner and client, with source order.
    let local =
        command::local(&options).map_err(|error| error.with_context("Failed to update project"))?;
    let enabled = spinner::enabled(
        false,
        context.stdout_tty,
        context.startup.settings.no_color == NoColor::Absent,
    );
    if enabled {
        context
            .write_stdout_with_policy(spinner::frame(0).as_bytes(), OutputPolicy::ConsoleLike)?;
    }
    let result = (|| {
        let (config_options, default_workspace, transport) = {
            let config = context.config()?;
            let credentials = context.credentials()?;
            let config_options = config.options.clone();
            let inputs = client::selection_inputs(&config_options, workspace)?;
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
        let inputs = client::selection_inputs(&config_options, workspace)?;
        let scope = WorkspaceScope {
            cli_workspace: inputs.cli_workspace,
            sourced_workspace: inputs.sourced_workspace.as_ref().map(|(value, _)| *value),
            default_workspace: default_workspace.as_deref(),
            api_key: &inputs.api_key,
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
        context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
    }
    let project =
        result.map_err(|error: AppError| error.with_context("Failed to update project"))?;
    context.write_stdout_with_policy(
        &command::output(project.as_ref()),
        OutputPolicy::ConsoleLike,
    )?;
    Ok(ExitStatus::Success)
}

struct IssueArchiveDeleteAction<'a> {
    target: Option<&'a str>,
    confirm: bool,
    bulk: initiative_bulk::BulkInput<'a>,
}
fn issue_archive_delete_confirm(
    context: &mut AppContext<'_>,
    message: &str,
) -> Result<crate::platform::prompt::PromptOutcome<bool>, AppError> {
    use crate::platform::prompt::PromptSession;
    if !context.stdin_tty {
        return Err(AppError::new(
            AppErrorKind::Validation,
            "Interactive confirmation required",
        )
        .with_suggestion("Use --confirm to skip."));
    }
    if crate::commands::issue_archive_delete::stdout_is_pipe()? {
        return Err(AppError::new(
            AppErrorKind::Validation,
            "Cannot confirm while stdout is a pipe; pass --confirm to continue.",
        ));
    }
    let mut session = PromptSession::confirmation_stdio(&mut *context.stdout)?;
    let result = session.confirm(message, false);
    session.finish_result(result)
}
fn dispatch_issue_archive_delete(
    context: &mut AppContext<'_>,
    action: IssueArchiveDeleteAction<'_>,
    mode: crate::commands::issue_archive_delete::Mode,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    use crate::commands::issue_archive_delete as command;
    use crate::platform::prompt::PromptOutcome;
    // Both source leaves construct the client before any local collection/resolution.
    let transport = relation_transport(context, workspace)?;
    if action.bulk.requested() {
        if mode == command::Mode::Archive && action.target.is_some() {
            return Err(AppError::new(AppErrorKind::Validation,"Cannot combine a positional issue ID with --bulk").with_suggestion("Pass every identifier through --bulk (or --bulk-file / --bulk-stdin), or drop the positional one."));
        }
        let ids = initiative_bulk::collect_ids_with_policy(
            &action.bulk,
            &mut std::io::stdin().lock(),
            initiative_bulk::TextPolicy::Lossy,
        )?;
        if ids.is_empty() {
            return Err(AppError::new(
                AppErrorKind::Validation,
                format!("No issue identifiers provided for bulk {}", mode.verb()),
            ));
        }
        context.write_stdout_with_policy(
            format!("Found {} issue(s) to {}.\n", ids.len(), mode.verb()).as_bytes(),
            OutputPolicy::ConsoleLike,
        )?;
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
                context.write_stdout_with_policy(
                    format!("Bulk {} cancelled.\n", mode.verb()).as_bytes(),
                    OutputPolicy::ConsoleLike,
                )?;
                return Ok(ExitStatus::Success);
            }
        }
        let targets = {
            let config = context.config()?;
            let credentials = context.credentials()?;
            let inputs = client::selection_inputs(&config.options, workspace)?;
            let scope = WorkspaceScope::from_selection(&inputs, credentials);
            let team = configured_team_key(&config.options);
            ids.into_iter()
                .map(|id| command::Target::prepare(id, team.as_deref(), &scope))
                .collect()
        };
        let progress_enabled = spinner::enabled(
            false,
            context.stdout_tty,
            context.startup.settings.no_color == NoColor::Absent,
        );
        let results = block_on_network(command::execute(&transport, targets, mode, |progress| {
            if progress_enabled {
                context.write_stdout_with_policy(&progress.render(), OutputPolicy::ConsoleLike)?;
            }
            Ok(())
        }))?;
        if progress_enabled {
            context.write_stdout_with_policy(
                initiative_bulk::PROGRESS_CLEAR,
                OutputPolicy::ConsoleLike,
            )?;
        }
        let (output, failed) = command::summary(&results, mode);
        context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
        return Ok(if failed {
            ExitStatus::HandledFailure
        } else {
            ExitStatus::Success
        });
    }
    if mode == command::Mode::Delete && action.target.is_none_or(str::is_empty) {
        return Err(AppError::new(AppErrorKind::Validation, "Issue ID required")
            .with_suggestion("Use --bulk for multiple issues."));
    }
    let id = resolve_relation_reference(context, action.target, workspace, || match mode {
        command::Mode::Archive => {
            AppError::new(AppErrorKind::Validation, "Could not determine issue ID")
                .with_suggestion("Please provide an issue ID like 'ENG-123'.")
        }
        command::Mode::Delete => AppError::not_found("Issue", action.target.unwrap_or_default()),
    })?;
    let details = block_on_network(command::single_details(&transport, &id, mode))?;
    if details.already_archived {
        context.write_stdout_with_policy(
            format!("Issue \"{}\" is already archived.\n", details.name()).as_bytes(),
            OutputPolicy::ConsoleLike,
        )?;
        return Ok(ExitStatus::Success);
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
            context.write_stdout_with_policy(
                format!(
                    "{} cancelled.\n",
                    match mode {
                        command::Mode::Archive => "Archive",
                        command::Mode::Delete => "Delete",
                    }
                )
                .as_bytes(),
                OutputPolicy::ConsoleLike,
            )?;
            return Ok(ExitStatus::Success);
        }
    }
    let output = block_on_network(command::submit_single(&transport, &id, &details, mode))?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

struct UpdateCreateAction<'a> {
    original: &'a str,
    body: Option<&'a str>,
    file: Option<&'a str>,
    health: Option<&'a str>,
    interactive: bool,
}
fn dispatch_update_create(
    context: &mut AppContext<'_>,
    action: UpdateCreateAction<'_>,
    mode: crate::commands::update_create::Mode,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    use crate::commands::update_create as command;
    use crate::platform::prompt::PromptOutcome;
    let result = (|| {
        let interactive = command::attended(
            action.interactive,
            context.stdin_tty,
            context.stdout_tty,
            action.body,
            action.file,
            action.health,
        )?;
        // Approved explicit-i preflight precedes both clients. Otherwise source client-first.
        let transport = relation_transport(context, workspace)?;
        let (id, display) = {
            let config = context.config()?;
            let credentials = context.credentials()?;
            let inputs = client::selection_inputs(&config.options, workspace)?;
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
        let env = context.config()?.child_env.clone();
        let root = std::env::temp_dir();
        let fields = if interactive {
            if let Some(name) = display {
                context.write_stdout_with_policy(
                    format!("\nCreating status update for: {name}\n\n").as_bytes(),
                    OutputPolicy::ConsoleLike,
                )?;
            }
            let mut session = crate::platform::prompt::PromptSession::stdio(&mut *context.stdout)?;
            let prompted = command::prompt(&mut session, &mut *context.stderr, &env, &root, mode);
            match session.finish_result(prompted)? {
                PromptOutcome::Submitted(fields) => fields,
                PromptOutcome::Interrupted => return initiative_interrupt_status(),
                PromptOutcome::EndOfInput => {
                    return Err(AppError::new(
                        AppErrorKind::Validation,
                        "unexpected EOF while prompting for status update",
                    ));
                }
            }
        } else {
            let body = if let Some(body) = action.body.filter(|value| !value.is_empty()) {
                Some(body.to_owned())
            } else if let Some(path) = action.file.filter(|value| !value.is_empty()) {
                Some(command::file(path, mode, false)?)
            } else if !context.stdin_tty {
                command::stdin_body(&mut std::io::stdin().lock())
            } else if context.stdout_tty {
                context.write_stdout_with_policy(
                    format!("{}\n", mode.opening()).as_bytes(),
                    OutputPolicy::ConsoleLike,
                )?;
                match command::edit(&env, &root, &mut *context.stderr)? {
                    PromptOutcome::Submitted(body) => {
                        if body.is_none() {
                            context.write_stdout_with_policy(
                                b"No content entered.\n",
                                OutputPolicy::ConsoleLike,
                            )?;
                        }
                        body
                    }
                    PromptOutcome::Interrupted => return initiative_interrupt_status(),
                    PromptOutcome::EndOfInput => {
                        return Err(AppError::new(
                            AppErrorKind::Invariant,
                            "editor cannot return input EOF",
                        ));
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
        context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
        Ok(ExitStatus::Success)
    })();
    result.map_err(|error: AppError| error.with_context(mode.context()))
}

fn dispatch_initiative_update(
    context: &mut AppContext<'_>,
    action: &cli::initiative::InitiativeUpdate,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    use crate::commands::initiative_update as command;
    use crate::platform::prompt::{PromptOutcome, PromptSession, escaped_display};
    let transport = relation_transport(context, workspace)?;
    let reference = {
        let config = context.config()?;
        let credentials = context.credentials()?;
        let inputs = client::selection_inputs(&config.options, workspace)?;
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
    if fields.should_prompt(action.interactive, context.stdout_tty) {
        context.write_stdout_with_policy(
            format!(
                "\nUpdating initiative: {}\n\n",
                escaped_display(&current.name)
            )
            .as_bytes(),
            OutputPolicy::ConsoleLike,
        )?;
        let mut session = PromptSession::stdio_cr_or_lf(&mut *context.stdout)?;
        let prompted = command::prompt(&mut session, &current);
        fields = match session.finish_result(prompted)? {
            PromptOutcome::Submitted(fields) => fields,
            PromptOutcome::Interrupted => return initiative_interrupt_status(),
            PromptOutcome::EndOfInput => {
                return Err(AppError::new(
                    AppErrorKind::Validation,
                    "unexpected EOF while updating initiative",
                ));
            }
        };
    }
    let owner_id = block_on_network(command::owner(&transport, fields.owner.as_deref()))?;
    if fields.empty() {
        context.write_stdout_with_policy(b"No changes specified\n", OutputPolicy::ConsoleLike)?;
        return Ok(ExitStatus::Success);
    }
    let output = document_fetch_with_spinner(
        context,
        false,
        command::submit(&transport, &id, fields.input(owner_id)),
    )?;
    context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
    Ok(ExitStatus::Success)
}

fn dispatch_issue_comment_update(
    context: &mut AppContext<'_>,
    action: &cli::issue::IssueCommentUpdate,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
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
            if context.stdin_tty {
                command::check_prompt_topology(true, command::stdout_is_fifo()?)?;
            }
            let mut session = PromptSession::stdin_stdio_cr_or_lf(&mut *context.stdout)?;
            let prompted = command::prompt_body(&mut session, &existing);
            body = match session.finish_result(prompted)? {
                PromptOutcome::Submitted(body) => Some(body),
                PromptOutcome::Interrupted => return initiative_interrupt_status(),
                PromptOutcome::EndOfInput => {
                    return Err(AppError::new(
                        AppErrorKind::Invariant,
                        "comment prompt must convert EOF to its text-specific error",
                    ));
                }
            };
        }
        let body = body.ok_or_else(|| {
            AppError::new(
                AppErrorKind::Invariant,
                "comment update body absent after prompt",
            )
        })?;
        let output = block_on_network(command::submit(&transport, &action.comment_id, body))?;
        context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
        Ok(ExitStatus::Success)
    })();
    result.map_err(|error: AppError| error.with_context(command::CONTEXT))
}

fn dispatch_config_generate(
    context: &mut AppContext<'_>,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    use crate::{
        commands::config_generate as command,
        platform::prompt::{PlainSelect, PromptOutcome, PromptSession},
    };
    let result = (|| {
        context.write_stdout_with_policy(command::BANNER.as_bytes(), OutputPolicy::ConsoleLike)?;
        // Borrow disjoint startup/stdout fields, not a full-context reference held by the session.
        let loaded = context.startup.result.as_ref().map_err(|_| {
            AppError::new(
                AppErrorKind::Invariant,
                "config action reached failed startup",
            )
        })?;
        let config = &loaded.config;
        let credentials = &loaded.credentials;
        let choice = command::workspace_choice(&config.options, credentials, workspace)?;
        let mut session = None;
        let mut prompt_output = Some(&mut *context.stdout);
        let answers = (|| {
            let selected = match choice {
                command::WorkspaceChoice::Existing => workspace.map(str::to_owned),
                command::WorkspaceChoice::Only(name) => Some(name),
                command::WorkspaceChoice::Menu {
                    options,
                    default_index,
                } => {
                    if context.stdin_tty {
                        command::check_prompt_topology(true, command::stdout_is_fifo()?)?;
                    }
                    let current = PromptSession::stdin_stdio_cr_or_lf(
                        prompt_output.take().ok_or_else(|| {
                            AppError::new(
                                AppErrorKind::Invariant,
                                "config prompt output already owned",
                            )
                        })?,
                    )?;
                    session = Some(current);
                    let current = session.as_mut().ok_or_else(|| {
                        AppError::new(AppErrorKind::Invariant, "workspace session absent")
                    })?;
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
                            return Err(AppError::new(
                                AppErrorKind::Invariant,
                                "workspace EOF conversion absent",
                            ));
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
            if context.stdin_tty {
                command::check_prompt_topology(true, command::stdout_is_fifo()?)?;
            }
            match session.as_mut() {
                Some(current) => current.resume()?,
                None => {
                    session = Some(PromptSession::stdin_stdio_cr_or_lf(
                        prompt_output.take().ok_or_else(|| {
                            AppError::new(
                                AppErrorKind::Invariant,
                                "config prompt output already owned",
                            )
                        })?,
                    )?)
                }
            }
            let current = session
                .as_mut()
                .ok_or_else(|| AppError::new(AppErrorKind::Invariant, "team session absent"))?;
            let choices = command::team_options(&teams);
            let id = match command::stage(
                current.searchable_select("Select a team:", "Search teams", &choices)?,
                "team",
            )? {
                PromptOutcome::Submitted(id) => id,
                PromptOutcome::Interrupted => return Ok(PromptOutcome::Interrupted),
                PromptOutcome::EndOfInput => {
                    return Err(AppError::new(
                        AppErrorKind::Invariant,
                        "team EOF conversion absent",
                    ));
                }
            };
            let key = command::team_key(&teams, &id)?.to_owned();
            let sort = match command::stage(command::sort_prompt(current)?, "sort order")? {
                PromptOutcome::Submitted(sort) => sort,
                PromptOutcome::Interrupted => return Ok(PromptOutcome::Interrupted),
                PromptOutcome::EndOfInput => {
                    return Err(AppError::new(
                        AppErrorKind::Invariant,
                        "sort EOF conversion absent",
                    ));
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
                return Err(AppError::new(
                    AppErrorKind::Invariant,
                    "config stage EOF conversion absent",
                ));
            }
        };
        let root = block_on_network(command::late_root(
            &context.cwd,
            &config.child_env,
            command::GitLimits::default(),
        ))?;
        let path = command::destination(&root, |path| {
            let absolute = if path.is_absolute() {
                path.to_owned()
            } else {
                context.cwd.join(path)
            };
            std::fs::metadata(absolute).is_ok() // follows symlinks, any stat success, ordinary errors fallback.
        });
        let content = command::template(&written_workspace, &key, sort);
        let output = command::write_config(&context.cwd, &path, &content)?;
        context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
        Ok(ExitStatus::Success)
    })();
    result.map_err(|error: AppError| error.with_context(command::CONTEXT))
}
