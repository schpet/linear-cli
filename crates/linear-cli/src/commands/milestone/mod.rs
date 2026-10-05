//! `linear milestone`: project milestones.
pub mod create;
mod delete;
mod list;
mod update;
mod view;

use crate::cli::milestone::MilestoneCommand;
use crate::client::LinearClient;
use crate::ctx::Ctx;
use crate::error::{Error, Result};
use crate::graphql::operations::milestone::{GetProjectMilestonesForLookup, LookupVariables};

pub fn run(ctx: &Ctx, command: &MilestoneCommand) -> Result<()> {
    match command {
        MilestoneCommand::List(args) => list::run(ctx, args),
        MilestoneCommand::View(args) => view::run(ctx, args),
        MilestoneCommand::Create(args) => create::run(ctx, args),
        MilestoneCommand::Update(args) => update::run(ctx, args),
        MilestoneCommand::Delete(args) => delete::run(ctx, args),
    }
}

/// The ID of the project's milestone named `name`, ignoring case.
pub async fn id_by_name(client: &LinearClient, project_id: &str, name: &str) -> Result<String> {
    let data: GetProjectMilestonesForLookup = client
        .query(LookupVariables {
            project_id: project_id.to_owned(),
            name: name.to_owned(),
        })
        .await?;
    data.project
        .ok_or_else(|| Error::not_found("Project", project_id))?
        .project_milestones
        .and_then(|connection| connection.nodes.into_iter().next())
        .map(|milestone| milestone.id.into_inner())
        .ok_or_else(|| Error::not_found("Milestone", name))
}
