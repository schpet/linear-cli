//! `linear project`.
pub mod collections;
pub mod comment_list;
pub mod create;
pub mod delete;
pub mod list;
pub mod update;
pub mod view;
pub mod write;

use crate::app::legacy::{block_on_network, finish_comment_add, spinner, submit_comment};
use crate::cli;
use crate::cli::project::ProjectCommand;
use crate::commands;
use crate::commands::client;
use crate::commands::comment_add;
use crate::commands::project::comment_list as project_comment_list;
use crate::commands::project::delete as project_delete;
use crate::commands::project::list as project_list;
use crate::commands::project::view as project_view;
use crate::commands::team_key::configured_team_key;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::refs::{
    ProjectReference, WorkspaceScope, prepare_project_lookup, prepare_team_lookup,
    resolve_project_with_transport, resolve_team_with_transport,
};

pub fn run(ctx: &Ctx, command: &ProjectCommand) -> Result<()> {
    let workspace = ctx.workspace();
    let context = ctx;
    match command {
        ProjectCommand::List(action) => dispatch_project_list(context, action, workspace),
        ProjectCommand::View(action) => dispatch_project_view(context, action, workspace),
        ProjectCommand::Create(action) => dispatch_project_create(context, action, workspace),
        ProjectCommand::Update(action) => dispatch_project_update(context, action, workspace),
        ProjectCommand::Delete(action) => dispatch_project_delete(context, action, workspace),
        ProjectCommand::Comment(action) => match &action.command {
            cli::project::ProjectCommentCommand::Add(action) => {
                dispatch_project_comment_add(context, action, workspace)
            }
            cli::project::ProjectCommentCommand::List(action) => {
                dispatch_project_comment_list(context, action, workspace)
            }
        },
    }
}

// Not yet migrated: these run on the transitional helpers in app::legacy.

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

fn dispatch_project_delete(
    context: &Ctx,
    action: &cli::project::ProjectDelete,
    workspace: Option<&str>,
) -> Result<()> {
    let original = &action.project_id;
    if !commands::confirm::deletion(
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
            None => crate::commands::comment_add::prompt(context)?,
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
    use crate::commands::project::create as command;
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
                    return Err(Error::cancelled());
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
    use crate::commands::project::update as command;
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
