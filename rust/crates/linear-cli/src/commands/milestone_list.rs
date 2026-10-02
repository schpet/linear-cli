//! `milestone list`: every page, sorted by target date, as a table or JSON.

use std::cmp::Ordering;
use std::future::Future;

use cynic::QueryBuilder;
use serde::Serialize;

use crate::commands::display::{display_width, fit, flexible_width, pad};
use crate::commands::table::underlined_header;
use crate::error::Error;
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

/// Prefix for every `milestone list` failure.
pub const CONTEXT: &str = "Failed to fetch milestones";

const PAGE_SIZE: i32 = 100;
const ID_WIDTH: usize = 36;
const TARGET_DATE_WIDTH: usize = 12;
const SPACE_WIDTH: usize = 4;
const PADDING: usize = 1;

/// One page request: `first: 100` always, `after` omitted on the first page.
pub fn request(
    project_id: &str,
    after: Option<String>,
) -> GraphQlRequest<GetProjectMilestonesVariables> {
    GraphQlRequest::with_variables(GetProjectMilestones::build(GetProjectMilestonesVariables {
        project_id: project_id.to_owned(),
        first: Some(PAGE_SIZE),
        after,
    }))
}

/// Fetch every page for an already-resolved project, then sort and render.
///
/// `original` is the raw `--project` value; a null `project` root on any page
/// reports it as not found and discards earlier pages. Every returned error
/// carries [`CONTEXT`] exactly once.
pub async fn run_with<F, Fut>(
    original: &str,
    project_id: &str,
    mut fetch: F,
    json: bool,
    columns: usize,
    color: bool,
) -> Result<Vec<u8>, Error>
where
    F: FnMut(GraphQlRequest<GetProjectMilestonesVariables>) -> Fut,
    Fut: Future<Output = Result<GetProjectMilestones, Error>>,
{
    let result = pagination::paginate(|after| {
        let future = fetch(request(project_id, after));
        async move {
            let project = future
                .await?
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
    .map_err(pagination_error)?;

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
    if json {
        render_json(&nodes, &page_info)
    } else {
        Ok(render_text(&nodes, columns, color).into_bytes())
    }
}

pub async fn run(
    transport: &GraphQlTransport,
    original: &str,
    project_id: &str,
    json: bool,
    columns: usize,
    color: bool,
) -> Result<Vec<u8>, Error> {
    run_with(
        original,
        project_id,
        |request| async move { transport.execute(&request).await.map_err(Error::from) },
        json,
        columns,
        color,
    )
    .await
}

fn pagination_error(error: PaginationError<Error>) -> Error {
    match error {
        PaginationError::Fetch { source, .. } => source.context(CONTEXT),
        PaginationError::MissingCursor { .. } => {
            Error::new("Linear reported more milestones but returned no pagination cursor")
                .with_hint("Retry the command.")
                .context(CONTEXT)
        }
        PaginationError::RepeatedCursor { page, .. } => Error::new(format!(
            "Linear repeated a milestone pagination cursor on page {page}"
        ))
        .with_hint("Retry the command.")
        .context(CONTEXT),
    }
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

fn render_json(nodes: &[ProjectMilestone], page_info: &PageInfo) -> Result<Vec<u8>, Error> {
    let nodes = nodes
        .iter()
        .map(|milestone| {
            Ok(JsonMilestone {
                id: &milestone.id,
                name: &milestone.name,
                target_date: &milestone.target_date,
                sort_order: &milestone.sort_order,
                project: JsonProject {
                    id: &milestone.project.id,
                    name: &milestone.project.name,
                },
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let mut output =
        serde_json::to_vec_pretty(&JsonConnection { nodes, page_info }).map_err(|error| {
            Error::new("could not serialize milestones")
                .with_source(error)
                .context(CONTEXT)
        })?;
    output.push(b'\n');
    Ok(output)
}

/// Render the table from already-sorted milestones.
///
/// The project column is clamped to 7..=30 display columns, and the name
/// column is the widest name capped by the remaining width (see
/// [`flexible_width`]), without widening to the `NAME` header.
pub fn render_text(nodes: &[ProjectMilestone], columns: usize, color: bool) -> String {
    if nodes.is_empty() {
        return "No milestones found for this project.\n".to_owned();
    }
    let project_width = nodes
        .iter()
        .map(|milestone| display_width(&milestone.project.name))
        .max()
        .unwrap_or(0)
        .clamp(7, 30);
    let fixed = ID_WIDTH + TARGET_DATE_WIDTH + project_width + SPACE_WIDTH;
    let max_name_width = nodes
        .iter()
        .map(|milestone| display_width(&milestone.name))
        .max()
        .unwrap_or(0);
    let name_width = flexible_width(max_name_width, columns.saturating_sub(PADDING + fixed));
    let mut output = underlined_header(
        &[
            pad("NAME", name_width),
            pad("ID", ID_WIDTH),
            pad("TARGET DATE", TARGET_DATE_WIDTH),
            pad("PROJECT", project_width),
        ],
        color,
    );
    for milestone in nodes {
        output.push_str(&format!(
            "{} {} {} {}\n",
            fit(&milestone.name, name_width),
            pad(milestone.id.inner(), ID_WIDTH),
            pad(
                target_date(milestone).unwrap_or("No date"),
                TARGET_DATE_WIDTH
            ),
            fit(&milestone.project.name, project_width),
        ));
    }
    output
}
