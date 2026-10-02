//! `milestone create`: one mutation after resolving the project.
use cynic::MutationBuilder;

use crate::cli::milestone::MilestoneCreate;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::milestone_create::{
    CreateProjectMilestone, CreateProjectMilestoneVariables, CreatedMilestone,
    ProjectMilestoneCreateInput,
};
use crate::graphql::scalars::TimelessDate;
use crate::graphql::transport::{GraphQlTransport, NetworkPhase, TransportFailure};
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
    client: &GraphQlTransport,
    project_id: String,
    args: &MilestoneCreate,
) -> Result<CreatedMilestone> {
    let request = GraphQlRequest::with_variables(CreateProjectMilestone::build(
        CreateProjectMilestoneVariables {
            input: ProjectMilestoneCreateInput {
                project_id,
                name: args.name.clone(),
                description: args.description.clone(),
                target_date: args.target_date.map(|date| TimelessDate(date.to_string())),
            },
        },
    ));
    let result: CreateProjectMilestone = client.execute(&request).await.map_err(|failure| {
        let uncertain = outcome_unknown(&failure);
        let mut error = Error::from(failure);
        if uncertain {
            error.push_message("; milestone may already exist");
        }
        error
    })?;
    let payload = result.project_milestone_create;
    if !payload.success {
        return Err(Error::new("Linear did not create the milestone"));
    }
    Ok(payload.project_milestone)
}

/// Only a failed connection proves nothing was sent. A timeout, any later
/// network failure (a reset after the request was written surfaces as a
/// request-phase error), or an undecodable success response leaves the
/// create's outcome unknown; errors Linear reported do not.
pub(crate) fn outcome_unknown(failure: &TransportFailure) -> bool {
    match failure {
        TransportFailure::Timeout { .. } | TransportFailure::Response(_) => true,
        TransportFailure::Network { phase, .. } => !matches!(phase, NetworkPhase::Connect),
        TransportFailure::ResponseTooLarge { status, .. } => status.is_success(),
        TransportFailure::RequestBody(_)
        | TransportFailure::GraphQl { .. }
        | TransportFailure::Http { .. } => false,
    }
}

fn render(milestone: &CreatedMilestone) -> String {
    let mut output = format!(
        "✓ Created milestone: {}\n  ID: {}\n",
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
    output.push_str(&format!("  Project: {}\n", milestone.project.name));
    output
}
