//! `initiative remove-project`: unlink a project from an initiative.
use crate::cli::initiative::InitiativeRemoveProject;
use crate::commands::outcome;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::common::IdVariables;
use crate::graphql::operations::initiative::RemoveProjectFromInitiative;

use super::common::Pair;

pub fn run(ctx: &Ctx, args: &InitiativeRemoveProject) -> Result<()> {
    remove(ctx, args).context("Failed to remove project from initiative")
}

fn remove(ctx: &Ctx, args: &InitiativeRemoveProject) -> Result<()> {
    if !args.confirm.yes {
        ctx.require_tty("for confirmation", "--yes")?;
    }
    let pair = Pair::parse(ctx, &args.initiative, &args.project)?;
    let client = ctx.client()?;
    let link = ctx.spin(true, pair.link(client))?;
    let Some(link_id) = link.id else {
        return Err(Error::new(format!(
            "Project \"{}\" is not linked to initiative \"{}\"",
            link.project, link.initiative
        )));
    };
    let question = format!(
        "Remove \"{}\" from initiative \"{}\"?",
        link.project, link.initiative
    );
    if !args.confirm.yes && !ctx.confirm(&question, "--yes")? {
        return outcome::canceled(ctx);
    }
    let result: RemoveProjectFromInitiative =
        ctx.spin(true, client.mutate(IdVariables { id: link_id }))?;
    if !result.initiative_to_project_delete.success {
        return Err(Error::new("Linear did not unlink the project"));
    }
    ctx.print(outcome::done(
        "Removed",
        "project",
        &format!("{} from initiative {}", link.project, link.initiative),
        None,
    ))
}
