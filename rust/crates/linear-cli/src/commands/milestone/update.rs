//! `milestone update`: change the given fields of one milestone.
use cynic::MutationBuilder;

use crate::cli::milestone::MilestoneUpdate;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::milestone_update::{
    ProjectMilestoneUpdateInput, UpdateProjectMilestone, UpdateProjectMilestoneVariables,
    UpdatedMilestone,
};
use crate::graphql::scalars::TimelessDate;
use crate::refs::{prepare_project_lookup, reject_linear_url, resolve_project_with_transport};

pub fn run(ctx: &Ctx, args: &MilestoneUpdate) -> Result<()> {
    update(ctx, args).context("Failed to update milestone")
}

fn update(ctx: &Ctx, args: &MilestoneUpdate) -> Result<()> {
    reject_linear_url(&args.id, "a milestone UUID")?;
    let project = match args.project.as_deref() {
        Some(project) => Some((prepare_project_lookup(project, &ctx.scope()?)?, project)),
        None => None,
    };
    let client = ctx.client()?;
    let milestone = ctx.spin(true, async {
        let project_id = match &project {
            Some((reference, original)) => {
                Some(resolve_project_with_transport(reference, original, client).await?)
            }
            None => None,
        };
        let request = GraphQlRequest::with_variables(UpdateProjectMilestone::build(
            UpdateProjectMilestoneVariables {
                id: args.id.clone(),
                input: ProjectMilestoneUpdateInput {
                    name: args.name.clone(),
                    description: args.description.clone(),
                    target_date: args.target_date.map(|date| TimelessDate(date.to_string())),
                    sort_order: args.sort_order.clone(),
                    project_id,
                },
            },
        ));
        let result: UpdateProjectMilestone = client.execute(&request).await?;
        let payload = result.project_milestone_update;
        if !payload.success {
            return Err(Error::new("Linear did not update the milestone"));
        }
        Ok(payload.project_milestone)
    })?;
    ctx.print(render(&milestone))
}

fn render(milestone: &UpdatedMilestone) -> String {
    let mut output = format!(
        "✓ Updated milestone: {}\n  ID: {}\n",
        milestone.name,
        milestone.id.inner()
    );
    if let Some(date) = milestone
        .target_date
        .as_ref()
        .filter(|date| !date.0.is_empty())
    {
        output.push_str(&format!("  Target Date: {}\n", date.0));
    }
    output.push_str(&format!(
        "  Sort Order: {}\n  Project: {}\n",
        milestone.sort_order, milestone.project.name
    ));
    output
}
