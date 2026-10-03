//! `milestone delete`: delete one project milestone after confirmation.
use crate::cli::milestone::MilestoneDelete;
use crate::commands::confirm;
use crate::commands::outcome;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::milestone::{
    DeleteProjectMilestone, DeleteProjectMilestoneVariables,
};
use crate::refs::reject_linear_url;

pub fn run(ctx: &Ctx, args: &MilestoneDelete) -> Result<()> {
    delete(ctx, args).context("Failed to delete milestone")
}

fn delete(ctx: &Ctx, args: &MilestoneDelete) -> Result<()> {
    let id = &args.id;
    reject_linear_url(id, "a milestone UUID")?;
    let question = format!("Are you sure you want to delete milestone {id}?");
    if !confirm::deletion(ctx, args.force, &question)? {
        return Ok(());
    }
    let client = ctx.client()?;
    let result: DeleteProjectMilestone = ctx.spin(
        true,
        client.mutate(DeleteProjectMilestoneVariables { id: id.clone() }),
    )?;
    if !result.project_milestone_delete.success {
        return Err(Error::new("Linear did not delete the milestone"));
    }
    ctx.print(outcome::done("Deleted", "milestone", id, None))
}
