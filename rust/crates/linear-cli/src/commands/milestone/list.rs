//! `milestone list`: every page, sorted by target date, as a table or JSON.
use std::cmp::Ordering;

use cynic::QueryBuilder;
use serde::Serialize;

use crate::cli::milestone::MilestoneList;
use crate::commands::table::{Cell, Column, Table};
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::milestones::{
    GetProjectMilestones, GetProjectMilestonesVariables, ProjectMilestone,
};
use crate::graphql::operations::number::Float;
use crate::graphql::operations::teams::PageInfo;
use crate::graphql::pagination::{self, Page, PaginationError};
use crate::graphql::scalars::TimelessDate;
use crate::graphql::transport::GraphQlTransport;
use crate::platform::collation;
use crate::refs::{prepare_project_lookup, resolve_project_with_transport};

pub fn run(ctx: &Ctx, args: &MilestoneList) -> Result<()> {
    list(ctx, args).context("Failed to list milestones")
}

fn list(ctx: &Ctx, args: &MilestoneList) -> Result<()> {
    let project = prepare_project_lookup(&args.project, &ctx.scope()?)?;
    let client = ctx.client()?;
    let (milestones, page_info) = ctx.spin(!args.json, async {
        let project_id = resolve_project_with_transport(&project, &args.project, client).await?;
        fetch(client, &args.project, &project_id).await
    })?;
    if args.json {
        ctx.print(render_json(&milestones, &page_info))
    } else if milestones.is_empty() {
        ctx.print("No milestones found for this project.\n")
    } else {
        ctx.print(render_text(&milestones).render_for(ctx))
    }
}

/// Every milestone of the project, sorted by target date (undated last), then name.
async fn fetch(
    client: &GraphQlTransport,
    original: &str,
    project_id: &str,
) -> Result<(Vec<ProjectMilestone>, PageInfo)> {
    let result = pagination::paginate(|after| {
        let request = GraphQlRequest::with_variables(GetProjectMilestones::build(
            GetProjectMilestonesVariables {
                project_id: project_id.to_owned(),
                first: Some(100),
                after,
            },
        ));
        async move {
            let data: GetProjectMilestones = client.execute(&request).await?;
            let project = data
                .project
                .ok_or_else(|| Error::not_found("Project", original))?;
            let connection = project.project_milestones;
            Ok::<Page<ProjectMilestone>, Error>(Page {
                nodes: connection.nodes,
                page_info: connection.page_info.into(),
            })
        }
    })
    .await
    .map_err(|error| match error {
        PaginationError::Fetch { source, .. } => source,
        PaginationError::MissingCursor { .. } => {
            Error::new("Linear reported more milestones but returned no pagination cursor")
                .with_hint("Retry the command.")
        }
        PaginationError::RepeatedCursor { page, .. } => Error::new(format!(
            "Linear repeated a milestone pagination cursor on page {page}"
        ))
        .with_hint("Retry the command."),
    })?;
    let mut nodes = result.nodes;
    nodes.sort_by(|left, right| {
        let name = || collation::compare(&left.name, &right.name);
        match (target_date(left), target_date(right)) {
            (None, None) => name(),
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (Some(left), Some(right)) => collation::compare(left, right).then_with(name),
        }
    });
    let page_info = PageInfo {
        has_next_page: result.page_info.has_next_page,
        end_cursor: result.page_info.end_cursor,
    };
    Ok((nodes, page_info))
}

/// A null or empty target date sorts last and prints `No date`.
fn target_date(milestone: &ProjectMilestone) -> Option<&str> {
    milestone
        .target_date
        .as_ref()
        .map(|date| date.0.as_str())
        .filter(|date| !date.is_empty())
}

#[derive(Serialize)]
struct JsonProject<'a> {
    id: &'a cynic::Id,
    name: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonMilestone<'a> {
    id: &'a cynic::Id,
    name: &'a str,
    target_date: &'a Option<TimelessDate>,
    sort_order: &'a Float,
    project: JsonProject<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonConnection<'a> {
    nodes: Vec<JsonMilestone<'a>>,
    page_info: &'a PageInfo,
}

fn render_json(nodes: &[ProjectMilestone], page_info: &PageInfo) -> Vec<u8> {
    let nodes = nodes
        .iter()
        .map(|milestone| JsonMilestone {
            id: &milestone.id,
            name: &milestone.name,
            target_date: &milestone.target_date,
            sort_order: &milestone.sort_order,
            project: JsonProject {
                id: &milestone.project.id,
                name: &milestone.project.name,
            },
        })
        .collect();
    let mut output = serde_json::to_vec_pretty(&JsonConnection { nodes, page_info })
        .expect("milestone JSON always serializes");
    output.push(b'\n');
    output
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
