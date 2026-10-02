//! `team list`: complete typed pagination and the two output formats.
use std::future::Future;
use std::time::SystemTime;

use cynic::QueryBuilder;
use serde::Serialize;

use crate::commands::display::{display_width, fit, flexible_width, pad};
use crate::commands::relative_time::format_relative_time;
use crate::commands::table::{terminal_color, underlined_header};
use crate::config::ConfigOptions;
use crate::error::{AppError, AppErrorKind};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::teams::{self, GetTeams, GetTeamsVariables};
use crate::graphql::pagination::{self, Page, PaginationError};
use crate::graphql::transport::GraphQlTransport;
use crate::platform::{collation, opener};

const CONTEXT: &str = "Failed to fetch teams";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Options {
    pub json: bool,
    pub web: bool,
    pub app: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonConnection<'a> {
    nodes: &'a [teams::Team],
    page_info: &'a teams::PageInfo,
}

/// The team settings URL for `--workspace`, or else the configured workspace.
pub fn web_opening(
    cli_workspace: Option<&str>,
    options: &ConfigOptions,
    app: bool,
) -> Result<(String, Vec<u8>), AppError> {
    let workspace = cli_workspace
        .or_else(|| {
            options
                .workspace()
                .map(|resolved| resolved.value().as_str())
        })
        .filter(|workspace| !workspace.is_empty())
        .ok_or_else(|| {
            AppError::new(
                AppErrorKind::Validation,
                "workspace is not set via command line, configuration file, or environment",
            )
            .with_context(CONTEXT)
        })?;
    let url = format!("https://linear.app/{workspace}/settings/teams");
    let destination = if app { "Linear.app" } else { "web browser" };
    let line = format!("Opening {url} in {destination}\n").into_bytes();
    Ok((url, line))
}

pub fn open(url: &str, app: bool) -> Result<(), AppError> {
    opener::open(url, app).map_err(|error| error.with_context(CONTEXT))
}

pub async fn run_with<F, Fut, Now>(
    mut fetch: F,
    json: bool,
    now: Now,
    columns: usize,
) -> Result<Vec<u8>, AppError>
where
    F: FnMut(GraphQlRequest<GetTeamsVariables>) -> Fut,
    Fut: Future<Output = Result<GetTeams, AppError>>,
    Now: FnOnce() -> SystemTime,
{
    run_with_style(&mut fetch, json, now, columns, false).await
}

async fn run_with_style<F, Fut, Now>(
    mut fetch: F,
    json: bool,
    now: Now,
    columns: usize,
    color: bool,
) -> Result<Vec<u8>, AppError>
where
    F: FnMut(GraphQlRequest<GetTeamsVariables>) -> Fut,
    Fut: Future<Output = Result<GetTeams, AppError>>,
    Now: FnOnce() -> SystemTime,
{
    let result = pagination::paginate(|after| {
        let request = GraphQlRequest::with_variables(GetTeams::build(GetTeamsVariables {
            filter: None,
            first: Some(100),
            after,
        }));
        let future = fetch(request);
        async move {
            let data = future.await?;
            Ok::<Page<teams::Team>, AppError>(Page {
                nodes: data.teams.nodes,
                page_info: data.teams.page_info.into(),
            })
        }
    })
    .await
    .map_err(|error| match error {
        PaginationError::Fetch { source, .. } => source.with_context(CONTEXT),
        PaginationError::MissingCursor { .. } => AppError::new(
            AppErrorKind::Validation,
            "Linear reported more teams but returned no pagination cursor",
        )
        .with_suggestion("Retry the command.")
        .with_context(CONTEXT),
        PaginationError::RepeatedCursor { page, .. } => AppError::new(
            AppErrorKind::Validation,
            format!("Linear repeated a team pagination cursor on page {page}"),
        )
        .with_suggestion("Retry the command.")
        .with_context(CONTEXT),
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
    let collator = collation::root().map_err(|error| error.with_context(CONTEXT))?;
    teams.sort_by(|left, right| collator.compare(&left.name, &right.name));
    if json {
        let page_info = teams::PageInfo {
            has_next_page: result.page_info.has_next_page,
            end_cursor: result.page_info.end_cursor,
        };
        let mut output = serde_json::to_vec_pretty(&JsonConnection {
            nodes: &teams,
            page_info: &page_info,
        })
        .map_err(|error| {
            AppError::new(AppErrorKind::Invariant, "could not serialize teams")
                .with_source(error)
                .with_context(CONTEXT)
        })?;
        output.push(b'\n');
        return Ok(output);
    }
    Ok(render_text(&teams, now(), columns, color).into_bytes())
}

pub async fn run(
    transport: &GraphQlTransport,
    json: bool,
    columns: usize,
    color: bool,
) -> Result<Vec<u8>, AppError> {
    run_with_style(
        |request| async move { transport.execute(&request).await.map_err(AppError::from) },
        json,
        SystemTime::now,
        columns,
        color,
    )
    .await
}

pub fn render_text(teams: &[teams::Team], now: SystemTime, columns: usize, color: bool) -> String {
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
    ];
    let mut output = underlined_header(&header, color);
    for (team, updated) in teams.iter().zip(updated) {
        let cycles = if team.cycles_enabled { "Yes" } else { "No" };
        let key = pad(&team.key, key_width);
        let name = fit(&team.name, name_width);
        let cycles = pad(cycles, cycles_width);
        let updated = pad(&updated, updated_width);
        let id = pad(team.id.inner(), id_width);
        if color {
            let key_color = terminal_color(
                team.color
                    .as_deref()
                    .filter(|value| !value.is_empty())
                    .unwrap_or("#ffffff"),
            );
            if let Some(key_color) = &key_color {
                output.push_str(key_color);
            }
            output.push_str(&key);
            if key_color.is_some() {
                output.push_str("\x1b[39m");
            }
            output.push_str(&format!(
                " {name} {cycles} \x1b[38;2;128;128;128m{updated}\x1b[39m \x1b[38;2;128;128;128m{id}\x1b[39m\x1b[0m\n"
            ));
        } else {
            output.push_str(&format!("{key} {name} {cycles} {updated} {id}\n"));
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::display_width;

    #[test]
    fn width_examples_match_frozen_deno_per_codepoint_measurement() {
        for (name, expected) in [
            ("漢", 2),
            ("e\u{301}", 1),
            ("👩‍💻", 4),
            ("❤️", 1),
            ("🇺🇸", 2),
            ("\u{7}", 0),
        ] {
            assert_eq!(display_width(name), expected, "{name:?}");
        }
    }
}
