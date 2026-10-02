//! `linear label`.
pub mod create;
pub mod delete;
pub mod list;

use crate::app::legacy::{block_on_network, spinner};
use crate::cli;
use crate::cli::label::LabelCommand;
use crate::commands;
use crate::commands::client;
use crate::commands::label::create as label_create;
use crate::commands::label::delete as label_delete;
use crate::commands::label::list as label_list;
use crate::commands::table;
use crate::commands::team_key::configured_team_key;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::refs::{WorkspaceScope, prepare_team_lookup, resolve_team_with_transport};

pub fn run(ctx: &Ctx, command: &LabelCommand) -> Result<()> {
    let workspace = ctx.workspace();
    let context = ctx;
    match command {
        LabelCommand::List(action) => dispatch_label_list(context, action, workspace),
        LabelCommand::Create(action) => dispatch_label_create(context, action, workspace),
        LabelCommand::Delete(action) => dispatch_label_delete(context, action, workspace),
    }
}

// Not yet migrated: these run on the transitional helpers in app::legacy.

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
        if !commands::confirm::deletion(
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
