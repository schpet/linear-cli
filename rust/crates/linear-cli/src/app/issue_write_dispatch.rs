//! Issue create/update prompt and dispatch wiring, kept command-local.
use super::{
    AppContext, StartPromptOutput, block_on_network, initiative_interrupt_status, project_ticks,
    resolve_issue, script_status, start_snapshot, start_work_on_created_issue,
};
use crate::{
    cli,
    commands::{client, team_key::configured_team_key},
    config::{NoColor, StartupConfig},
    error::{AppError, AppErrorKind, ExitStatus},
    platform::{
        output::{Output, OutputPolicy, Stream},
        spinner,
    },
};
use std::io::Write;

struct IssueCreateUi<'a> {
    output: StartPromptOutput<'a>,
    stderr: &'a mut dyn Write,
    env: crate::config::ChildEnvOverlay,
    temporary_root: std::path::PathBuf,
    spin: bool,
    spinning: bool,
}
impl crate::commands::issue_write::Ui for IssueCreateUi<'_> {
    fn text(
        &mut self,
        message: &str,
        required: bool,
        default: Option<&str>,
    ) -> Result<String, AppError> {
        self.stop_spinner()?;
        let message = crate::platform::prompt::escaped_display(message);
        let answer = self.output.prompt()?.text_with_display_default(
            &message,
            crate::platform::prompt_text::TextOptions { required, default },
        )?;
        self.output.suspend()?;
        let value = create_answer(answer)?;
        self.resume_spinner()?;
        Ok(value)
    }
    fn choose(
        &mut self,
        message: &str,
        options: &[crate::commands::issue_write::Named],
        default: usize,
        search: bool,
    ) -> Result<String, AppError> {
        self.stop_spinner()?;
        let message = crate::platform::prompt::escaped_display(message);
        // Menu identity is local and unique; source IDs and duplicate/control names
        // retain semantics after selection and are only escaped for display.
        let answer = if search {
            let rows: Vec<_> = options
                .iter()
                .enumerate()
                .map(|(index, o)| crate::platform::selector::SelectOption {
                    label: crate::platform::prompt::escaped_display(&o.name),
                    value: index.to_string(),
                })
                .collect();
            let (label, no_match) = if message.starts_with("Which team") {
                ("Search teams", "no teams match submitted search query")
            } else {
                (
                    "Search projects",
                    "no projects match submitted search query",
                )
            };
            self.output
                .prompt()?
                .searchable_select_with_no_match(&message, label, &rows, no_match)?
        } else {
            let rows: Vec<_> = options
                .iter()
                .enumerate()
                .map(|(index, o)| crate::platform::prompt::PlainOption {
                    label: crate::platform::prompt::escaped_display(&o.name),
                    value: index.to_string(),
                    script_token: format!("option-{index}"),
                })
                .collect();
            self.output
                .prompt()?
                .select(&crate::platform::prompt::PlainSelect {
                    message: &message,
                    options: &rows,
                    default_index: default,
                    default_hint: None,
                })?
        };
        self.output.suspend()?;
        let selected = create_answer(answer)?;
        self.resume_spinner()?;
        issue_create_menu_value(&selected, options)
    }
    fn checkbox(
        &mut self,
        message: &str,
        options: &[crate::commands::issue_write::Named],
        search: bool,
    ) -> Result<Vec<String>, AppError> {
        self.stop_spinner()?;
        let rows: Vec<_> = options
            .iter()
            .enumerate()
            .map(|(index, o)| crate::platform::prompt::PlainOption {
                label: crate::platform::prompt::escaped_display(&o.name),
                value: index.to_string(),
                script_token: format!("field-{index}"),
            })
            .collect();
        let answer = self.output.prompt()?.checkbox(
            &crate::platform::prompt::escaped_display(message),
            &rows,
            search,
        )?;
        self.output.suspend()?;
        let selected = create_answer(answer)?;
        self.resume_spinner()?;
        selected
            .iter()
            .map(|selected| issue_create_menu_value(selected, options))
            .collect()
    }
    fn suspend(&mut self) -> Result<(), AppError> {
        // Every method returns only after suspending. No blind second suspend.
        self.output.writer().map(|_| ())
    }
    fn output(&mut self, text: &str) -> Result<(), AppError> {
        script_status(self.output.writer()?, text.as_bytes())
    }
    fn error(&mut self, text: &str) -> Result<(), AppError> {
        Output::new(&mut *self.stderr, Stream::Stderr).write(text.as_bytes())
    }
    fn discover_editor(&mut self) -> Result<Option<String>, AppError> {
        crate::platform::editor::discover(&self.env)
            .map(|name| {
                name.into_string().map_err(|_| {
                    AppError::new(
                        AppErrorKind::Validation,
                        "Editor executable is not representable as text",
                    )
                })
            })
            .transpose()
    }
    fn optional_editor(&mut self) -> Result<Option<String>, AppError> {
        use crate::platform::editor::{self, EditorOutcome};
        match editor::open(&self.env, None, &self.temporary_root)? {
            EditorOutcome::Content(content) => Ok(content),
            EditorOutcome::Missing => {
                self.error("No editor found. Please set EDITOR environment variable or configure git editor with: git config --global core.editor <editor>\n")?;
                Ok(None)
            }
            EditorOutcome::Failed(error) => {
                self.error(&format!("{}\n", error.message))?;
                Ok(None)
            }
        }
    }
}
fn issue_create_menu_value(
    selected: &str,
    options: &[crate::commands::issue_write::Named],
) -> Result<String, AppError> {
    let index = selected.parse::<usize>().map_err(|error| {
        AppError::new(
            AppErrorKind::Invariant,
            "issue-create menu returned a non-index value",
        )
        .with_source(error)
    })?;
    options
        .get(index)
        .map(|option| option.id.clone())
        .ok_or_else(|| {
            AppError::new(
                AppErrorKind::Invariant,
                "issue-create menu returned an out-of-range value",
            )
        })
}
fn create_answer<T>(answer: crate::platform::prompt::PromptOutcome<T>) -> Result<T, AppError> {
    use crate::platform::prompt::PromptOutcome;
    match answer {
        PromptOutcome::Submitted(value) => Ok(value),
        PromptOutcome::EndOfInput => Err(AppError::new(
            AppErrorKind::Validation,
            "Input ended before issue creation prompts completed",
        )),
        PromptOutcome::Interrupted => Err(AppError::new(
            AppErrorKind::Cancellation,
            "Issue creation interrupted",
        )),
    }
}
impl IssueCreateUi<'_> {
    fn stop_spinner(&mut self) -> Result<(), AppError> {
        if self.spinning {
            script_status(self.output.writer()?, spinner::CLEAR)?;
            self.spinning = false;
        }
        Ok(())
    }
    fn resume_spinner(&mut self) -> Result<(), AppError> {
        if self.spin && !self.spinning {
            script_status(self.output.writer()?, spinner::frame(0).as_bytes())?;
            self.spinning = true;
        }
        Ok(())
    }
    fn close(&mut self) -> Result<(), AppError> {
        self.stop_spinner()?;
        self.output.close()
    }
}
fn issue_write_backend(
    config: &StartupConfig,
    credentials: &crate::auth::CredentialStore,
    workspace: Option<&str>,
) -> Result<crate::commands::issue_write_network::NetworkBackend, AppError> {
    Ok(crate::commands::issue_write_network::NetworkBackend {
        transport: client::prepare_transport(
            &config.options,
            credentials,
            workspace,
            &config.transport_env,
        )?,
        options: config.options.clone(),
        cli_workspace: workspace.map(str::to_owned),
        default_workspace: credentials.default().map(str::to_owned),
    })
}
fn issue_create_settings(config: &StartupConfig) -> crate::commands::issue_write::CreateSettings {
    use crate::{
        commands::issue_write::{AssignSelf, CreateSettings},
        config::AssignSelf as ConfigAssignSelf,
    };
    CreateSettings {
        default_team: configured_team_key(&config.options),
        assign_self: match config
            .options
            .issue_create_assign_self()
            .map(|value| *value.value())
            .unwrap_or(ConfigAssignSelf::Auto)
        {
            ConfigAssignSelf::Always => AssignSelf::Always,
            ConfigAssignSelf::Auto => AssignSelf::Auto,
            ConfigAssignSelf::Never => AssignSelf::Never,
        },
        ask_project: config
            .options
            .issue_create_ask_project()
            .is_some_and(|value| *value.value()),
    }
}
fn issue_write_wait<T>(
    writer: &mut dyn Write,
    pending: impl std::future::Future<Output = Result<T, AppError>>,
    spin: bool,
) -> Result<T, AppError> {
    if spin {
        script_status(writer, spinner::frame(0).as_bytes())?;
    }
    let result = block_on_network(async {
        if !spin {
            return pending.await;
        }
        tokio::pin!(pending);
        let mut ticks = tokio::time::interval(spinner::TICK_INTERVAL);
        ticks.tick().await;
        let mut frame = 1_usize;
        loop {
            tokio::select! {biased;result=&mut pending=>break result,_=ticks.tick()=>{
                script_status(writer,spinner::frame(frame).as_bytes())?;frame=frame.wrapping_add(1);
            }}
        }
    });
    if spin {
        script_status(writer, spinner::CLEAR)?;
    }
    result
}
pub(super) fn dispatch_issue_create(
    context: &mut AppContext<'_>,
    action: &cli::issue::IssueCreate,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    use crate::commands::{
        issue_create as command,
        issue_write::{Backend, Ui},
    };
    let fields = command::Fields::from(action);
    let description = match fields.local() {
        Ok(description) => description,
        Err(error) => {
            context.write_stderr(format!("✗ {}\n", error.message).as_bytes())?;
            return Ok(ExitStatus::HandledFailure);
        }
    };
    let interactive = fields.full_interactive(description.as_deref(), context.stdout_tty);
    if !interactive && let Err(error) = fields.require_flag_title() {
        context.write_stderr(format!("✗ {}\n", error.message).as_bytes())?;
        return Ok(ExitStatus::HandledFailure);
    }
    let result = (|| {
        let (config, credentials) = start_snapshot(&context.startup)?;
        let spin = !interactive
            && spinner::enabled(
                false,
                context.stdout_tty,
                context.startup.settings.no_color == NoColor::Absent,
            );
        let mut ui = IssueCreateUi {
            output: StartPromptOutput::new(&mut *context.stdout),
            stderr: &mut *context.stderr,
            env: config.child_env.clone(),
            temporary_root: std::env::temp_dir(),
            spin,
            spinning: false,
        };
        let action_result = (|| {
            ui.resume_spinner()?; // Flag spinner begins before client preparation and all lookups.
            let backend = issue_write_backend(config, credentials, workspace)?;
            let settings = issue_create_settings(config);
            let (input, title, start) = if interactive {
                let prompted = crate::commands::issue_create_prompt::prompt(
                    &backend, &mut ui, &settings, &fields,
                )?;
                ui.output("Creating issue...\n\n")?;
                (prompted.input, Some(prompted.title), prompted.start)
            } else {
                let assembled = block_on_network(command::flag_input(
                    &backend,
                    &mut ui,
                    &settings,
                    &fields,
                    description,
                    !fields.no_interactive && context.stdout_tty,
                ))?;
                ui.stop_spinner()?;
                ui.output(&command::flag_header(&assembled.team_display))?;
                (assembled.input, None, fields.start)
            };
            let issue = issue_write_wait(ui.output.writer()?, backend.create(input), spin)?;
            ui.spinning = false;
            ui.output(&match &title {
                Some(title) => command::interactive_output(&issue, title),
                None => command::flag_output(&issue),
            })?;
            if start {
                // Interactive source resolves the returned team key again; flag source passes the key.
                let team = if interactive {
                    block_on_network(backend.team(issue.team_key.clone()))?.id
                } else {
                    issue.team_key.clone()
                };
                return start_work_on_created_issue(
                    ui.stderr,
                    config,
                    credentials,
                    workspace,
                    &issue.id,
                    &team,
                    &mut ui.output,
                    &context.cwd,
                    spinner::enabled(
                        false,
                        context.stdout_tty,
                        context.startup.settings.no_color == NoColor::Absent,
                    ),
                    context.stdin_tty,
                    None,
                    None,
                );
            }
            Ok(ExitStatus::Success)
        })();
        ui.close()?;
        action_result
    })();
    match result {
        Err(error) if error.kind == AppErrorKind::Cancellation => initiative_interrupt_status(),
        other => other.map_err(|error: AppError| error.with_context("Failed to create issue")),
    }
}
pub(super) fn dispatch_issue_update(
    context: &mut AppContext<'_>,
    action: &cli::issue::IssueUpdate,
    workspace: Option<&str>,
) -> Result<ExitStatus, AppError> {
    use crate::commands::{issue_update as command, issue_write::Backend};
    let result = (|| {
        let fields = command::Fields::from(action);
        let description = fields.local()?;
        let identifier = resolve_issue(context, action.issue_id.as_deref(), workspace).map_err(|error|
            if error.message=="Could not determine issue ID" {
                error.with_suggestion("Please provide an issue ID like 'ENG-123' or run from a branch with an issue ID.")
            }else{error})?;
        let spin = spinner::enabled(
            false,
            context.stdout_tty,
            context.startup.settings.no_color == NoColor::Absent,
        );
        if spin {
            context.write_stdout_with_policy(
                spinner::frame(0).as_bytes(),
                OutputPolicy::ConsoleLike,
            )?;
        }
        let assembled = (|| {
            let backend = {
                let (config, credentials) = start_snapshot(&context.startup)?;
                issue_write_backend(config, credentials, workspace)?
            };
            let input = project_ticks(
                context,
                command::input(&backend, &identifier, &fields, description),
                spin,
            )?;
            Ok::<_, AppError>((backend, input))
        })();
        if spin {
            context.write_stdout_with_policy(spinner::CLEAR, OutputPolicy::ConsoleLike)?;
        }
        let (backend, input) = assembled?;
        context.write_stdout_with_policy(
            command::header(&identifier).as_bytes(),
            OutputPolicy::ConsoleLike,
        )?;
        let issue = issue_write_wait(context.stdout, backend.update(identifier, input), spin)?;
        context.write_stdout_with_policy(
            command::output(&issue).as_bytes(),
            OutputPolicy::ConsoleLike,
        )?;
        Ok(ExitStatus::Success)
    })();
    result.map_err(|error: AppError| error.with_context("Failed to update issue"))
}
