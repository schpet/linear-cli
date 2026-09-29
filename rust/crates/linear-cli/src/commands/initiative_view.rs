//! Initiative detail resolution and display.
use chrono::{DateTime, Local, Utc};
use cynic::QueryBuilder;
use serde::Serialize;

use crate::commands::relative_time::format_relative_time;
use crate::commands::table::terminal_color;
use crate::config::NoColor;
use crate::error::{AppError, AppErrorKind};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::initiative_view::{
    DetailVariables, GetInitiativeByNameForView, GetInitiativeBySlugForView, GetInitiativeDetails,
    InitiativeDetails, NameVariables, ResolveInitiativeBySlug, SlugVariables, UrlSlugVariables,
};
use crate::graphql::operations::initiatives::{InitiativeStatus, InitiativeUpdateHealthType};
use crate::graphql::operations::projects::ProjectStatusType;
use crate::graphql::transport::GraphQlTransport;
use crate::platform::markdown_terminal::{self, HostSource, RenderOptions};
use crate::refs::{LinearUrlKind, LinearUrlRef, WorkspaceScope, expect_url_kind, is_linear_uuid};

pub const RESOLVE_CONTEXT: &str = "Failed to resolve initiative";
pub const FETCH_CONTEXT: &str = "Failed to fetch initiative details";
pub const OPEN_CONTEXT: &str = "Failed to open initiative";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Reference {
    Id(String),
    UrlSlug(String),
    NameOrSlug(String),
}

pub fn prepare_reference(input: &str, scope: &WorkspaceScope<'_>) -> Result<Reference, AppError> {
    match expect_url_kind(
        input,
        LinearUrlKind::Initiative,
        "an initiative URL, UUID, slug ID, or exact name",
        scope,
    )? {
        Some(LinearUrlRef::Initiative { slug_id, .. }) => Ok(Reference::UrlSlug(slug_id)),
        Some(_) => Err(AppError::new(
            AppErrorKind::Invariant,
            "initiative URL kind mismatch",
        )),
        None if is_linear_uuid(input) => Ok(Reference::Id(input.to_owned())),
        None => Ok(Reference::NameOrSlug(input.to_owned())),
    }
}

async fn resolve_text(
    transport: &GraphQlTransport,
    text: &str,
    original: &str,
) -> Result<String, AppError> {
    let request =
        GraphQlRequest::with_variables(GetInitiativeBySlugForView::build(SlugVariables {
            slug_id: text.to_owned(),
        }));
    let slug: GetInitiativeBySlugForView = transport
        .execute(&request)
        .await
        .map_err(AppError::from)
        .map_err(|e| e.with_context(RESOLVE_CONTEXT))?;
    if let Some(node) = slug.initiatives.nodes.first() {
        return Ok(node.id.inner().to_owned());
    }
    let request =
        GraphQlRequest::with_variables(GetInitiativeByNameForView::build(NameVariables {
            name: text.to_owned(),
        }));
    let name: GetInitiativeByNameForView = transport
        .execute(&request)
        .await
        .map_err(AppError::from)
        .map_err(|e| e.with_context(RESOLVE_CONTEXT))?;
    name.initiatives
        .nodes
        .first()
        .map(|node| node.id.inner().to_owned())
        .ok_or_else(|| AppError::not_found("Initiative", original).with_context(RESOLVE_CONTEXT))
}

pub async fn resolve_reference(
    transport: &GraphQlTransport,
    reference: &Reference,
    original: &str,
) -> Result<String, AppError> {
    match reference {
        Reference::Id(id) => Ok(id.clone()),
        Reference::NameOrSlug(text) => resolve_text(transport, text, original).await,
        Reference::UrlSlug(slug_id) => {
            let request =
                GraphQlRequest::with_variables(ResolveInitiativeBySlug::build(UrlSlugVariables {
                    slug_id: slug_id.clone(),
                    include_archived: Some(false),
                }));
            let result: ResolveInitiativeBySlug = transport
                .execute(&request)
                .await
                .map_err(AppError::from)
                .map_err(|e| e.with_context(RESOLVE_CONTEXT))?;
            let id = result
                .initiatives
                .nodes
                .first()
                .map(|node| node.id.inner().to_owned())
                .ok_or_else(|| {
                    AppError::not_found("Initiative", original).with_context(RESOLVE_CONTEXT)
                })?;
            if is_linear_uuid(&id) {
                Ok(id)
            } else {
                resolve_text(transport, &id, original).await
            }
        }
    }
}

pub async fn fetch_details(
    transport: &GraphQlTransport,
    id: String,
    original: &str,
) -> Result<InitiativeDetails, AppError> {
    let request =
        GraphQlRequest::with_variables(GetInitiativeDetails::build(DetailVariables { id }));
    let result: GetInitiativeDetails = transport
        .execute(&request)
        .await
        .map_err(AppError::from)
        .map_err(|e| e.with_context(FETCH_CONTEXT))?;
    let detail = result
        .initiative
        .ok_or_else(|| AppError::not_found("Initiative", original).with_context(FETCH_CONTEXT))?;
    verify_detail(&detail).map_err(|e| e.with_context(FETCH_CONTEXT))?;
    Ok(detail)
}

fn verify_detail(detail: &InitiativeDetails) -> Result<(), AppError> {
    if let InitiativeStatus::Unknown(value) = &detail.status {
        return Err(AppError::new(
            AppErrorKind::Invariant,
            format!("Linear returned an unknown initiative status: {value}"),
        ));
    }
    if let Some(InitiativeUpdateHealthType::Unknown(value)) = &detail.health {
        return Err(AppError::new(
            AppErrorKind::Invariant,
            format!("Linear returned an unknown initiative health: {value}"),
        ));
    }
    for project in &detail.projects.nodes {
        if let ProjectStatusType::Unknown(value) = &project.status.status_type {
            return Err(AppError::new(
                AppErrorKind::Invariant,
                format!("Linear returned an unknown project status type: {value}"),
            ));
        }
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonDetail<'a> {
    id: &'a cynic::Id,
    slug_id: &'a str,
    name: &'a str,
    description: Option<&'a str>,
    status: &'a str,
    target_date: Option<&'a str>,
    health: Option<&'a str>,
    color: Option<&'a str>,
    icon: Option<&'a str>,
    url: &'a str,
    archived_at: Option<&'a str>,
    created_at: &'a str,
    updated_at: &'a str,
    owner: Option<JsonOwner<'a>>,
    projects: JsonProjects<'a>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonOwner<'a> {
    id: &'a cynic::Id,
    name: &'a str,
    display_name: &'a str,
}
#[derive(Serialize)]
struct JsonProjects<'a> {
    nodes: Vec<JsonProject<'a>>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonProject<'a> {
    id: &'a cynic::Id,
    slug_id: &'a str,
    name: &'a str,
    status: JsonProjectStatus<'a>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonProjectStatus<'a> {
    name: &'a str,
    #[serde(rename = "type")]
    status_type: &'a str,
}

pub fn render_json(detail: &InitiativeDetails) -> Result<Vec<u8>, AppError> {
    let projection = JsonDetail {
        id: &detail.id,
        slug_id: &detail.slug_id,
        name: &detail.name,
        description: detail.description.as_deref(),
        status: detail.status.as_str(),
        target_date: detail.target_date.as_ref().map(|d| d.0.as_str()),
        health: detail
            .health
            .as_ref()
            .map(InitiativeUpdateHealthType::as_str),
        color: detail.color.as_deref(),
        icon: detail.icon.as_deref(),
        url: &detail.url,
        archived_at: detail.archived_at.as_ref().map(|d| d.0.as_str()),
        created_at: &detail.created_at.0,
        updated_at: &detail.updated_at.0,
        owner: detail.owner.as_ref().map(|o| JsonOwner {
            id: &o.id,
            name: &o.name,
            display_name: &o.display_name,
        }),
        projects: JsonProjects {
            nodes: detail
                .projects
                .nodes
                .iter()
                .map(|p| JsonProject {
                    id: &p.id,
                    slug_id: &p.slug_id,
                    name: &p.name,
                    status: JsonProjectStatus {
                        name: &p.status.name,
                        status_type: p.status.status_type.as_str(),
                    },
                })
                .collect(),
        },
    };
    let mut bytes = serde_json::to_vec_pretty(&projection).map_err(|e| {
        AppError::new(AppErrorKind::Invariant, "could not serialize initiative").with_source(e)
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn project_rank(status: &ProjectStatusType) -> u8 {
    match status {
        ProjectStatusType::Started => 0,
        ProjectStatusType::Planned => 1,
        ProjectStatusType::Backlog => 2,
        ProjectStatusType::Paused => 3,
        ProjectStatusType::Completed => 4,
        ProjectStatusType::Canceled => 5,
        ProjectStatusType::Unknown(_) => 6,
    }
}

pub fn markdown(detail: &InitiativeDetails, now: DateTime<Utc>, terminal: bool) -> String {
    let icon = detail
        .icon
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| format!("{s} "))
        .unwrap_or_default();
    let mut lines = vec![
        format!("# {icon}{}", detail.name),
        String::new(),
        format!("**Slug:** {}", detail.slug_id),
        format!("**URL:** {}", detail.url),
    ];
    if !terminal {
        lines.push(format!("**Status:** {}", detail.status.as_str()));
    }
    if let Some(health) = &detail.health {
        lines.push(format!("**Health:** {}", health.as_str()));
    }
    if let Some(owner) = &detail.owner {
        let display = if owner.display_name.is_empty() {
            &owner.name
        } else {
            &owner.display_name
        };
        lines.push(format!("**Owner:** {display}"));
    }
    if let Some(date) = &detail.target_date {
        lines.push(format!("**Target Date:** {}", date.0));
    }
    if let Some(date) = &detail.archived_at {
        lines.push(format!(
            "**Archived:** {}",
            format_relative_time(&date.0, now, &Local)
        ));
    }
    lines.push(String::new());
    lines.push(format!(
        "**Created:** {}",
        format_relative_time(&detail.created_at.0, now, &Local)
    ));
    lines.push(format!(
        "**Updated:** {}",
        format_relative_time(&detail.updated_at.0, now, &Local)
    ));
    if let Some(description) = detail.description.as_deref().filter(|s| !s.is_empty()) {
        lines.extend([
            String::new(),
            "## Description".to_owned(),
            String::new(),
            description.to_owned(),
        ]);
    }
    lines.push(String::new());
    if detail.projects.nodes.is_empty() {
        lines.extend([
            "## Projects".to_owned(),
            String::new(),
            "*No projects linked to this initiative.*".to_owned(),
        ]);
    } else {
        lines.push(format!("## Projects ({})", detail.projects.nodes.len()));
        lines.push(String::new());
        let mut projects = detail.projects.nodes.iter().enumerate().collect::<Vec<_>>();
        projects
            .sort_by_key(|(index, project)| (project_rank(&project.status.status_type), *index));
        for (_, project) in projects {
            let status = if project.status.name.is_empty() {
                "Unknown"
            } else {
                &project.status.name
            };
            lines.push(format!("- **{}** ({status})", project.name));
        }
    }
    lines.join("\n")
}

pub fn render_text(
    detail: &InitiativeDetails,
    terminal: bool,
    columns: std::num::NonZeroU16,
    no_color: NoColor,
) -> Result<Vec<u8>, AppError> {
    let now = Utc::now();
    let body = markdown(detail, now, terminal);
    if !terminal {
        return Ok(format!("{body}\n").into_bytes());
    }
    let options = RenderOptions::for_terminal(columns, no_color, true, None, HostSource::System);
    let rendered = markdown_terminal::render(&body, &options)?;
    let line = format!("**Status:** {}", detail.status.as_str());
    let colored = if no_color == NoColor::Nonempty {
        line
    } else {
        let hex = match detail.status {
            InitiativeStatus::Active => "#27AE60",
            InitiativeStatus::Planned => "#5E6AD2",
            InitiativeStatus::Completed | InitiativeStatus::Proposed => "#6B6F76",
            InitiativeStatus::Canceled => "#EB5757",
            InitiativeStatus::Unknown(_) => "#6B6F76",
        };
        let sgr = terminal_color(hex).ok_or_else(|| {
            AppError::new(
                AppErrorKind::Invariant,
                "invalid built-in initiative status color",
            )
        })?;
        format!("{sgr}{line}\x1b[0m")
    };
    Ok(format!("{colored}\n{rendered}\n").into_bytes())
}

pub fn opening(detail: &InitiativeDetails, app: bool) -> Vec<u8> {
    let target = if app { "Linear.app" } else { "web browser" };
    format!("Opening {} in {target}\n", detail.url).into_bytes()
}
