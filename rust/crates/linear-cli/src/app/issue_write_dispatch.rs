//! Issue create/update prompt and dispatch wiring, kept command-local.
use super::spinner;
use super::{
    StartPromptOutput, block_on_network, initiative_interrupt_status, project_ticks, resolve_issue,
    script_status, start_snapshot, start_work_on_created_issue,
};
use crate::{
    cli,
    commands::{client, team_key::configured_team_key},
    config::StartupConfig,
    ctx::Ctx,
    error::{Error, ErrorKind, Result, ResultExt},
};
use std::io::Write;

struct IssueCreateUi<'a> {
    output: StartPromptOutput<'a>,
    stderr: &'a mut dyn Write,
    env: crate::config::ChildEnvOverlay,
    spin: bool,
    spinning: bool,
}
impl crate::commands::issue_write::Ui for IssueCreateUi<'_> {
    fn text(
        &mut self,
        message: &str,
        required: bool,
        default: Option<&str>,
    ) -> Result<String, Error> {
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
    ) -> Result<String, Error> {
        self.stop_spinner()?;
        let message = crate::platform::prompt::escaped_display(message);
        // Rows are identified by position, so duplicate or control-character names
        // still select the right option; names are only escaped for display.
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
    ) -> Result<Vec<String>, Error> {
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
    fn suspend(&mut self) -> Result<(), Error> {
        // Every method returns only after suspending. No blind second suspend.
        self.output.writer().map(|_| ())
    }
    fn output(&mut self, text: &str) -> Result<(), Error> {
        script_status(self.output.writer()?, text.as_bytes())
    }
    fn error(&mut self, text: &str) -> Result<(), Error> {
        self.stderr.write_all(text.as_bytes()).map_err(|error| {
            Error::new(format!("failed to write to stderr: {error}")).with_source(error)
        })
    }
    fn discover_editor(&mut self) -> Result<Option<String>, Error> {
        crate::platform::editor::configured(&self.env)
            .map(|name| {
                name.into_string()
                    .map_err(|_| Error::new("Editor executable is not representable as text"))
            })
            .transpose()
    }
    fn optional_editor(&mut self) -> Result<Option<String>, Error> {
        match crate::platform::editor::edit("", &self.env) {
            Ok(text) => Ok(crate::commands::text_input::edited_body(&text)),
            Err(error) => {
                self.error(&format!("{error}\n"))?;
                Ok(None)
            }
        }
    }
}
fn issue_create_menu_value(
    selected: &str,
    options: &[crate::commands::issue_write::Named],
) -> Result<String, Error> {
    let index = selected.parse::<usize>().map_err(|error| {
        Error::new("issue-create menu returned a non-index value").with_source(error)
    })?;
    options
        .get(index)
        .map(|option| option.id.clone())
        .ok_or_else(|| Error::new("issue-create menu returned an out-of-range value"))
}
fn create_answer<T>(answer: crate::platform::prompt::PromptOutcome<T>) -> Result<T, Error> {
    use crate::platform::prompt::PromptOutcome;
    match answer {
        PromptOutcome::Submitted(value) => Ok(value),
        PromptOutcome::EndOfInput => Err(Error::new(
            "Input ended before issue creation prompts completed",
        )),
        PromptOutcome::Interrupted => Err(Error::cancelled()),
    }
}
impl IssueCreateUi<'_> {
    fn stop_spinner(&mut self) -> Result<(), Error> {
        if self.spinning {
            script_status(self.output.writer()?, spinner::CLEAR)?;
            self.spinning = false;
        }
        Ok(())
    }
    fn resume_spinner(&mut self) -> Result<(), Error> {
        if self.spin && !self.spinning {
            script_status(self.output.writer()?, spinner::frame(0).as_bytes())?;
            self.spinning = true;
        }
        Ok(())
    }
    fn close(&mut self) -> Result<(), Error> {
        self.stop_spinner()?;
        self.output.close()
    }
}
fn issue_write_backend(
    config: &StartupConfig,
    credentials: &crate::auth::CredentialStore,
    workspace: Option<&str>,
) -> Result<crate::commands::issue_write_network::NetworkBackend, Error> {
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
    pending: impl std::future::Future<Output = Result<T, Error>>,
    spin: bool,
) -> Result<T, Error> {
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
    context: &Ctx,
    action: &cli::issue::IssueCreate,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::commands::{
        issue_create as command,
        issue_write::{Backend, Ui},
    };
    let fields = command::Fields::from(action);
    let description = match fields.local() {
        Ok(description) => description,
        Err(error) => {
            context.eprint(format!("✗ {}\n", error.message()).as_bytes())?;
            return Err(Error::reported());
        }
    };
    let interactive = fields.full_interactive(description.as_deref(), context.stdout_tty());
    if !interactive && let Err(error) = fields.require_flag_title() {
        context.eprint(format!("✗ {}\n", error.message()).as_bytes())?;
        return Err(Error::reported());
    }
    let result = (|| {
        let (config, credentials) = start_snapshot(context)?;
        let spin = !interactive && spinner::enabled(false, context.stdout_tty(), true);
        let mut stderr = std::io::stderr();
        let mut ui = IssueCreateUi {
            output: StartPromptOutput::new(context.stdout()),
            stderr: &mut stderr,
            env: config.child_env.clone(),
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
                    !fields.no_interactive && context.stdout_tty(),
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
                // Interactive creation resolves the chosen team key to an id again; flags pass the key.
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
                    context.cwd(),
                    spinner::enabled(false, context.stdout_tty(), true),
                    context.stdin_tty(),
                    None,
                    None,
                );
            }
            Ok(())
        })();
        ui.close()?;
        action_result
    })();
    match result {
        Err(error) if error.kind() == ErrorKind::Cancelled => initiative_interrupt_status(),
        other => other.context("Failed to create issue"),
    }
}
pub(super) fn dispatch_issue_update(
    context: &Ctx,
    action: &cli::issue::IssueUpdate,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::commands::{issue_update as command, issue_write::Backend};
    let result: Result<()> = (|| {
        let fields = command::Fields::from(action);
        let description = fields.local()?;
        let identifier = resolve_issue(context, action.issue_id.as_deref(), workspace).map_err(|error|
            if error.message() == "Could not determine issue ID" {
                error.with_hint("Please provide an issue ID like 'ENG-123' or run from a branch with an issue ID.")
            }else{error})?;
        let spin = spinner::enabled(false, context.stdout_tty(), true);
        if spin {
            context.print(spinner::frame(0).as_bytes())?;
        }
        let assembled = (|| {
            let backend = {
                let (config, credentials) = start_snapshot(context)?;
                issue_write_backend(config, credentials, workspace)?
            };
            let input = project_ticks(
                context,
                command::input(&backend, &identifier, &fields, description),
                spin,
            )?;
            Ok::<_, Error>((backend, input))
        })();
        if spin {
            context.print(spinner::CLEAR)?;
        }
        let (backend, input) = assembled?;
        context.print(command::header(&identifier).as_bytes())?;
        let issue = issue_write_wait(
            &mut context.stdout(),
            backend.update(identifier, input),
            spin,
        )?;
        context.print(command::output(&issue).as_bytes())?;
        Ok(())
    })();
    result.context("Failed to update issue")
}
