//! `milestone delete`: delete one project milestone after confirmation.
use cynic::MutationBuilder;

use crate::cli::milestone::MilestoneDelete;
use crate::commands::confirm;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::milestone_delete::{
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
    let request = GraphQlRequest::with_variables(DeleteProjectMilestone::build(
        DeleteProjectMilestoneVariables { id: id.clone() },
    ));
    let result: DeleteProjectMilestone = ctx.spin(true, client.execute(&request))?;
    if !result.project_milestone_delete.success {
        return Err(Error::new("Linear did not delete the milestone"));
    }
    ctx.print(format!("✓ Deleted milestone {id}\n"))
}
