//! `linear team`.
pub mod autolinks;
pub mod create;
pub mod delete;
pub mod id;
pub mod list;
pub mod members;
pub mod states;

use crate::app::legacy::{block_on_network, delete_confirmation, relation_transport, spinner};
use crate::cli;
use crate::cli::team::TeamCommand;
use crate::commands::client;
use crate::commands::table;
use crate::commands::team::create as team_create;
use crate::commands::team::delete as team_delete;
use crate::commands::team::id as team_id;
use crate::commands::team::list as team_list;
use crate::commands::team::members as team_members;
use crate::commands::team::states as team_states;
use crate::commands::team_key::configured_team_key;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::refs::{WorkspaceScope, prepare_team_lookup, resolve_team_with_transport};

pub fn run(ctx: &Ctx, command: &TeamCommand) -> Result<()> {
    let workspace = ctx.workspace();
    let context = ctx;
    match command {
        TeamCommand::Create(action) => dispatch_team_create(context, action, workspace),
        TeamCommand::Delete(action) => dispatch_team_delete(context, action, workspace),
        TeamCommand::List(action) => dispatch_team_list(context, action, workspace),
        TeamCommand::Id(action) => dispatch_team_id(context, action, workspace),
        TeamCommand::Autolinks(_) => {
            crate::commands::team::autolinks::execute(context.config(), workspace, context.cwd())
        }
        TeamCommand::Members(action) => dispatch_team_members(context, action, workspace),
        TeamCommand::States(action) => dispatch_team_states(context, action, workspace),
    }
}

// Not yet migrated: these run on the transitional helpers in app::legacy.

fn missing_team_key() -> Error {
    Error::new("Could not determine team key from directory name")
        .with_hint("Please specify a team key, name, or ID as an argument.")
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

fn dispatch_team_delete(
    context: &Ctx,
    action: &cli::team::TeamDelete,
    workspace: Option<&str>,
) -> Result<()> {
    use crate::{
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
                        PromptOutcome::Interrupted => return Err(Error::cancelled()),
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
                PromptOutcome::Interrupted => return Err(Error::cancelled()),
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
