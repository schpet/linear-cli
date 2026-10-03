//! `initiative add-project`: link a project to an initiative.
use crate::cli::initiative::InitiativeAddProject;
use crate::commands::outcome;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::initiative::{
    AddProjectToInitiative, AddVariables, InitiativeToProjectCreateInput,
};

use super::common::Pair;

pub fn run(ctx: &Ctx, args: &InitiativeAddProject) -> Result<()> {
    add(ctx, args).context("Failed to add project to initiative")
}

fn add(ctx: &Ctx, args: &InitiativeAddProject) -> Result<()> {
    let pair = Pair::parse(ctx, &args.initiative, &args.project)?;
    let client = ctx.client()?;
    let link = ctx.spin(true, pair.link(client))?;
    if link.id.is_some() {
        return ctx.print(format!(
            "Project \"{}\" is already linked to initiative \"{}\"\n",
            link.project, link.initiative
        ));
    }
    let result: AddProjectToInitiative = ctx.spin(
        true,
        client.mutate(AddVariables {
            input: InitiativeToProjectCreateInput {
                initiative_id: link.initiative_id.clone(),
                project_id: link.project_id.clone(),
                sort_order: args.sort_order.clone(),
            },
        }),
    )?;
    if !result.initiative_to_project_create.success {
        return Err(Error::new("Linear did not link the project"));
    }
    ctx.print(outcome::done(
        "Added",
        "project",
        &format!("{} to initiative {}", link.project, link.initiative),
        None,
    ))
}
