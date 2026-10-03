//! `milestone list`: every page, sorted by target date, as a table or JSON.
use std::cmp::Ordering;

use crate::cli::milestone::MilestoneList;
use crate::client::LinearClient;
use crate::commands::json;
use crate::commands::table::{Cell, Column, Table};
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::milestone::{
    GetProjectMilestones, GetProjectMilestonesVariables, ProjectMilestone,
};
use crate::graphql::pagination::{self, Page};
use crate::platform::collation;
use crate::refs::{prepare_project_lookup, resolve_project_with_transport};

pub fn run(ctx: &Ctx, args: &MilestoneList) -> Result<()> {
    list(ctx, args).context("Failed to list milestones")
}

fn list(ctx: &Ctx, args: &MilestoneList) -> Result<()> {
    let project = prepare_project_lookup(&args.project, &ctx.scope()?)?;
    let client = ctx.client()?;
    let mut milestones = ctx.spin(!args.json, async {
        let project_id = resolve_project_with_transport(&project, &args.project, client).await?;
        fetch(client, &args.project, &project_id).await
    })?;
    args.limit.apply(&mut milestones);
    if args.json {
        ctx.print(json::render(&milestones))
    } else if milestones.is_empty() {
        ctx.print("No milestones found for this project.\n")
    } else {
        ctx.print(render_text(&milestones).render_for(ctx))
    }
}

/// Every milestone of the project, sorted by target date (undated last), then name.
async fn fetch(
    client: &LinearClient,
    original: &str,
    project_id: &str,
) -> Result<Vec<ProjectMilestone>> {
    let mut nodes = pagination::collect(None, |after, first| {
        let variables = GetProjectMilestonesVariables {
            project_id: project_id.to_owned(),
            first: Some(first),
            after,
        };
        async move {
            let data: GetProjectMilestones = client.query(variables).await?;
            let project = data
                .project
                .ok_or_else(|| Error::not_found("Project", original))?;
            let connection = project.project_milestones;
            Ok(Page {
                nodes: connection.nodes,
                page_info: connection.page_info,
            })
        }
    })
    .await?;
    nodes.sort_by(|left, right| {
        let name = || collation::compare(&left.name, &right.name);
        // Milestones without a target date sort last.
        match (left.target_date, right.target_date) {
            (None, None) => name(),
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (Some(left), Some(right)) => left.cmp(&right).then_with(name),
        }
    });
    Ok(nodes)
}

fn render_text(nodes: &[ProjectMilestone]) -> Table {
    let mut table = Table::new([
        Column::flexible("NAME"),
        Column::fixed("ID"),
        Column::fixed("TARGET DATE"),
        Column::flexible("PROJECT"),
    ]);
    for milestone in nodes {
        table.row([
            Cell::from(milestone.name.as_str()),
            Cell::from(milestone.id.inner()),
            Cell::from(
                milestone
                    .target_date
                    .map_or_else(|| "No date".to_owned(), |date| date.to_string()),
            ),
            Cell::from(milestone.project.name.as_str()),
        ]);
    }
    table
}
