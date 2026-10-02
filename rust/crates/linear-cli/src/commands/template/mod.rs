//! `linear template`.
pub mod json;
pub mod list;
pub mod view;

use crate::app::legacy::{block_on_network, spinner};
use crate::cli;
use crate::cli::template::TemplateCommand;
use crate::commands::table;
use crate::commands::template::list as template_list;
use crate::commands::template::view as template_view;
use crate::ctx::Ctx;
use crate::error::Result;

pub fn run(ctx: &Ctx, command: &TemplateCommand) -> Result<()> {
    let workspace = ctx.workspace();
    let context = ctx;
    match command {
        TemplateCommand::List(action) => dispatch_template_list(context, action, workspace),
        TemplateCommand::View(action) => dispatch_template_view(context, action, workspace),
    }
}

// Not yet migrated: these run on the transitional helpers in app::legacy.

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
