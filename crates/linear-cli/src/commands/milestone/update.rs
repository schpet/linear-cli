//! `milestone update`: change the given fields of one milestone.
use crate::cli::milestone::MilestoneUpdate;
use crate::commands::outcome;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::milestone::{
    ProjectMilestoneUpdateInput, UpdateProjectMilestone, UpdateProjectMilestoneVariables,
    UpdatedMilestone,
};
use crate::graphql::scalars::TimelessDate;
use crate::refs::{self, project::ProjectReference, reject_linear_url};

pub fn run(ctx: &Ctx, args: &MilestoneUpdate) -> Result<()> {
    update(ctx, args).context("Failed to update milestone")
}

fn update(ctx: &Ctx, args: &MilestoneUpdate) -> Result<()> {
    reject_linear_url(&args.id, "a milestone UUID")?;
    let project = match args.project.as_deref() {
        Some(project) => Some(ProjectReference::parse(project, &ctx.scope()?)?),
        None => None,
    };
    let client = ctx.client()?;
    let milestone = ctx.spin(true, async {
        let project_id = match &project {
            Some(reference) => Some(refs::project::resolve(client, reference).await?),
            None => None,
        };
        let result: UpdateProjectMilestone = client
            .mutate(UpdateProjectMilestoneVariables {
                id: args.id.clone(),
                input: ProjectMilestoneUpdateInput {
                    name: args.name.clone(),
                    description: args.description.clone(),
                    target_date: args.target_date.map(TimelessDate::from),
                    sort_order: args.sort_order.clone(),
                    project_id,
                },
            })
            .await?;
        let payload = result.project_milestone_update;
        if !payload.success {
            return Err(Error::new("Linear did not update the milestone"));
        }
        Ok(payload.project_milestone)
    })?;
    ctx.print(render(&milestone))
}

fn render(milestone: &UpdatedMilestone) -> String {
    let mut output = outcome::done("Updated", "milestone", &milestone.name, None);
    output.push_str(&format!("  ID: {}\n", milestone.id.inner()));
    if let Some(date) = milestone.target_date {
        output.push_str(&format!("  Target Date: {date}\n"));
    }
    output.push_str(&format!(
        "  Sort Order: {}\n  Project: {}\n",
        milestone.sort_order, milestone.project.name
    ));
    output
}
