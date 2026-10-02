//! `linear milestone`: project milestones.
pub mod create;
pub mod delete;
pub mod list;
pub mod update;
pub mod view;

use crate::app::legacy::{block_on_network, spinner};
use crate::cli;
use crate::cli::milestone::MilestoneCommand;
use crate::commands;
use crate::commands::client;
use crate::commands::milestone::create as milestone_create;
use crate::commands::milestone::list as milestone_list;
use crate::commands::milestone::update as milestone_update;
use crate::commands::milestone::view as milestone_view;
use crate::commands::table;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::refs::{WorkspaceScope, prepare_project_lookup, resolve_project_with_transport};

pub fn run(ctx: &Ctx, command: &MilestoneCommand) -> Result<()> {
    let workspace = ctx.workspace();
    let context = ctx;
    match command {
        MilestoneCommand::List(action) => dispatch_milestone_list(context, action, workspace),
        MilestoneCommand::View(action) => dispatch_milestone_view(context, action, workspace),
        MilestoneCommand::Create(action) => dispatch_milestone_create(context, action, workspace),
        MilestoneCommand::Update(action) => dispatch_milestone_update(context, action, workspace),
        MilestoneCommand::Delete(args) => commands::milestone::delete::run(ctx, args),
    }
}

// Not yet migrated: these run on the transitional helpers in app::legacy.

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
