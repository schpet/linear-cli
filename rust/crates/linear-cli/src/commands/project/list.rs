//! `project list`: every project of a team (or the workspace), as a table or
//! JSON, or the projects page opened in Linear.

use std::time::SystemTime;

use cynic::QueryBuilder;
use serde::Serialize;

use crate::cli::project::ProjectList;
use crate::commands::display::{display_width, fit, flexible_width, pad};
use crate::commands::relative_time::format_relative_time;
use crate::commands::table::{self, underlined_header};
use crate::commands::team_key::configured_team_key;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::number::Float;
use crate::graphql::operations::projects::{
    GetProjects, GetProjectsVariables, Project, ProjectFilter, ProjectStatusFilter,
    ProjectStatusType, TeamCollectionFilter,
};
use crate::graphql::operations::teams::{PageInfo, StringComparator, TeamFilter};
use crate::graphql::pagination::{self, EmptyCursorPolicy, Page};
use crate::graphql::scalars::{DateTime, TimelessDate};
use crate::graphql::transport::GraphQlTransport;
use crate::platform::{collation, style};
use crate::refs::{prepare_team_lookup, resolve_team_with_transport};

pub fn run(ctx: &Ctx, args: &ProjectList) -> Result<()> {
    list(ctx, args).context("Failed to list projects")
}

fn list(ctx: &Ctx, args: &ProjectList) -> Result<()> {
    let team_lookup = args
        .team
        .as_deref()
        .map(|team| prepare_team_lookup(team, &ctx.scope()?))
        .transpose()?;
    let configured = if args.all_teams {
        None
    } else {
        configured_team_key(ctx.options())
    };
    if (args.web || args.app) && team_lookup.is_none() {
        return ctx.open_in_linear(&projects_path(configured.as_deref()), args.app);
    }
    let client = ctx.client()?;
    let team_key = async {
        match &team_lookup {
            Some(lookup) => {
                Ok::<_, Error>(Some(resolve_team_with_transport(lookup, client).await?.key))
            }
            None => Ok(configured.clone()),
        }
    };
    if args.web || args.app {
        let team_key = ctx.spin(true, team_key)?;
        return ctx.open_in_linear(&projects_path(team_key.as_deref()), args.app);
    }
    let status = args.status.as_deref();
    let (projects, page_info) = ctx.spin(!args.json, async {
        let team_key = team_key.await?;
        fetch(client, filter(team_key.as_deref(), status)).await
    })?;
    if args.json {
        ctx.print(render_json(&projects, &page_info))
    } else {
        let columns = table::stdout_columns(ctx.stdout_tty());
        ctx.print(render_text(
            &projects,
            SystemTime::now(),
            columns,
            ctx.color(),
        ))
    }
}

/// The projects page of a team, or of the whole workspace.
fn projects_path(team_key: Option<&str>) -> String {
    team_key.map_or_else(
        || "projects/all".to_owned(),
        |team| format!("team/{team}/projects/all"),
    )
}

pub(super) fn filter(team_key: Option<&str>, status: Option<&str>) -> Option<ProjectFilter> {
    let accessible_teams = team_key.map(|key| TeamCollectionFilter {
        some: Some(TeamFilter {
            key: Some(StringComparator {
                eq: Some(key.to_owned()),
                ..Default::default()
            }),
            ..Default::default()
        }),
    });
    let status = status.map(|name| ProjectStatusFilter {
        name: Some(StringComparator {
            eq: Some(name.to_owned()),
            ..Default::default()
        }),
    });
    if accessible_teams.is_none() && status.is_none() {
        None
    } else {
        Some(ProjectFilter {
            accessible_teams,
            status,
        })
    }
}

/// Every matching project, in Linear's manual order.
async fn fetch(
    client: &GraphQlTransport,
    filter: Option<ProjectFilter>,
) -> Result<(Vec<Project>, PageInfo)> {
    let pages = pagination::paginate_with_policy(EmptyCursorPolicy::Allow, |after| {
        let request = GraphQlRequest::with_variables(GetProjects::build(GetProjectsVariables {
            filter: filter.clone(),
            first: Some(100),
            after,
        }));
        async move {
            let data: GetProjects = client.execute(&request).await?;
            Ok(Page {
                nodes: data.projects.nodes,
                page_info: data.projects.page_info.into(),
            })
        }
    })
    .await
    .map_err(|error| super::pagination_error("projects", error))?;
    let mut projects = pages.nodes;
    projects.sort_by(|left, right| {
        left.sort_order
            .get()
            .total_cmp(&right.sort_order.get())
            .then_with(|| collation::compare(&left.name, &right.name))
            .then_with(|| collation::compare(left.id.inner(), right.id.inner()))
    });
    let page_info = PageInfo {
        has_next_page: pages.page_info.has_next_page,
        end_cursor: pages.page_info.end_cursor,
    };
    Ok((projects, page_info))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonConnection<'a> {
    nodes: Vec<JsonProject<'a>>,
    page_info: &'a PageInfo,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonProject<'a> {
    id: &'a str,
    name: &'a str,
    description: &'a str,
    slug_id: &'a str,
    icon: Option<&'a str>,
    color: &'a str,
    sort_order: &'a Float,
    status: &'a crate::graphql::operations::projects::ProjectStatus,
    lead: Option<&'a crate::graphql::operations::projects::ProjectLead>,
    priority: i32,
    health: Option<&'a crate::graphql::operations::projects::ProjectUpdateHealthType>,
    start_date: Option<&'a TimelessDate>,
    target_date: Option<&'a TimelessDate>,
    started_at: Option<&'a DateTime>,
    completed_at: Option<&'a DateTime>,
    canceled_at: Option<&'a DateTime>,
    created_at: &'a DateTime,
    updated_at: &'a DateTime,
    url: &'a str,
    teams: &'a crate::graphql::operations::projects::ProjectTeams,
}

fn render_json(projects: &[Project], page_info: &PageInfo) -> Vec<u8> {
    let nodes = projects
        .iter()
        .map(|project| JsonProject {
            id: project.id.inner(),
            name: &project.name,
            description: &project.description,
            slug_id: &project.slug_id,
            icon: project.icon.as_deref(),
            color: &project.color,
            sort_order: &project.sort_order,
            status: &project.status,
            lead: project.lead.as_ref(),
            priority: project.priority,
            health: project.health.as_ref(),
            start_date: project.start_date.as_ref(),
            target_date: project.target_date.as_ref(),
            started_at: project.started_at.as_ref(),
            completed_at: project.completed_at.as_ref(),
            canceled_at: project.canceled_at.as_ref(),
            created_at: &project.created_at,
            updated_at: &project.updated_at,
            url: &project.url,
            teams: &project.teams,
        })
        .collect();
    let mut bytes = serde_json::to_vec_pretty(&JsonConnection { nodes, page_info })
        .expect("project JSON always serializes");
    bytes.push(b'\n');
    bytes
}

pub(super) fn priority_label(priority: i32) -> String {
    match priority {
        0 => "None".to_owned(),
        1 => "Urgent".to_owned(),
        2 => "High".to_owned(),
        3 => "Medium".to_owned(),
        4 => "Low".to_owned(),
        other => other.to_string(),
    }
}

/// The date that matters for the project's status, like "Started 3 days ago".
fn display_date(project: &Project, now: SystemTime) -> String {
    let relative = |label: &str, date: &DateTime| {
        format!(
            "{label} {}",
            format_relative_time(&date.0, now.into(), &chrono::Local)
        )
    };
    let planned = |label: &str, date: &TimelessDate| format!("{label}: {}", date.0);
    let updated = || relative("Updated", &project.updated_at);
    let created = || relative("Created", &project.created_at);
    match &project.status.status_type {
        ProjectStatusType::Started => match (&project.started_at, &project.start_date) {
            (Some(started), _) => relative("Started", started),
            (None, Some(start)) => planned("Start", start),
            (None, None) => created(),
        },
        ProjectStatusType::Completed => project
            .completed_at
            .as_ref()
            .map_or_else(updated, |date| relative("Done", date)),
        ProjectStatusType::Canceled => project
            .canceled_at
            .as_ref()
            .map_or_else(updated, |date| relative("Canceled", date)),
        ProjectStatusType::Planned => match (&project.start_date, &project.target_date) {
            (Some(start), _) => planned("Start", start),
            (None, Some(target)) => planned("Target", target),
            (None, None) => created(),
        },
        // A status type newer than this program shows like a paused one.
        ProjectStatusType::Backlog | ProjectStatusType::Paused | ProjectStatusType::Unknown(_) => {
            updated()
        }
    }
}

fn render_text(projects: &[Project], now: SystemTime, columns: usize, color: bool) -> String {
    if projects.is_empty() {
        return "No projects found.\n".to_owned();
    }
    let dates: Vec<_> = projects
        .iter()
        .map(|project| display_date(project, now))
        .collect();
    let slug_width = projects
        .iter()
        .map(|project| display_width(&project.slug_id))
        .max()
        .unwrap_or(0)
        .max(4);
    let status_width = projects
        .iter()
        .map(|project| display_width(&project.status.name))
        .max()
        .unwrap_or(0)
        .max(6);
    let priority_width = projects
        .iter()
        .map(|project| display_width(&priority_label(project.priority)))
        .max()
        .unwrap_or(0)
        .max(8);
    let health_width = projects
        .iter()
        .map(|project| display_width(health(project)))
        .max()
        .unwrap_or(0)
        .max(6);
    let lead_width = projects
        .iter()
        .map(|project| display_width(lead(project)))
        .max()
        .unwrap_or(0)
        .max(4);
    let team_width = projects
        .iter()
        .map(|project| display_width(&teams(project)))
        .max()
        .unwrap_or(0)
        .max(5);
    let date_width = dates
        .iter()
        .map(|date| display_width(date))
        .max()
        .unwrap_or(0)
        .max(4);
    let fixed = slug_width
        + status_width
        + priority_width
        + health_width
        + lead_width
        + team_width
        + date_width
        + 4;
    let max_name_width = projects
        .iter()
        .map(|project| display_width(&project.name))
        .max()
        .unwrap_or(0);
    let name_width = flexible_width(
        max_name_width,
        columns.saturating_sub(1).saturating_sub(fixed),
    );
    let headers = [
        pad("SLUG", slug_width),
        pad("NAME", name_width),
        pad("STATUS", status_width),
        pad("PRIORITY", priority_width),
        pad("HEALTH", health_width),
        pad("LEAD", lead_width),
        pad("TEAMS", team_width),
        pad("DATE", date_width),
    ];
    let mut output = underlined_header(&headers, color);
    for (project, date) in projects.iter().zip(dates) {
        let slug = pad(&project.slug_id, slug_width);
        let name = fit(&project.name, name_width);
        let status = pad(&project.status.name, status_width);
        let priority = pad(&priority_label(project.priority), priority_width);
        let health = pad(health(project), health_width);
        let lead = pad(lead(project), lead_width);
        let teams = pad(&teams(project), team_width);
        let date = pad(&date, date_width);
        let status = hex_color(&status, &project.status.color, color);
        let date = style::gray(&date, color);
        output.push_str(&format!(
            "{slug} {name} {status} {priority} {health} {lead} {teams} {date}\n"
        ));
    }
    output
}

fn health(project: &Project) -> &str {
    project.health.as_ref().map_or("Unknown", |health| {
        let value = health.as_str();
        if value.is_empty() { "Unknown" } else { value }
    })
}

fn lead(project: &Project) -> &str {
    project.lead.as_ref().map_or("-", |lead| {
        if lead.initials.is_empty() {
            "-"
        } else {
            &lead.initials
        }
    })
}

fn teams(project: &Project) -> String {
    let teams = project
        .teams
        .nodes
        .iter()
        .map(|team| team.key.as_str())
        .collect::<Vec<_>>()
        .join(",");
    if teams.is_empty() {
        "-".to_owned()
    } else {
        teams
    }
}

/// Paints `text` in a Linear color like `#5e6ad2`; other values leave it plain.
fn hex_color(text: &str, hex: &str, enabled: bool) -> String {
    let rgb = hex.strip_prefix('#').and_then(|digits| {
        let channel = |range| u8::from_str_radix(digits.get(range)?, 16).ok();
        match digits.len() {
            6 => Some((channel(0..2)?, channel(2..4)?, channel(4..6)?)),
            3 => Some((
                channel(0..1)? * 17,
                channel(1..2)? * 17,
                channel(2..3)? * 17,
            )),
            _ => None,
        }
    });
    match rgb {
        Some((red, green, blue)) if enabled => console::Style::new()
            .true_color(red, green, blue)
            .force_styling(true)
            .apply_to(text)
            .to_string(),
        Some(_) | None => text.to_owned(),
    }
}
