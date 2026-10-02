//! `project-update list`: the first page of status updates as a table or JSON.

use std::future::Future;
use std::num::NonZeroU32;
use std::time::SystemTime;

use cynic::QueryBuilder;
use serde::Serialize;

use crate::commands::display::{display_width, pad, truncate_text};
use crate::commands::relative_time::format_relative_time;
use crate::commands::style;
use crate::commands::table::underlined_header;
use crate::error::{AppError, AppErrorKind};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::project_updates::{
    ListProjectUpdates, ListProjectUpdatesVariables, UpdateNode, UpdateProject,
};
use crate::graphql::transport::GraphQlTransport;

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
    id.chars().take(8).collect()
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
            display_width(node.health.as_ref().map_or("-", |health| {
                let value = health.as_str();
                if value.is_empty() { "-" } else { value }
            }))
        })
        .max()
        .unwrap_or(0)
        .max(6);
    let date_width = updates
        .iter()
        .map(|node| {
            display_width(&format_relative_time(
                &node.created_at.0,
                now.into(),
                &chrono::Local,
            ))
        })
        .max()
        .unwrap_or(0)
        .max(4);
    let author_width = updates
        .iter()
        .map(|node| display_width(author(node)))
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
        output.push_str(&pad(
            &format_relative_time(&node.created_at.0, now.into(), &chrono::Local),
            date_width,
        ));
        output.push(' ');
        output.push_str(&pad(author(node), author_width));
        output.push('\n');
        if !node.body.is_empty() {
            let preview = node.body.replace('\n', " ");
            let preview = truncate_text(preview.trim(), available_width);
            output.push_str(&style::gray(&format!("   {preview}"), color));
            output.push('\n');
        }
    }
    output
}
