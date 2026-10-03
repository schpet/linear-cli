//! `initiative view`: an initiative's details as Markdown or JSON, or opened in Linear.
use chrono::{DateTime, Local, Utc};
use serde::Serialize;

use crate::cli::initiative::InitiativeView;
use crate::client::LinearClient;
use crate::commands::json;
use crate::commands::relative_time::ago;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::initiative::{
    DetailVariables, GetInitiativeDetails, InitiativeDetails,
};
use crate::graphql::operations::initiative::{InitiativeStatus, InitiativeUpdateHealthType};
use crate::graphql::operations::project::ProjectStatusType;
use crate::graphql::scalars;
use crate::refs::{self, initiative::Archived};

pub fn run(ctx: &Ctx, args: &InitiativeView) -> Result<()> {
    view(ctx, args).context("Failed to view initiative")
}

fn view(ctx: &Ctx, args: &InitiativeView) -> Result<()> {
    let original = &args.initiative_id;
    let reference = super::common::reference(ctx, original)?;
    let client = ctx.client()?;
    let detail = ctx.spin(!args.json, async {
        let id = refs::initiative::resolve(client, &reference, Archived::Exclude).await?;
        fetch(client, id, original).await
    })?;
    if args.web || args.app {
        return ctx.open_url(&detail.url, args.app);
    }
    if args.json {
        return ctx.print(render_json(&detail));
    }
    let now = Utc::now();
    if !ctx.stdout_tty() {
        return ctx.print(format!("{}\n", markdown(&detail, now, false)));
    }
    let rendered = ctx.render_markdown(&markdown(&detail, now, true));
    let status = format!("**Status:** {}", detail.status.as_str());
    let status = super::list::status_style(&detail.status, &status, ctx.color());
    ctx.print(format!("{status}\n{rendered}\n"))
}

async fn fetch(client: &LinearClient, id: String, original: &str) -> Result<InitiativeDetails> {
    let result: GetInitiativeDetails = client.query(DetailVariables { id }).await?;
    let detail = result
        .initiative
        .ok_or_else(|| Error::not_found("Initiative", original))?;
    verify_detail(&detail)?;
    Ok(detail)
}

fn verify_detail(detail: &InitiativeDetails) -> Result<()> {
    if let InitiativeStatus::Unknown(value) = &detail.status {
        return Err(Error::new(format!(
            "Linear returned an unknown initiative status: {value}"
        )));
    }
    if let Some(InitiativeUpdateHealthType::Unknown(value)) = &detail.health {
        return Err(Error::new(format!(
            "Linear returned an unknown initiative health: {value}"
        )));
    }
    for project in &detail.projects.nodes {
        if let ProjectStatusType::Unknown(value) = &project.status.status_type {
            return Err(Error::new(format!(
                "Linear returned an unknown project status type: {value}"
            )));
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
    target_date: Option<&'a scalars::TimelessDate>,
    health: Option<&'a str>,
    color: Option<&'a str>,
    icon: Option<&'a str>,
    url: &'a str,
    archived_at: Option<&'a scalars::DateTime>,
    created_at: &'a scalars::DateTime,
    updated_at: &'a scalars::DateTime,
    owner: Option<JsonOwner<'a>>,
    projects: Vec<JsonProject<'a>>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonOwner<'a> {
    id: &'a cynic::Id,
    name: &'a str,
    display_name: &'a str,
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

fn render_json(detail: &InitiativeDetails) -> Vec<u8> {
    let projection = JsonDetail {
        id: &detail.id,
        slug_id: &detail.slug_id,
        name: &detail.name,
        description: detail.description.as_deref(),
        status: detail.status.as_str(),
        target_date: detail.target_date.as_ref(),
        health: detail
            .health
            .as_ref()
            .map(InitiativeUpdateHealthType::as_str),
        color: detail.color.as_deref(),
        icon: detail.icon.as_deref(),
        url: &detail.url,
        archived_at: detail.archived_at.as_ref(),
        created_at: &detail.created_at,
        updated_at: &detail.updated_at,
        owner: detail.owner.as_ref().map(|o| JsonOwner {
            id: &o.id,
            name: &o.name,
            display_name: &o.display_name,
        }),
        projects: detail
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
    };
    json::render(&projection)
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

fn markdown(detail: &InitiativeDetails, now: DateTime<Utc>, terminal: bool) -> String {
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
        lines.push(format!("**Target Date:** {date}"));
    }
    if let Some(date) = &detail.archived_at {
        lines.push(format!("**Archived:** {}", ago(date.0, now, &Local)));
    }
    lines.push(String::new());
    lines.push(format!(
        "**Created:** {}",
        ago(detail.created_at.0, now, &Local)
    ));
    lines.push(format!(
        "**Updated:** {}",
        ago(detail.updated_at.0, now, &Local)
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
