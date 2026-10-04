//! `project list`: every project of a team (or the workspace), as a table or
//! JSON, or the projects page opened in Linear.

use std::cmp::Ordering;
use std::time::SystemTime;

use serde::Serialize;

use crate::cli::project::{ProjectList, Status};
use crate::client::LinearClient;
use crate::commands::json;
use crate::commands::relative_time::ago;
use crate::commands::table::{Cell, Column, Table};
use crate::commands::team_key::configured_team_key;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::project::{
    GetProjects, GetProjectsVariables, Project, ProjectFilter, ProjectStatusFilter,
    ProjectStatusType, TeamCollectionFilter,
};
use crate::graphql::operations::team::{StringComparator, TeamFilter};
use crate::graphql::pagination::{self, Page};
use crate::graphql::scalars::Float;
use crate::graphql::scalars::{DateTime, TimelessDate};
use crate::platform::{collation, style};
use crate::refs::{self, team::TeamReference};

pub fn run(ctx: &Ctx, args: &ProjectList) -> Result<()> {
    list(ctx, args).context("Failed to list projects")
}

fn list(ctx: &Ctx, args: &ProjectList) -> Result<()> {
    let team_lookup = args
        .team
        .as_deref()
        .map(|team| TeamReference::parse(team, &ctx.scope()?))
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
            Some(lookup) => Ok::<_, Error>(Some(refs::team::resolve(client, lookup).await?.key)),
            None => Ok(configured.clone()),
        }
    };
    if args.web || args.app {
        let team_key = ctx.spin(true, team_key)?;
        return ctx.open_in_linear(&projects_path(team_key.as_deref()), args.app);
    }
    let status = args.status;
    let mut projects = ctx.spin(!args.json, async {
        let team_key = team_key.await?;
        fetch(client, filter(team_key.as_deref(), status)).await
    })?;
    args.limit.apply(&mut projects);
    if args.json {
        ctx.print(render_json(&projects))
    } else if projects.is_empty() {
        ctx.print("No projects found.\n")
    } else {
        ctx.print(render_text(&projects, SystemTime::now()).render_for(ctx))
    }
}

/// The projects page of a team, or of the whole workspace.
fn projects_path(team_key: Option<&str>) -> String {
    team_key.map_or_else(
        || "projects/all".to_owned(),
        |team| format!("team/{team}/projects/all"),
    )
}

pub(super) fn filter(team_key: Option<&str>, status: Option<Status>) -> Option<ProjectFilter> {
    let accessible_teams = team_key.map(|key| TeamCollectionFilter {
        some: Some(TeamFilter {
            key: Some(StringComparator {
                eq: Some(key.to_owned()),
                ..Default::default()
            }),
            ..Default::default()
        }),
    });
    let status = status.map(|status| ProjectStatusFilter {
        status_type: Some(StringComparator {
            eq: Some(ProjectStatusType::from(status).as_str().to_owned()),
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
async fn fetch(client: &LinearClient, filter: Option<ProjectFilter>) -> Result<Vec<Project>> {
    let mut projects = pagination::collect(None, |after, first| {
        let variables = GetProjectsVariables {
            filter: filter.clone(),
            first: Some(first),
            after,
        };
        async move {
            let data: GetProjects = client.query(variables).await?;
            Ok(Page {
                nodes: data.projects.nodes,
                page_info: data.projects.page_info,
            })
        }
    })
    .await?;
    projects.sort_by(|left, right| {
        manual_order(
            (left.sort_order.get(), &left.name, left.id.inner()),
            (right.sort_order.get(), &right.name, right.id.inner()),
        )
    });
    Ok(projects)
}

/// The order of Linear's project list: the manual `sortOrder` projects are
/// dragged into, then name and id to break ties. Each side is `(sortOrder,
/// name, id)`.
pub(super) fn manual_order(left: (f64, &str, &str), right: (f64, &str, &str)) -> Ordering {
    left.0
        .total_cmp(&right.0)
        .then_with(|| collation::compare(left.1, right.1))
        .then_with(|| collation::compare(left.2, right.2))
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
    status: &'a crate::graphql::operations::project::ProjectStatus,
    lead: Option<&'a crate::graphql::operations::project::ProjectLead>,
    priority: i32,
    health: Option<&'a crate::graphql::operations::project::ProjectUpdateHealthType>,
    start_date: Option<&'a TimelessDate>,
    target_date: Option<&'a TimelessDate>,
    started_at: Option<&'a DateTime>,
    completed_at: Option<&'a DateTime>,
    canceled_at: Option<&'a DateTime>,
    created_at: &'a DateTime,
    updated_at: &'a DateTime,
    url: &'a str,
    teams: &'a crate::graphql::operations::project::ProjectTeams,
}

fn render_json(projects: &[Project]) -> Vec<u8> {
    let projects: Vec<_> = projects
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
    json::render(&projects)
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
        format!("{label} {}", ago(date.0, now.into(), &chrono::Local))
    };
    let planned = |label: &str, date: &TimelessDate| format!("{label}: {date}");
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

fn render_text(projects: &[Project], now: SystemTime) -> Table {
    let mut table = Table::new([
        Column::fixed("SLUG"),
        Column::flexible("NAME"),
        Column::fixed("STATUS"),
        Column::fixed("PRIORITY").droppable(4),
        Column::fixed("HEALTH").droppable(5),
        Column::fixed("LEAD").droppable(3),
        Column::fixed("TEAMS").droppable(1),
        Column::fixed("DATE").droppable(2),
    ]);
    for project in projects {
        let hex = project.status.color.clone();
        table.row([
            Cell::from(project.slug_id.as_str()),
            Cell::from(project.name.as_str()),
            Cell::styled(project.status.name.as_str(), move |text, on| {
                style::rgb(text, &hex, on)
            }),
            Cell::from(priority_label(project.priority)),
            Cell::from(health(project)),
            Cell::from(lead(project)),
            Cell::from(teams(project)),
            Cell::styled(display_date(project, now), style::gray),
        ]);
    }
    table
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
