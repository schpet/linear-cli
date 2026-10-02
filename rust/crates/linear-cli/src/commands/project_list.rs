//! `project list`: typed project pagination, stable display ordering and the
//! selected GraphQL connection in JSON or terminal form.

use std::future::Future;
use std::time::SystemTime;

use cynic::QueryBuilder;
use serde::Serialize;

use crate::commands::display::{display_width, pad, truncate_js};
use crate::commands::table::{time_ago, underlined_header, utf16_len};
use crate::error::{AppError, AppErrorKind};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::number::Float;
use crate::graphql::operations::projects::{
    GetProjects, GetProjectsVariables, GetViewer, Project, ProjectFilter, ProjectStatusFilter,
    ProjectStatusType, TeamCollectionFilter,
};
use crate::graphql::operations::teams::{PageInfo, StringComparator, TeamFilter};
use crate::graphql::pagination::{self, EmptyCursorPolicy, Page, PaginationError};
use crate::graphql::scalars::{DateTime, TimelessDate};
use crate::graphql::transport::GraphQlTransport;
use crate::platform::{collation, opener};

pub const FETCH_CONTEXT: &str = "Failed to fetch projects";
pub const OPEN_CONTEXT: &str = "Failed to open projects";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Options {
    pub team: Option<String>,
    pub all_teams: bool,
    pub status: Option<String>,
    pub json: bool,
    pub web: bool,
    pub app: bool,
}

pub fn check_conflicting_flags(options: &Options) -> Result<(), AppError> {
    if options.team.is_some() && options.all_teams {
        return Err(AppError::new(
            AppErrorKind::Validation,
            "Cannot use both --team and --all-teams flags",
        ));
    }
    Ok(())
}

pub fn filter(team_key: Option<&str>, status: Option<&str>) -> Option<ProjectFilter> {
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

pub async fn run_with<F, Fut, Now>(
    mut fetch: F,
    team_key: Option<&str>,
    status: Option<&str>,
    json: bool,
    now: Now,
    columns: usize,
    color: bool,
) -> Result<Vec<u8>, AppError>
where
    F: FnMut(GraphQlRequest<GetProjectsVariables>) -> Fut,
    Fut: Future<Output = Result<GetProjects, AppError>>,
    Now: FnOnce() -> SystemTime,
{
    run_uncontextualized(&mut fetch, team_key, status, json, now, columns, color)
        .await
        .map_err(|error| error.with_context(FETCH_CONTEXT))
}

async fn run_uncontextualized<F, Fut, Now>(
    mut fetch: F,
    team_key: Option<&str>,
    status: Option<&str>,
    json: bool,
    now: Now,
    columns: usize,
    color: bool,
) -> Result<Vec<u8>, AppError>
where
    F: FnMut(GraphQlRequest<GetProjectsVariables>) -> Fut,
    Fut: Future<Output = Result<GetProjects, AppError>>,
    Now: FnOnce() -> SystemTime,
{
    let filter = filter(team_key, status);
    let pages = pagination::paginate_with_policy(EmptyCursorPolicy::Allow, |after| {
        let request = GraphQlRequest::with_variables(GetProjects::build(GetProjectsVariables {
            filter: filter.clone(),
            first: Some(100),
            after,
        }));
        let future = fetch(request);
        async move {
            let data = future.await?;
            Ok::<Page<Project>, AppError>(Page {
                nodes: data.projects.nodes,
                page_info: data.projects.page_info.into(),
            })
        }
    })
    .await
    .map_err(|error| match error {
        PaginationError::Fetch { source, .. } => source,
        PaginationError::MissingCursor { page } => AppError::new(
            AppErrorKind::Validation,
            format!(
                "Linear reported more projects but returned no pagination cursor on page {page}"
            ),
        )
        .with_suggestion("Retry the command."),
        PaginationError::RepeatedCursor { page, .. } => AppError::new(
            AppErrorKind::Validation,
            format!("Linear repeated a project pagination cursor on page {page}"),
        )
        .with_suggestion("Retry the command."),
    })?;

    let mut projects = pages.nodes;
    if projects.is_empty() {
        if !json {
            return Ok(b"No projects found.\n".to_vec());
        }
    } else {
        let collator = collation::root()?;
        projects.sort_by(|left, right| {
            left.sort_order
                .get()
                .total_cmp(&right.sort_order.get())
                .then_with(|| collator.compare(&left.name, &right.name))
                .then_with(|| collator.compare(left.id.inner(), right.id.inner()))
        });
    }
    if json {
        render_json(
            &projects,
            &PageInfo {
                has_next_page: pages.page_info.has_next_page,
                end_cursor: pages.page_info.end_cursor,
            },
        )
    } else {
        Ok(render_text(&projects, now(), columns, color)?.into_bytes())
    }
}

pub async fn run(
    transport: &GraphQlTransport,
    team_key: Option<&str>,
    status: Option<&str>,
    json: bool,
    columns: usize,
    color: bool,
) -> Result<Vec<u8>, AppError> {
    run_with(
        |request| async move { transport.execute(&request).await.map_err(AppError::from) },
        team_key,
        status,
        json,
        SystemTime::now,
        columns,
        color,
    )
    .await
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

fn render_json(projects: &[Project], page_info: &PageInfo) -> Result<Vec<u8>, AppError> {
    let nodes = projects
        .iter()
        .map(|project| {
            Ok(JsonProject {
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
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    let mut bytes =
        serde_json::to_vec_pretty(&JsonConnection { nodes, page_info }).map_err(|error| {
            AppError::new(AppErrorKind::Invariant, "could not serialize projects")
                .with_source(error)
        })?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn priority_label(priority: i32) -> String {
    match priority {
        0 => "None".to_owned(),
        1 => "Urgent".to_owned(),
        2 => "High".to_owned(),
        3 => "Medium".to_owned(),
        4 => "Low".to_owned(),
        other => other.to_string(),
    }
}

fn display_date(project: &Project, now: SystemTime) -> Result<String, AppError> {
    let updated = || format!("Updated {}", time_ago(&project.updated_at.0, now));
    let created = || format!("Created {}", time_ago(&project.created_at.0, now));
    match &project.status.status_type {
        ProjectStatusType::Started => Ok(project.started_at.as_ref().map_or_else(
            || {
                project
                    .start_date
                    .as_ref()
                    .map_or_else(created, |date| format!("Start: {}", date.0))
            },
            |date| format!("Started {}", time_ago(&date.0, now)),
        )),
        ProjectStatusType::Completed => Ok(project
            .completed_at
            .as_ref()
            .map_or_else(updated, |date| format!("Done {}", time_ago(&date.0, now)))),
        ProjectStatusType::Canceled => {
            Ok(project.canceled_at.as_ref().map_or_else(updated, |date| {
                format!("Canceled {}", time_ago(&date.0, now))
            }))
        }
        ProjectStatusType::Planned => Ok(project.start_date.as_ref().map_or_else(
            || {
                project
                    .target_date
                    .as_ref()
                    .map_or_else(created, |date| format!("Target: {}", date.0))
            },
            |date| format!("Start: {}", date.0),
        )),
        ProjectStatusType::Backlog | ProjectStatusType::Paused => Ok(updated()),
        ProjectStatusType::Unknown(value) => Err(AppError::new(
            AppErrorKind::Validation,
            format!("Linear returned an unknown project status type: {value}"),
        )
        .with_suggestion("Update the CLI, or report this if it persists.")),
    }
}

pub fn render_text(
    projects: &[Project],
    now: SystemTime,
    columns: usize,
    color: bool,
) -> Result<String, AppError> {
    if projects.is_empty() {
        return Ok("No projects found.\n".to_owned());
    }
    let dates = projects
        .iter()
        .map(|project| display_date(project, now))
        .collect::<Result<Vec<_>, _>>()?;
    let slug_width = projects
        .iter()
        .map(|project| utf16_len(&project.slug_id))
        .max()
        .unwrap_or(0)
        .max(4);
    let status_width = projects
        .iter()
        .map(|project| utf16_len(&project.status.name))
        .max()
        .unwrap_or(0)
        .max(6);
    let priority_width = projects
        .iter()
        .map(|project| utf16_len(&priority_label(project.priority)))
        .max()
        .unwrap_or(0)
        .max(8);
    let health_width = projects
        .iter()
        .map(|project| utf16_len(health(project)))
        .max()
        .unwrap_or(0)
        .max(6);
    let lead_width = projects
        .iter()
        .map(|project| utf16_len(lead(project)))
        .max()
        .unwrap_or(0)
        .max(4);
    let team_width = projects
        .iter()
        .map(|project| utf16_len(&teams(project)))
        .max()
        .unwrap_or(0)
        .max(5);
    let date_width = dates
        .iter()
        .map(|date| utf16_len(date))
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
    let name_width = max_name_width.min(columns.saturating_sub(1).saturating_sub(fixed));
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
        let name = truncate_js(&project.name, name_width);
        let status = pad(&project.status.name, status_width);
        let priority = pad(&priority_label(project.priority), priority_width);
        let health = pad(health(project), health_width);
        let lead = pad(lead(project), lead_width);
        let teams = pad(&teams(project), team_width);
        let date = pad(&date, date_width);
        if color {
            output.push_str(&format!("{slug} {name} "));
            if let Some(ansi) = crate::commands::table::terminal_color(&project.status.color) {
                output.push_str(&ansi);
                output.push_str(&status);
                output.push_str("\x1b[39m");
            } else {
                output.push_str(&status);
            }
            output.push_str(&format!(
                " {priority} {health} {lead} {teams} \x1b[38;2;128;128;128m{date}\x1b[39m\x1b[0m\n"
            ));
        } else {
            output.push_str(&format!(
                "{slug} {name} {status} {priority} {health} {lead} {teams} {date}\n"
            ));
        }
    }
    Ok(output)
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

pub fn opening(workspace: &str, team_key: Option<&str>, app: bool) -> (String, Vec<u8>) {
    let url = team_key.map_or_else(
        || format!("https://linear.app/{workspace}/projects/all"),
        |team| format!("https://linear.app/{workspace}/team/{team}/projects/all"),
    );
    let destination = if app { "Linear.app" } else { "web browser" };
    let line = format!("Opening {url} in {destination}\n").into_bytes();
    (url, line)
}

pub async fn viewer_workspace(transport: &GraphQlTransport) -> Result<String, AppError> {
    let request = GraphQlRequest::without_variables(GetViewer::build(()));
    let result: GetViewer = transport.execute(&request).await.map_err(AppError::from)?;
    Ok(result.viewer.organization.url_key)
}

pub fn open(url: &str, app: bool) -> Result<(), AppError> {
    opener::open(url, app).map_err(|error| error.with_context(OPEN_CONTEXT))
}
