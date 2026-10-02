//! `linear project-update`.
pub mod list;

use crate::app::legacy::{block_on_network, spinner};
use crate::cli;
use crate::cli::project_update::ProjectUpdateCommand;
use crate::commands::client;
use crate::commands::project_update::list as project_update_list;
use crate::commands::table;
use crate::commands::update_create::{UpdateCreateAction, dispatch_update_create};
use crate::ctx::Ctx;
use crate::error::Result;
use crate::refs::{WorkspaceScope, prepare_project_lookup, resolve_project_with_transport};

pub fn run(ctx: &Ctx, command: &ProjectUpdateCommand) -> Result<()> {
    let workspace = ctx.workspace();
    let context = ctx;
    match command {
        ProjectUpdateCommand::Create(action) => dispatch_update_create(
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
        ProjectUpdateCommand::List(action) => {
            dispatch_project_update_list(context, action, workspace)
        }
    }
}

// Not yet migrated: these run on the transitional helpers in app::legacy.

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
