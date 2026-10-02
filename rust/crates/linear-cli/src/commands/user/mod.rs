//! `linear user`.
pub mod list;

use crate::app::legacy::{block_on_network, spinner};
use crate::cli;
use crate::cli::user::UserCommand;
use crate::commands::client;
use crate::commands::user::list as user_list;
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};

pub fn run(ctx: &Ctx, command: &UserCommand) -> Result<()> {
    let workspace = ctx.workspace();
    let context = ctx;
    match command {
        UserCommand::List(action) => dispatch_user_list(context, action, workspace),
    }
}

// Not yet migrated: these run on the transitional helpers in app::legacy.

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
