//! `project-update list`: one typed page and source-shaped output.

use std::future::Future;
use std::num::NonZeroU32;
use std::time::SystemTime;

use cynic::QueryBuilder;
use serde::Serialize;

use crate::commands::display::{pad, truncate_text};
use crate::commands::table::{time_ago, underlined_header, utf16_len};
use crate::error::{AppError, AppErrorKind};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::project_updates::{
    ListProjectUpdates, ListProjectUpdatesVariables, UpdateNode, UpdateProject,
};
use crate::graphql::transport::GraphQlTransport;
use crate::text::js_space;

pub const CONTEXT: &str = "Failed to fetch project updates";

pub fn output_color(stdout_tty: bool, no_color: bool) -> bool {
    stdout_tty && !no_color
}

/// CLI page sizes are positive u32 values; GraphQL has a signed Int boundary.
/// Conversion fails before transport or project resolution; nothing truncates.
pub fn graphql_int(value: NonZeroU32) -> Result<i32, AppError> {
    i32::try_from(value.get()).map_err(|error| {
        AppError::new(
            AppErrorKind::Validation,
            "--limit must be at most 2147483647 for a GraphQL Int",
        )
        .with_source(error)
        .with_context(CONTEXT)
    })
}

#[derive(Clone, Copy)]
pub struct RenderOptions {
    pub json: bool,
    pub columns: usize,
    pub color: bool,
    pub now: SystemTime,
}

pub fn request(id: &str, first: i32) -> GraphQlRequest<ListProjectUpdatesVariables> {
    GraphQlRequest::with_variables(ListProjectUpdates::build(ListProjectUpdatesVariables {
        id: id.to_owned(),
        first: Some(first),
    }))
}

pub async fn run_with<F, Fut>(
    original: &str,
    id: &str,
    first: i32,
    fetch: F,
    options: RenderOptions,
) -> Result<Vec<u8>, AppError>
where
    F: FnOnce(GraphQlRequest<ListProjectUpdatesVariables>) -> Fut,
    Fut: Future<Output = Result<ListProjectUpdates, AppError>>,
{
    let project = fetch(request(id, first))
        .await
        .map_err(|error| error.with_context(CONTEXT))?
        .project
        .ok_or_else(|| AppError::not_found("Project", original).with_context(CONTEXT))?;
    if options.json {
        render_json(&project)
    } else {
        Ok(render_text(&project, options.columns, options.color, options.now).into_bytes())
    }
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
    run_with(
        original,
        id,
        first,
        |request| async move { transport.execute(&request).await.map_err(AppError::from) },
        RenderOptions {
            json,
            columns,
            color,
            now: SystemTime::now(),
        },
    )
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonProject<'a> {
    name: &'a str,
    slug_id: &'a str,
    project_updates: JsonConnection<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonConnection<'a> {
    nodes: Vec<JsonUpdate<'a>>,
    page_info: &'a crate::graphql::operations::teams::PageInfo,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonUpdate<'a> {
    id: &'a cynic::Id,
    body: &'a str,
    health: Option<&'a str>,
    url: &'a str,
    created_at: &'a crate::graphql::scalars::DateTime,
    user: Option<JsonUser<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonUser<'a> {
    name: &'a str,
    display_name: &'a str,
}

pub fn render_json(project: &UpdateProject) -> Result<Vec<u8>, AppError> {
    let nodes = project
        .project_updates
        .nodes
        .iter()
        .map(|node| JsonUpdate {
            id: &node.id,
            body: &node.body,
            health: node.health.as_ref().map(|health| health.as_str()),
            url: &node.url,
            created_at: &node.created_at,
            user: node.user.as_ref().map(|user| JsonUser {
                name: &user.name,
                display_name: &user.display_name,
            }),
        })
        .collect();
    let value = JsonProject {
        name: &project.name,
        slug_id: &project.slug_id,
        project_updates: JsonConnection {
            nodes,
            page_info: &project.project_updates.page_info,
        },
    };
    let mut output = serde_json::to_vec_pretty(&value).map_err(|error| {
        AppError::new(
            AppErrorKind::Invariant,
            "could not serialize project updates",
        )
        .with_source(error)
        .with_context(CONTEXT)
    })?;
    output.push(b'\n');
    Ok(output)
}

fn author(node: &UpdateNode) -> &str {
    node.user
        .as_ref()
        .map(|user| {
            if user.display_name.is_empty() {
                &user.name
            } else {
                &user.display_name
            }
        })
        .filter(|name| !name.is_empty())
        .map_or("-", String::as_str)
}

fn short_id(id: &str) -> String {
    String::from_utf16_lossy(&id.encode_utf16().take(8).collect::<Vec<_>>())
}

/// Deno's console formatter sees the body inside `%c   {body}%c` with two
/// style arguments. The opening `%c` consumes gray; body placeholders can
/// consume the empty reset argument before the closing `%c` sees it.
fn console_body(preview: &str, color: bool) -> String {
    let format = format!("   {preview}%c");
    let mut chars = format.chars().peekable();
    let mut output = String::new();
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
    project: &UpdateProject,
    columns: usize,
    color: bool,
    now: SystemTime,
) -> String {
    let updates = &project.project_updates.nodes;
    if updates.is_empty() {
        return format!("No status updates found for project: {}\n", project.name);
    }
    let health_width = updates
        .iter()
        .map(|node| {
            utf16_len(node.health.as_ref().map_or("-", |health| {
                let value = health.as_str();
                if value.is_empty() { "-" } else { value }
            }))
        })
        .max()
        .unwrap_or(0)
        .max(6);
    let date_width = updates
        .iter()
        .map(|node| utf16_len(&time_ago(&node.created_at.0, now)))
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
    let mut output = format!("Status updates for: {}\n\n", project.name);
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
        let health = node.health.as_ref().map_or("-", |health| {
            let value = health.as_str();
            if value.is_empty() { "-" } else { value }
        });
        let health_cell = pad(health, health_width);
        let color_code = match health {
            "onTrack" => Some("\x1b[32m"),
            "atRisk" => Some("\x1b[33m"),
            "offTrack" => Some("\x1b[31m"),
            _ => None,
        };
        output.push_str(&pad(&short_id(node.id.inner()), 8));
        output.push(' ');
        if color && let Some(code) = color_code {
            output.push_str(code);
        }
        output.push_str(&health_cell);
        if color && color_code.is_some() {
            output.push_str("\x1b[39m");
        }
        output.push(' ');
        output.push_str(&pad(&time_ago(&node.created_at.0, now), date_width));
        output.push(' ');
        let author_cell = pad(author(node), author_width);
        if color_code.is_some() {
            output.push_str(&author_cell.replace("%%", "%"));
        } else {
            output.push_str(&author_cell);
        }
        if color && color_code.is_some() {
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
