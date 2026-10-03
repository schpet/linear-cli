//! `cycle list`: every page, newest first, as a table or JSON.

use crate::cli::cycle::CycleList;
use crate::client::LinearClient;
use crate::commands::json;
use crate::commands::table::{Cell, Column, Table};
use crate::commands::team_key::team_or_configured;
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};
use crate::graphql::operations::cycle::{self, GetTeamCycles, GetTeamCyclesVariables};
use crate::graphql::pagination::{self, Page};
use crate::platform::style;
use crate::refs::{prepare_team_lookup, resolve_team_with_transport};

pub fn run(ctx: &Ctx, args: &CycleList) -> Result<()> {
    list(ctx, args).context("Failed to list cycles")
}

fn list(ctx: &Ctx, args: &CycleList) -> Result<()> {
    let team = team_or_configured(ctx, args.team.as_deref())?;
    let lookup = prepare_team_lookup(&team, &ctx.scope()?)?;
    let client = ctx.client()?;
    let mut cycles = ctx.spin(!args.json, async {
        let team = resolve_team_with_transport(&lookup, client).await?;
        fetch(client, &team.id).await
    })?;
    args.limit.apply(&mut cycles);
    if args.json {
        ctx.print(json::render(&cycles))
    } else if cycles.is_empty() {
        ctx.print("No cycles found for this team.\n")
    } else {
        ctx.print(render_text(&cycles).render_for(ctx))
    }
}

/// Every cycle of the team, newest first.
async fn fetch(client: &LinearClient, team_id: &str) -> Result<Vec<cycle::Cycle>> {
    let mut nodes = pagination::collect(None, |after, first| {
        let variables = GetTeamCyclesVariables {
            team_id: team_id.to_owned(),
            first: Some(first),
            after,
        };
        async move {
            let data: GetTeamCycles = client.query(variables).await?;
            Ok(Page {
                nodes: data.team.cycles.nodes,
                page_info: data.team.cycles.page_info,
            })
        }
    })
    .await?;
    nodes.sort_by(|left, right| right.starts_at.cmp(&left.starts_at));
    Ok(nodes)
}

fn cycle_name(cycle: &cycle::Cycle, number: &str) -> String {
    cycle
        .name
        .as_deref()
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("Cycle {number}"))
}

fn status(cycle: &cycle::Cycle) -> &'static str {
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

fn render_text(nodes: &[cycle::Cycle]) -> Table {
    let mut table = Table::new([
        Column::fixed("#"),
        Column::flexible("NAME"),
        Column::fixed("START"),
        Column::fixed("END"),
        Column::fixed("STATUS"),
    ]);
    for cycle in nodes {
        let number = cycle.number.to_string();
        let name = cycle_name(cycle, &number);
        let status = if cycle.is_active {
            Cell::styled(status(cycle), style::green)
        } else if cycle.is_past || cycle.completed_at.is_some() {
            Cell::styled(status(cycle), style::gray)
        } else {
            Cell::from(status(cycle))
        };
        table.row([
            Cell::from(number),
            Cell::from(name),
            Cell::from(cycle.starts_at.0.date_naive().to_string()),
            Cell::from(cycle.ends_at.0.date_naive().to_string()),
            status,
        ]);
    }
    table
}
