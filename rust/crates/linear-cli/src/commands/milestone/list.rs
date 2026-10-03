//! `milestone list`: every page, sorted by target date, as a table or JSON.
use std::cmp::Ordering;

use cynic::QueryBuilder;

use crate::cli::milestone::MilestoneList;
use crate::client::LinearClient;
use crate::commands::json;
use crate::commands::table::{Cell, Column, Table};
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::milestones::{
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
        let request = GraphQlRequest::with_variables(GetProjectMilestones::build(
            GetProjectMilestonesVariables {
                project_id: project_id.to_owned(),
                first: Some(first),
                after,
            },
        ));
        async move {
            let data: GetProjectMilestones = client.execute(&request).await?;
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
        match (target_date(left), target_date(right)) {
            (None, None) => name(),
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (Some(left), Some(right)) => collation::compare(left, right).then_with(name),
        }
    });
    Ok(nodes)
}

/// A null or empty target date sorts last and prints `No date`.
fn target_date(milestone: &ProjectMilestone) -> Option<&str> {
    milestone
        .target_date
        .as_ref()
        .map(|date| date.0.as_str())
        .filter(|date| !date.is_empty())
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
            Cell::from(target_date(milestone).unwrap_or("No date")),
            Cell::from(milestone.project.name.as_str()),
        ]);
    }
    table
}
