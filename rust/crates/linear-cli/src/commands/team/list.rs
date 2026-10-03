//! `team list`: every team that is not archived, by name, as a table or JSON.
use std::time::SystemTime;

use crate::cli::team::TeamList;
use crate::client::LinearClient;
use crate::commands::json;
use crate::commands::relative_time::ago;
use crate::commands::table::{Cell, Column, Table};
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};
use crate::graphql::operations::team::{self, GetTeams, GetTeamsVariables};
use crate::graphql::pagination::{self, Page};
use crate::platform::{collation, style};

pub fn run(ctx: &Ctx, args: &TeamList) -> Result<()> {
    list(ctx, args).context("Failed to list teams")
}

fn list(ctx: &Ctx, args: &TeamList) -> Result<()> {
    if args.web || args.app {
        return ctx.open_in_linear("settings/teams", args.app);
    }
    let client = ctx.client()?;
    let mut teams = ctx.spin(!args.json, fetch(client))?;
    args.limit.apply(&mut teams);
    if args.json {
        ctx.print(json::render(&teams))
    } else if teams.is_empty() {
        ctx.print("No teams found.\n")
    } else {
        ctx.print(render_text(&teams, SystemTime::now()).render_for(ctx))
    }
}

/// Every team that is not archived, sorted by name.
async fn fetch(client: &LinearClient) -> Result<Vec<team::Team>> {
    let teams = pagination::collect(None, |after, first| {
        let variables = GetTeamsVariables {
            filter: None,
            first: Some(first),
            after,
        };
        async move {
            let data: GetTeams = client.query(variables).await?;
            Ok(Page {
                nodes: data.teams.nodes,
                page_info: data.teams.page_info,
            })
        }
    })
    .await?;
    let mut teams: Vec<_> = teams
        .into_iter()
        .filter(|team| team.archived_at.is_none())
        .collect();
    teams.sort_by(|left, right| collation::compare(&left.name, &right.name));
    Ok(teams)
}

fn render_text(teams: &[team::Team], now: SystemTime) -> Table {
    let mut table = Table::new([
        Column::fixed("KEY"),
        Column::flexible("NAME"),
        Column::fixed("CYCLES"),
        Column::fixed("UPDATED"),
        Column::fixed("ID"),
    ]);
    for team in teams {
        let hex = team.color.clone().unwrap_or_default();
        table.row([
            Cell::styled(team.key.as_str(), move |text, on| {
                style::rgb(text, &hex, on)
            }),
            Cell::from(team.name.as_str()),
            Cell::from(if team.cycles_enabled { "Yes" } else { "No" }),
            Cell::styled(
                ago(team.updated_at.0, now.into(), &chrono::Local),
                style::gray,
            ),
            Cell::styled(team.id.inner(), style::gray),
        ]);
    }
    table
}
