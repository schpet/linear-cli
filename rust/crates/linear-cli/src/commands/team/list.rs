//! `team list`: every team that is not archived, by name, as a table or JSON.
use std::time::SystemTime;

use cynic::QueryBuilder;
use serde::Serialize;

use crate::cli::team::TeamList;
use crate::commands::relative_time::format_relative_time;
use crate::commands::table::{Cell, Column, Table};
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::teams::{self, GetTeams, GetTeamsVariables};
use crate::graphql::pagination::{self, Page, PaginationError};
use crate::graphql::transport::GraphQlTransport;
use crate::platform::{collation, style};

pub fn run(ctx: &Ctx, args: &TeamList) -> Result<()> {
    list(ctx, args).context("Failed to list teams")
}

fn list(ctx: &Ctx, args: &TeamList) -> Result<()> {
    if args.web || args.app {
        return ctx.open_in_linear("settings/teams", args.app);
    }
    let client = ctx.client()?;
    let (teams, page_info) = ctx.spin(!args.json, fetch(client))?;
    if args.json {
        ctx.print(render_json(&teams, &page_info))
    } else if teams.is_empty() {
        ctx.print("No teams found.\n")
    } else {
        ctx.print(render_text(&teams, SystemTime::now()).render_for(ctx))
    }
}

/// Every team that is not archived, sorted by name.
async fn fetch(client: &GraphQlTransport) -> Result<(Vec<teams::Team>, teams::PageInfo)> {
    let result = pagination::paginate(|after| {
        let request = GraphQlRequest::with_variables(GetTeams::build(GetTeamsVariables {
            filter: None,
            first: Some(100),
            after,
        }));
        async move {
            let data: GetTeams = client.execute(&request).await?;
            Ok::<Page<teams::Team>, Error>(Page {
                nodes: data.teams.nodes,
                page_info: data.teams.page_info.into(),
            })
        }
    })
    .await
    .map_err(|error| match error {
        PaginationError::Fetch { source, .. } => source,
        PaginationError::MissingCursor { .. } => {
            Error::new("Linear reported more teams but returned no pagination cursor")
                .with_hint("Retry the command.")
        }
        PaginationError::RepeatedCursor { page, .. } => Error::new(format!(
            "Linear repeated a team pagination cursor on page {page}"
        ))
        .with_hint("Retry the command."),
    })?;
    let mut teams: Vec<_> = result
        .nodes
        .into_iter()
        .filter(|team| {
            team.archived_at
                .as_ref()
                .is_none_or(|date| date.0.is_empty())
        })
        .collect();
    teams.sort_by(|left, right| collation::compare(&left.name, &right.name));
    let page_info = teams::PageInfo {
        has_next_page: result.page_info.has_next_page,
        end_cursor: result.page_info.end_cursor,
    };
    Ok((teams, page_info))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonConnection<'a> {
    nodes: &'a [teams::Team],
    page_info: &'a teams::PageInfo,
}

fn render_json(teams: &[teams::Team], page_info: &teams::PageInfo) -> Vec<u8> {
    let mut output = serde_json::to_vec_pretty(&JsonConnection {
        nodes: teams,
        page_info,
    })
    .expect("team JSON always serializes");
    output.push(b'\n');
    output
}

fn render_text(teams: &[teams::Team], now: SystemTime) -> Table {
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
                format_relative_time(&team.updated_at.0, now.into(), &chrono::Local),
                style::gray,
            ),
            Cell::styled(team.id.inner(), style::gray),
        ]);
    }
    table
}
