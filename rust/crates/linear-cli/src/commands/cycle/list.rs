//! `cycle list`: every page, newest first, as a table or JSON.

use cynic::QueryBuilder;
use serde::Serialize;

use crate::cli::cycle::CycleList;
use crate::commands::display::{display_width, fit, flexible_width, pad};
use crate::commands::table;
use crate::commands::team_key::team_or_configured;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::cycles::{self, GetTeamCycles, GetTeamCyclesVariables};
use crate::graphql::operations::number::WholeNumber;
use crate::graphql::operations::teams::PageInfo;
use crate::graphql::pagination::{self, Page, PaginationError};
use crate::graphql::transport::GraphQlTransport;
use crate::platform::{collation, style};
use crate::refs::{prepare_team_lookup, resolve_team_with_transport};

pub fn run(ctx: &Ctx, args: &CycleList) -> Result<()> {
    list(ctx, args).context("Failed to list cycles")
}

fn list(ctx: &Ctx, args: &CycleList) -> Result<()> {
    let team = team_or_configured(ctx, args.team.as_deref())?;
    let lookup = prepare_team_lookup(&team, &ctx.scope()?)?;
    let client = ctx.client()?;
    let (cycles, page_info) = ctx.spin(!args.json, async {
        let team = resolve_team_with_transport(&lookup, client).await?;
        fetch(client, &team.id).await
    })?;
    if args.json {
        ctx.print(render_json(&cycles, &page_info))
    } else {
        let columns = table::stdout_columns(ctx.stdout_tty());
        ctx.print(render_text(&cycles, columns, ctx.color()))
    }
}

/// Every cycle of the team, newest first, with the last page's info.
async fn fetch(client: &GraphQlTransport, team_id: &str) -> Result<(Vec<cycles::Cycle>, PageInfo)> {
    let result = pagination::paginate(|after| {
        let request =
            GraphQlRequest::with_variables(GetTeamCycles::build(GetTeamCyclesVariables {
                team_id: team_id.to_owned(),
                first: Some(100),
                after,
            }));
        async move {
            let data: GetTeamCycles = client.execute(&request).await?;
            Ok::<Page<cycles::Cycle>, Error>(Page {
                nodes: data.team.cycles.nodes,
                page_info: data.team.cycles.page_info.into(),
            })
        }
    })
    .await
    .map_err(|error| match error {
        PaginationError::Fetch { source, .. } => source,
        PaginationError::MissingCursor { .. } => {
            Error::new("Linear reported more cycles but returned no pagination cursor")
                .with_hint("Retry the command.")
        }
        PaginationError::RepeatedCursor { page, .. } => Error::new(format!(
            "Linear repeated a cycle pagination cursor on page {page}"
        ))
        .with_hint("Retry the command."),
    })?;
    let mut nodes = result.nodes;
    nodes.sort_by(|left, right| collation::compare(&right.starts_at.0, &left.starts_at.0));
    let page_info = PageInfo {
        has_next_page: result.page_info.has_next_page,
        end_cursor: result.page_info.end_cursor,
    };
    Ok((nodes, page_info))
}

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

fn render_json(nodes: &[cycles::Cycle], page_info: &PageInfo) -> Vec<u8> {
    let nodes = nodes
        .iter()
        .map(|cycle| JsonCycle {
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
        .collect();
    let mut output = serde_json::to_vec_pretty(&JsonConnection { nodes, page_info })
        .expect("cycle JSON always serializes");
    output.push(b'\n');
    output
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

fn render_text(nodes: &[cycles::Cycle], columns: usize, color: bool) -> String {
    if nodes.is_empty() {
        return "No cycles found for this team.\n".to_owned();
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
    let name_width = flexible_width(max_name_width, available);
    let header = [
        pad("#", number_width),
        pad("NAME", name_width),
        pad("START", 10),
        pad("END", 10),
        pad("STATUS", 9),
    ]
    .join(" ");
    let mut output = format!(
        "{}\n",
        style::bold(&style::underline(&header, color), color)
    );
    for ((cycle, number), name) in nodes.iter().zip(&numbers).zip(&names) {
        let label = pad(status(cycle), 9);
        let styled = if cycle.is_active {
            style::green(&label, color)
        } else if cycle.is_past || cycle.completed_at.is_some() {
            style::gray(&label, color)
        } else {
            label
        };
        output.push_str(&format!(
            "{} {} {} {} {styled}\n",
            pad(number, number_width),
            fit(name, name_width),
            pad(&date_prefix(&cycle.starts_at.0), 10),
            pad(&date_prefix(&cycle.ends_at.0), 10),
        ));
    }
    output
}
