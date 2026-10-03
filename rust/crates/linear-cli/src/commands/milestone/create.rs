//! `milestone create`: one mutation after resolving the project.
use crate::cli::milestone::MilestoneCreate;
use crate::client::LinearClient;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::milestone::{
    CreateProjectMilestone, CreateProjectMilestoneVariables, CreatedMilestone,
    ProjectMilestoneCreateInput,
};
use crate::graphql::scalars::TimelessDate;
use crate::refs::{prepare_project_lookup, resolve_project_with_transport};

pub fn run(ctx: &Ctx, args: &MilestoneCreate) -> Result<()> {
    create(ctx, args).context("Failed to create milestone")
}

fn create(ctx: &Ctx, args: &MilestoneCreate) -> Result<()> {
    let project = prepare_project_lookup(&args.project, &ctx.scope()?)?;
    let client = ctx.client()?;
    let milestone = ctx.spin(true, async {
        let project_id = resolve_project_with_transport(&project, &args.project, client).await?;
        submit(client, project_id, args).await
    })?;
    ctx.print(render(&milestone))
}

/// Sends the mutation once. A failure after the request may have reached
/// Linear says the milestone may already exist; nothing is retried.
async fn submit(
    client: &LinearClient,
    project_id: String,
    args: &MilestoneCreate,
) -> Result<CreatedMilestone> {
    let result: CreateProjectMilestone = client
        .mutate(CreateProjectMilestoneVariables {
            input: ProjectMilestoneCreateInput {
                project_id,
                name: args.name.clone(),
                description: args.description.clone(),
                target_date: args.target_date.map(TimelessDate::from),
            },
        })
        .await
        .map_err(|failure| failure.into_create_error("milestone"))?;
    let payload = result.project_milestone_create;
    if !payload.success {
        return Err(Error::new("Linear did not create the milestone"));
    }
    Ok(payload.project_milestone)
}

fn render(milestone: &CreatedMilestone) -> String {
    let mut output = format!(
        "✓ Created milestone: {}\n  ID: {}\n",
        milestone.name,
        milestone.id.inner()
    );
    if let Some(date) = milestone.target_date {
        output.push_str(&format!("  Target Date: {date}\n"));
    }
    output.push_str(&format!("  Project: {}\n", milestone.project.name));
    output
}
