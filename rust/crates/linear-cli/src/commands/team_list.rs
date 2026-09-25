//! `team list`: complete typed pagination and the two output formats.
use std::future::Future;
use std::time::SystemTime;

use chrono::{DateTime, NaiveDate, Utc};
use cynic::QueryBuilder;
use serde::Serialize;
use unicode_width::UnicodeWidthChar;

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

/// A web/app request deliberately ignores the inherited CLI workspace flag.
/// The frozen command calls `getOption("workspace")` without the parsed value.
pub fn web_opening(options: &ConfigOptions, app: bool) -> Result<(String, Vec<u8>), AppError> {
    let workspace = options
        .workspace()
        .map(|resolved| resolved.value().as_str())
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

fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

fn display_width(text: &str) -> usize {
    text.chars()
        .map(|character| character.width().unwrap_or(0))
        .sum()
}

fn pad(text: &str, width: usize) -> String {
    format!(
        "{text}{}",
        " ".repeat(width.saturating_sub(display_width(text)))
    )
}

fn truncate_js(text: &str, name_width: usize) -> String {
    if utf16_len(text) <= name_width {
        return pad(text, name_width);
    }
    let code_units: Vec<_> = text.encode_utf16().collect();
    let end = if name_width >= 3 {
        name_width - 3
    } else {
        code_units.len().saturating_sub(3 - name_width)
    };
    let prefix: Vec<_> = code_units.into_iter().take(end).collect();
    format!("{}...", String::from_utf16_lossy(&prefix))
}

fn time_ago(value: &str, now: SystemTime) -> String {
    let updated = DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|date| date.to_utc())
        .or_else(|| {
            NaiveDate::parse_from_str(value, "%Y-%m-%d")
                .ok()
                .and_then(|date| date.and_hms_opt(0, 0, 0))
                .map(|date| DateTime::<Utc>::from_naive_utc_and_offset(date, Utc))
        });
    let Some(updated) = updated else {
        return "NaN days ago".to_owned();
    };
    let now: DateTime<Utc> = now.into();
    let diff = now.signed_duration_since(updated);
    let minutes = diff.num_milliseconds().div_euclid(60_000);
    if minutes < 1 {
        return "just now".to_owned();
    }
    if minutes < 60 {
        return format!("{minutes} minutes ago");
    }
    let hours = minutes.div_euclid(60);
    if hours < 24 {
        return format!("{hours} hour{} ago", if hours == 1 { "" } else { "s" });
    }
    let days = hours.div_euclid(24);
    format!("{days} day{} ago", if days == 1 { "" } else { "s" })
}

fn terminal_color(color: &str) -> Option<String> {
    let hex = color.strip_prefix('#')?;
    let rgb = match hex.len() {
        6 => {
            let red = u8::from_str_radix(hex.get(0..2)?, 16).ok()?;
            let green = u8::from_str_radix(hex.get(2..4)?, 16).ok()?;
            let blue = u8::from_str_radix(hex.get(4..6)?, 16).ok()?;
            (red, green, blue)
        }
        3 => {
            let red = u8::from_str_radix(hex.get(0..1)?, 16).ok()? * 17;
            let green = u8::from_str_radix(hex.get(1..2)?, 16).ok()? * 17;
            let blue = u8::from_str_radix(hex.get(2..3)?, 16).ok()? * 17;
            (red, green, blue)
        }
        _ => return None,
    };
    Some(format!("\x1b[38;2;{};{};{}m", rgb.0, rgb.1, rgb.2))
}

pub fn render_text(teams: &[teams::Team], now: SystemTime, columns: usize, color: bool) -> String {
    if teams.is_empty() {
        return "No teams found.\n".to_owned();
    }
    let id_width = teams
        .iter()
        .map(|team| utf16_len(team.id.inner()))
        .max()
        .unwrap_or(0)
        .max(2);
    let key_width = teams
        .iter()
        .map(|team| utf16_len(&team.key))
        .max()
        .unwrap_or(0)
        .max(3);
    let updated: Vec<_> = teams
        .iter()
        .map(|team| time_ago(&team.updated_at.0, now))
        .collect();
    let updated_width = updated
        .iter()
        .map(|value| utf16_len(value))
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
    let name_width = available_width.min(max_name_width);
    let header = [
        pad("KEY", key_width),
        pad("NAME", name_width),
        pad("CYCLES", cycles_width),
        pad("UPDATED", updated_width),
        pad("ID", id_width),
    ];
    let mut output = if color {
        let mut line = String::new();
        for (index, cell) in header.iter().enumerate() {
            if index > 0 {
                line.push(' ');
            }
            line.push_str("\x1b[4m");
            line.push_str(cell);
            line.push_str(if index + 1 == header.len() {
                "\x1b[0m"
            } else {
                "\x1b[24m"
            });
        }
        line.push('\n');
        line
    } else {
        format!("{}\n", header.join(" "))
    };
    for (team, updated) in teams.iter().zip(updated) {
        let cycles = if team.cycles_enabled { "Yes" } else { "No" };
        let key = pad(&team.key, key_width);
        let name = truncate_js(&team.name, name_width);
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

const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn spinner_frame(index: usize) -> String {
    format!(
        "\r\x1b[K{}\x1b[0m ",
        SPINNER_FRAMES
            .get(index % SPINNER_FRAMES.len())
            .copied()
            .unwrap_or("⠋")
    )
}

pub const SPINNER_CLEAR: &[u8] = b"\r\x1b[K";

pub fn stdout_columns(is_terminal: bool) -> usize {
    if !is_terminal {
        return 120;
    }
    if let Some((terminal_size::Width(width), _)) =
        terminal_size::terminal_size_of(std::io::stdout())
    {
        return usize::from(width);
    }
    0
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
