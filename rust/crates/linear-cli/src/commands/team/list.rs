//! `team list`: every team that is not archived, by name, as a table or JSON.
use std::time::SystemTime;

use cynic::QueryBuilder;
use serde::Serialize;

use crate::cli::team::TeamList;
use crate::commands::display::{display_width, fit, flexible_width, pad};
use crate::commands::relative_time::format_relative_time;
use crate::commands::table;
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
    } else {
        let columns = table::stdout_columns(ctx.stdout_tty());
        ctx.print(render_text(&teams, SystemTime::now(), columns, ctx.color()))
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

fn render_text(teams: &[teams::Team], now: SystemTime, columns: usize, color: bool) -> String {
    if teams.is_empty() {
        return "No teams found.\n".to_owned();
    }
    let id_width = teams
        .iter()
        .map(|team| display_width(team.id.inner()))
        .max()
        .unwrap_or(0)
        .max(2);
    let key_width = teams
        .iter()
        .map(|team| display_width(&team.key))
        .max()
        .unwrap_or(0)
        .max(3);
    let updated: Vec<_> = teams
        .iter()
        .map(|team| format_relative_time(&team.updated_at.0, now.into(), &chrono::Local))
        .collect();
    let updated_width = updated
        .iter()
        .map(|value| display_width(value))
        .max()
        .unwrap_or(0)
        .max(7);
    let cycles_width = 6;
    let fixed = id_width + key_width + cycles_width + updated_width + 5;
    let available_width = columns.saturating_sub(1).saturating_sub(fixed);
    let max_name_width = teams
        .iter()
        .map(|team| display_width(&team.name))
        .max()
        .unwrap_or(0);
    let name_width = flexible_width(max_name_width, available_width);
    let header = [
        pad("KEY", key_width),
        pad("NAME", name_width),
        pad("CYCLES", cycles_width),
        pad("UPDATED", updated_width),
        pad("ID", id_width),
    ]
    .join(" ");
    let mut output = format!(
        "{}\n",
        style::bold(&style::underline(&header, color), color)
    );
    for (team, updated) in teams.iter().zip(updated) {
        let cycles = if team.cycles_enabled { "Yes" } else { "No" };
        output.push_str(&format!(
            "{} {} {} {} {}\n",
            team_color(&pad(&team.key, key_width), team.color.as_deref(), color),
            fit(&team.name, name_width),
            pad(cycles, cycles_width),
            style::gray(&pad(&updated, updated_width), color),
            style::gray(&pad(team.id.inner(), id_width), color),
        ));
    }
    output
}

/// `text` in the team's `#rrggbb` color; plain when the color is missing or malformed.
fn team_color(text: &str, hex: Option<&str>, color: bool) -> String {
    let rgb = hex
        .and_then(|hex| hex.strip_prefix('#'))
        .filter(|hex| hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .and_then(|hex| u32::from_str_radix(hex, 16).ok());
    match rgb {
        Some(rgb) if color => {
            let [_, red, green, blue] = rgb.to_be_bytes();
            console::Style::new()
                .true_color(red, green, blue)
                .force_styling(true)
                .apply_to(text)
                .to_string()
        }
        Some(_) | None => text.to_owned(),
    }
}
