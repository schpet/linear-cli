//! `linear initiative-update`.
pub mod list;

use crate::app::legacy::{block_on_network, spinner};
use crate::cli;
use crate::cli::initiative_update::InitiativeUpdateCommand;
use crate::commands::client;
use crate::commands::initiative::view as initiative_view;
use crate::commands::initiative_update::list as initiative_update_list;
use crate::commands::table;
use crate::commands::update_create::{UpdateCreateAction, dispatch_update_create};
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};
use crate::refs::WorkspaceScope;

pub fn run(ctx: &Ctx, command: &InitiativeUpdateCommand) -> Result<()> {
    let workspace = ctx.workspace();
    let context = ctx;
    match command {
        InitiativeUpdateCommand::Create(action) => dispatch_update_create(
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
        ),
        InitiativeUpdateCommand::List(action) => {
            dispatch_initiative_update_list(context, action, workspace)
        }
    }
}

// Not yet migrated: these run on the transitional helpers in app::legacy.

fn dispatch_initiative_update_list(
    context: &Ctx,
    action: &cli::initiative_update::InitiativeUpdateList,
    workspace: Option<&str>,
) -> Result<()> {
    let json = action.json;
    let first = initiative_update_list::graphql_int(action.limit)?;
    let original = &action.initiative_id;
    let show_spinner = spinner::enabled(json, context.stdout_tty(), true);
    if show_spinner {
        context.print(spinner::frame(0).as_bytes())?;
    }
    let result = (|| {
        let config = context.config();
        let credentials = context.credentials()?;

        let inputs = client::selection_inputs(&config.options, workspace);
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
        let columns = table::stdout_columns(context.stdout_tty());
        let color = context.stdout_tty() && context.color();
        block_on_network(async {
            let id = initiative_view::resolve_reference(&transport, &reference, original)
                .await
                .context(initiative_update_list::CONTEXT)?;
            initiative_update_list::run(&transport, original, &id, first, json, columns, color)
                .await
        })
    })();
    if show_spinner {
        context.print(spinner::CLEAR)?;
    }
    let output = result.map_err(|error| {
        if error.has_context() {
            error
        } else {
            error.context(initiative_update_list::CONTEXT)
        }
    })?;
    context.print(&output)?;
    Ok(())
}
