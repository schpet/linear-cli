//! `cycle list`: typed pages, stable ordering, and the Deno output shapes.

use cynic::QueryBuilder;
use serde::Serialize;
use std::future::Future;

use crate::commands::display::{display_width, pad, truncate_js};
use crate::error::{AppError, AppErrorKind};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::cycles::{self, GetTeamCycles, GetTeamCyclesVariables};
use crate::graphql::operations::number::WholeNumber;
use crate::graphql::operations::teams::PageInfo;
use crate::graphql::pagination::{self, Page, PaginationError};
use crate::graphql::transport::GraphQlTransport;
use crate::platform::collation;

pub const CONTEXT: &str = "Failed to list cycles";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonCycle<'a> {
    id: &'a cynic::Id,
    number: WholeNumber,
    name: &'a Option<String>,
    starts_at: &'a crate::graphql::scalars::DateTime,
    ends_at: &'a crate::graphql::scalars::DateTime,
    completed_at: &'a Option<crate::graphql::scalars::DateTime>,
    is_active: bool,
    is_future: bool,
    is_past: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonConnection<'a> {
    nodes: Vec<JsonCycle<'a>>,
    page_info: &'a PageInfo,
}

fn render_json(nodes: &[cycles::Cycle], page_info: &PageInfo) -> Result<Vec<u8>, AppError> {
    let nodes = nodes
        .iter()
        .map(|cycle| {
            Ok(JsonCycle {
                id: &cycle.id,
                number: cycle.number,
                name: &cycle.name,
                starts_at: &cycle.starts_at,
                ends_at: &cycle.ends_at,
                completed_at: &cycle.completed_at,
                is_active: cycle.is_active,
                is_future: cycle.is_future,
                is_past: cycle.is_past,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    let mut output =
        serde_json::to_vec_pretty(&JsonConnection { nodes, page_info }).map_err(|error| {
            AppError::new(AppErrorKind::Invariant, "could not serialize cycles")
                .with_source(error)
                .with_context(CONTEXT)
        })?;
    output.push(b'\n');
    Ok(output)
}

pub async fn run_with<F, Fut>(
    team_id: &str,
    mut fetch: F,
    json: bool,
    columns: usize,
    color: bool,
) -> Result<Vec<u8>, AppError>
where
    F: FnMut(GraphQlRequest<GetTeamCyclesVariables>) -> Fut,
    Fut: Future<Output = Result<GetTeamCycles, AppError>>,
{
    let result = pagination::paginate(|after| {
        let request =
            GraphQlRequest::with_variables(GetTeamCycles::build(GetTeamCyclesVariables {
                team_id: team_id.to_owned(),
                first: Some(100),
                after,
            }));
        let future = fetch(request);
        async move {
            let data = future.await?;
            Ok::<Page<cycles::Cycle>, AppError>(Page {
                nodes: data.team.cycles.nodes,
                page_info: data.team.cycles.page_info.into(),
            })
        }
    })
    .await
    .map_err(|error| match error {
        PaginationError::Fetch { source, .. } => source.with_context(CONTEXT),
        PaginationError::MissingCursor { .. } => AppError::new(
            AppErrorKind::Validation,
            "Linear reported more cycles but returned no pagination cursor",
        )
        .with_suggestion("Retry the command.")
        .with_context(CONTEXT),
        PaginationError::RepeatedCursor { page, .. } => AppError::new(
            AppErrorKind::Validation,
            format!("Linear repeated a cycle pagination cursor on page {page}"),
        )
        .with_suggestion("Retry the command.")
        .with_context(CONTEXT),
    })?;

    let mut nodes = result.nodes;
    let collator = collation::root().map_err(|error| error.with_context(CONTEXT))?;
    nodes.sort_by(|left, right| collator.compare(&right.starts_at.0, &left.starts_at.0));
    let page_info = PageInfo {
        has_next_page: result.page_info.has_next_page,
        end_cursor: result.page_info.end_cursor,
    };
    if json {
        render_json(&nodes, &page_info)
    } else {
        Ok(render_text(&nodes, columns, color)?.into_bytes())
    }
}

pub async fn run(
    transport: &GraphQlTransport,
    team_id: &str,
    json: bool,
    columns: usize,
    color: bool,
) -> Result<Vec<u8>, AppError> {
    run_with(
        team_id,
        |request| async move { transport.execute(&request).await.map_err(AppError::from) },
        json,
        columns,
        color,
    )
    .await
}

fn cycle_name(cycle: &cycles::Cycle, number: &str) -> String {
    cycle
        .name
        .as_deref()
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("Cycle {number}"))
}

fn status(cycle: &cycles::Cycle) -> &'static str {
    if cycle.is_active {
        "Active"
    } else if cycle.is_future {
        "Upcoming"
    } else if cycle.completed_at.is_some() {
        "Completed"
    } else if cycle.is_past {
        "Past"
    } else {
        "Unknown"
    }
}

fn date_prefix(date: &str) -> String {
    date.chars().take(10).collect()
}

pub fn render_text(
    nodes: &[cycles::Cycle],
    columns: usize,
    color: bool,
) -> Result<String, AppError> {
    if nodes.is_empty() {
        return Ok("No cycles found for this team.\n".to_owned());
    }
    let numbers: Vec<String> = nodes.iter().map(|cycle| cycle.number.to_string()).collect();
    let names: Vec<_> = nodes
        .iter()
        .zip(&numbers)
        .map(|(cycle, number)| cycle_name(cycle, number))
        .collect();
    let number_width = numbers.iter().map(String::len).max().unwrap_or(0).max(1);
    let fixed = number_width + 10 + 10 + 9 + 4;
    let available = columns.saturating_sub(1).saturating_sub(fixed);
    let max_name_width = names
        .iter()
        .map(|name| display_width(name))
        .max()
        .unwrap_or(0)
        .max(4);
    let name_width = max_name_width.min(available);
    let header = [
        pad("#", number_width),
        pad("NAME", name_width),
        pad("START", 10),
        pad("END", 10),
        pad("STATUS", 9),
    ]
    .join(" ");
    let mut output = if color {
        format!("\x1b[1m\x1b[4m{header}\x1b[24m\x1b[22m\n")
    } else {
        format!("{header}\n")
    };
    for ((cycle, number), name) in nodes.iter().zip(&numbers).zip(&names) {
        let label = pad(status(cycle), 9);
        let styled = if !color {
            label
        } else if cycle.is_active {
            format!("\x1b[32m{label}\x1b[39m")
        } else if cycle.is_past || cycle.completed_at.is_some() {
            format!("\x1b[90m{label}\x1b[39m")
        } else {
            label
        };
        output.push_str(&format!(
            "{} {} {} {} {styled}\n",
            pad(number, number_width),
            truncate_js(name, name_width),
            pad(&date_prefix(&cycle.starts_at.0), 10),
            pad(&date_prefix(&cycle.ends_at.0), 10),
        ));
    }
    Ok(output)
}
