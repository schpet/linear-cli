//! `initiative-update list`: a typed first page with source-shaped output.

use chrono::{DateTime, Local, Utc};
use cynic::QueryBuilder;
use serde::Serialize;

use crate::commands::display::{pad, truncate_text};
use crate::commands::relative_time::format_relative_time;
use crate::commands::table::{underlined_header, utf16_len};
use crate::error::{AppError, AppErrorKind};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::initiative_updates::{
    ListInitiativeUpdates, ListInitiativeUpdatesVariables, UpdateInitiative, UpdateNode,
};
use crate::graphql::operations::initiatives::InitiativeUpdateHealthType;
use crate::graphql::transport::GraphQlTransport;
use crate::text::js_space;

pub const CONTEXT: &str = "Failed to fetch initiative updates";

pub fn graphql_int(value: f64) -> Result<i32, AppError> {
    crate::commands::project_update_list::graphql_int(value).map_err(|mut error| {
        error.context = Some(CONTEXT.to_owned());
        error
    })
}

pub fn request(id: &str, first: i32) -> GraphQlRequest<ListInitiativeUpdatesVariables> {
    GraphQlRequest::with_variables(ListInitiativeUpdates::build(
        ListInitiativeUpdatesVariables {
            id: id.to_owned(),
            first: Some(first),
        },
    ))
}

pub async fn run(
    transport: &GraphQlTransport,
    original: &str,
    id: &str,
    first: i32,
    json: bool,
    columns: usize,
    color: bool,
) -> Result<Vec<u8>, AppError> {
    let result: ListInitiativeUpdates = transport
        .execute(&request(id, first))
        .await
        .map_err(AppError::from)
        .map_err(|error| error.with_context(CONTEXT))?;
    let initiative = result
        .initiative
        .ok_or_else(|| AppError::not_found("Initiative", original).with_context(CONTEXT))?;
    for update in &initiative.initiative_updates.nodes {
        if let InitiativeUpdateHealthType::Unknown(value) = &update.health {
            return Err(AppError::new(
                AppErrorKind::Invariant,
                format!("Linear returned an unknown initiative update health: {value}"),
            )
            .with_context(CONTEXT));
        }
    }
    if json {
        render_json(&initiative)
    } else {
        Ok(render_text(&initiative, columns, color, Utc::now()).into_bytes())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonInitiative<'a> {
    name: &'a str,
    slug_id: &'a str,
    initiative_updates: JsonConnection<'a>,
}

#[derive(Serialize)]
struct JsonConnection<'a> {
    nodes: Vec<JsonUpdate<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonUpdate<'a> {
    id: &'a cynic::Id,
    body: &'a str,
    health: &'a str,
    url: &'a str,
    created_at: &'a crate::graphql::scalars::DateTime,
    user: Option<JsonUser<'a>>,
}

#[derive(Serialize)]
struct JsonUser<'a> {
    name: &'a str,
}

pub fn render_json(initiative: &UpdateInitiative) -> Result<Vec<u8>, AppError> {
    let value = JsonInitiative {
        name: &initiative.name,
        slug_id: &initiative.slug_id,
        initiative_updates: JsonConnection {
            nodes: initiative
                .initiative_updates
                .nodes
                .iter()
                .map(|node| JsonUpdate {
                    id: &node.id,
                    body: &node.body,
                    health: node.health.as_str(),
                    url: &node.url,
                    created_at: &node.created_at,
                    user: node.user.as_ref().map(|user| JsonUser { name: &user.name }),
                })
                .collect(),
        },
    };
    let mut output = serde_json::to_vec_pretty(&value).map_err(|error| {
        AppError::new(
            AppErrorKind::Invariant,
            "could not serialize initiative updates",
        )
        .with_source(error)
        .with_context(CONTEXT)
    })?;
    output.push(b'\n');
    Ok(output)
}

fn health(node: &UpdateNode) -> (&str, &str) {
    match &node.health {
        InitiativeUpdateHealthType::OnTrack => ("On Track", "\x1b[38;2;39;174;96m"),
        InitiativeUpdateHealthType::AtRisk => ("At Risk", "\x1b[38;2;242;153;74m"),
        InitiativeUpdateHealthType::OffTrack => ("Off Track", "\x1b[38;2;235;87;87m"),
        InitiativeUpdateHealthType::Unknown(value) if value.is_empty() => {
            ("-", "\x1b[38;2;107;111;118m")
        }
        InitiativeUpdateHealthType::Unknown(value) => (value, "\x1b[38;2;107;111;118m"),
    }
}

fn author(node: &UpdateNode) -> &str {
    node.user
        .as_ref()
        .map(|user| user.name.as_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("-")
}

fn short_id(id: &str) -> String {
    String::from_utf16_lossy(&id.encode_utf16().take(8).collect::<Vec<_>>())
}

// Deno's console formatter consumes body placeholders before the trailing %c.
fn console_body(preview: &str, color: bool) -> String {
    let format = format!("{preview}%c");
    let mut chars = format.chars().peekable();
    let mut output = String::from("  ");
    let mut reset_available = true;
    if color {
        output.push_str("\x1b[38;2;128;128;128m");
    }
    while let Some(ch) = chars.next() {
        if ch != '%' {
            output.push(ch);
            continue;
        }
        match chars.peek().copied() {
            Some('%') => {
                chars.next();
                output.push('%');
            }
            Some('c') if reset_available => {
                chars.next();
                reset_available = false;
                if color {
                    output.push_str("\x1b[39m");
                }
            }
            Some('s') if reset_available => {
                chars.next();
                reset_available = false;
            }
            Some('d' | 'i' | 'f') if reset_available => {
                chars.next();
                reset_available = false;
                output.push_str("NaN");
            }
            Some('o' | 'O') if reset_available => {
                chars.next();
                reset_available = false;
                if color {
                    output.push_str("\x1b[32m");
                }
                output.push_str("\"\"");
                if color {
                    output.push_str("\x1b[39m");
                }
            }
            _ => output.push('%'),
        }
    }
    if color {
        output.push_str("\x1b[0m");
    }
    if reset_available {
        output.push(' ');
    }
    output
}

pub fn render_text(
    initiative: &UpdateInitiative,
    columns: usize,
    color: bool,
    now: DateTime<Utc>,
) -> String {
    let updates = &initiative.initiative_updates.nodes;
    if updates.is_empty() {
        return format!("No status updates found for: {}\n", initiative.name);
    }
    let health_width = updates
        .iter()
        .map(|node| utf16_len(health(node).0))
        .max()
        .unwrap_or(0)
        .max(6);
    let date_width = updates
        .iter()
        .map(|node| utf16_len(&format_relative_time(&node.created_at.0, now, &Local)))
        .max()
        .unwrap_or(0)
        .max(4);
    let author_width = updates
        .iter()
        .map(|node| utf16_len(author(node)))
        .max()
        .unwrap_or(0)
        .max(6);
    let available_width = columns
        .saturating_sub(1 + 8 + health_width + date_width + author_width + 4)
        .max(10);
    let mut output = format!("Status updates for: {}\n\n", initiative.name);
    output.push_str(&underlined_header(
        &[
            pad("ID", 8),
            pad("HEALTH", health_width),
            pad("DATE", date_width),
            pad("AUTHOR", author_width),
        ],
        color,
    ));
    for node in updates {
        let (health_name, health_color) = health(node);
        let date = format_relative_time(&node.created_at.0, now, &Local);
        output.push_str(&pad(&short_id(node.id.inner()), 8));
        output.push(' ');
        if color {
            output.push_str(health_color);
        }
        output.push_str(&pad(health_name, health_width));
        if color {
            output.push_str("\x1b[39m");
        }
        output.push(' ');
        if color {
            output.push_str("\x1b[38;2;128;128;128m");
        }
        output.push_str(&pad(&date, date_width));
        if color {
            output.push_str("\x1b[39m");
        }
        output.push(' ');
        output.push_str(&pad(author(node), author_width).replace("%%", "%"));
        if color {
            output.push_str("\x1b[0m");
        }
        output.push('\n');
        if !node.body.is_empty() {
            let preview = node.body.replace('\n', " ");
            let preview = truncate_text(preview.trim_matches(js_space), available_width);
            output.push_str(&console_body(&preview, color));
            output.push('\n');
        }
    }
    output
}
