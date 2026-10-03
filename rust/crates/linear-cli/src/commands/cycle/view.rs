//! `cycle view`: one cycle by number, name, URL or relative offset.

use chrono::{DateTime, TimeZone, Utc};
use serde::Serialize;

use crate::cli::cycle::CycleView;
use crate::commands::json;
use crate::commands::relative_time::ago;
use crate::commands::team_key::{configured_team_key, no_team};
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::cycle::{DetailCycle, DetailVariables, GetCycleDetails};
use crate::graphql::scalars::WholeNumber;
use crate::refs::{self, cycle::CycleReference, team::TeamReference};

pub fn run(ctx: &Ctx, args: &CycleView) -> Result<()> {
    view(ctx, args).context("Failed to fetch cycle details")
}

fn view(ctx: &Ctx, args: &CycleView) -> Result<()> {
    let scope = ctx.scope()?;
    let reference = CycleReference::parse(&args.cycle_ref, &scope)?;
    let team = args
        .team
        .clone()
        .or_else(|| reference.url_team_key().map(str::to_owned))
        .or_else(|| configured_team_key(ctx.options()))
        .ok_or_else(no_team)?;
    let team = TeamReference::parse(&team, &scope)?;
    let client = ctx.client()?;
    let cycle = ctx.spin(!args.json, async {
        let team = refs::team::resolve(client, &team).await?;
        let id = refs::cycle::resolve(client, &team.id, &reference).await?;
        let details: GetCycleDetails = client.query(DetailVariables { id }).await?;
        details
            .cycle
            .ok_or_else(|| Error::not_found("Cycle", &args.cycle_ref))
    })?;
    if args.json {
        ctx.print(render_json(&cycle))
    } else {
        ctx.show_markdown(&markdown(&cycle, Utc::now(), &chrono::Local), false)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonCycle<'a> {
    id: &'a cynic::Id,
    number: WholeNumber,
    name: &'a Option<String>,
    description: &'a Option<String>,
    starts_at: &'a crate::graphql::scalars::DateTime,
    ends_at: &'a crate::graphql::scalars::DateTime,
    completed_at: &'a Option<crate::graphql::scalars::DateTime>,
    is_active: bool,
    is_future: bool,
    is_past: bool,
    created_at: &'a crate::graphql::scalars::DateTime,
    updated_at: &'a crate::graphql::scalars::DateTime,
    team: JsonTeam<'a>,
    issues: Vec<JsonIssue<'a>>,
}
#[derive(Serialize)]
struct JsonTeam<'a> {
    id: &'a cynic::Id,
    key: &'a str,
    name: &'a str,
}
#[derive(Serialize)]
struct JsonIssue<'a> {
    id: &'a cynic::Id,
    identifier: &'a str,
    title: &'a str,
    state: JsonState<'a>,
}
#[derive(Serialize)]
struct JsonState<'a> {
    name: &'a str,
    #[serde(rename = "type")]
    state_type: &'a str,
}

fn render_json(cycle: &DetailCycle) -> Vec<u8> {
    let projected = JsonCycle {
        id: &cycle.id,
        number: cycle.number,
        name: &cycle.name,
        description: &cycle.description,
        starts_at: &cycle.starts_at,
        ends_at: &cycle.ends_at,
        completed_at: &cycle.completed_at,
        is_active: cycle.is_active,
        is_future: cycle.is_future,
        is_past: cycle.is_past,
        created_at: &cycle.created_at,
        updated_at: &cycle.updated_at,
        team: JsonTeam {
            id: &cycle.team.id,
            key: &cycle.team.key,
            name: &cycle.team.name,
        },
        issues: cycle
            .issues
            .nodes
            .iter()
            .map(|issue| JsonIssue {
                id: &issue.id,
                identifier: &issue.identifier,
                title: &issue.title,
                state: JsonState {
                    name: &issue.state.name,
                    state_type: &issue.state.state_type,
                },
            })
            .collect(),
    };
    json::render(&projected)
}

fn markdown<Tz: TimeZone>(cycle: &DetailCycle, now: DateTime<Utc>, zone: &Tz) -> String {
    let number = cycle.number;
    let title = cycle
        .name
        .as_deref()
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("Cycle {number}"));
    let status = if cycle.is_active {
        "Active"
    } else if cycle.is_future {
        "Upcoming"
    } else if cycle.completed_at.is_some() {
        "Completed"
    } else if cycle.is_past {
        "Past"
    } else {
        "Unknown"
    };
    let mut lines = vec![
        format!("# {title}"),
        String::new(),
        format!("**Number:** {number}"),
        format!("**Start:** {}", cycle.starts_at.0.date_naive()),
        format!("**End:** {}", cycle.ends_at.0.date_naive()),
        format!("**Status:** {status}"),
        format!("**Team:** {} ({})", cycle.team.name, cycle.team.key),
        String::new(),
        format!("**Created:** {}", ago(cycle.created_at.0, now, zone)),
        format!("**Updated:** {}", ago(cycle.updated_at.0, now, zone)),
    ];
    if let Some(description) = cycle
        .description
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        lines.extend([
            String::new(),
            "## Description".to_owned(),
            String::new(),
            description.to_owned(),
        ]);
    }
    let issues = &cycle.issues.nodes;
    if issues.is_empty() {
        lines.extend([String::new(), "_No issues in this cycle yet._".to_owned()]);
    } else {
        let count = |kind: &str| {
            issues
                .iter()
                .filter(|issue| issue.state.state_type == kind)
                .count()
        };
        let completed = count("completed");
        let total = issues.len();
        let percent = (completed * 200 + total) / (total * 2);
        lines.extend([
            String::new(),
            "## Issues".to_owned(),
            String::new(),
            format!("**Progress:** {completed}/{total} ({percent}%)"),
            format!("**Total Issues:** {total}"),
        ]);
        for (kind, label) in [
            ("completed", "Completed"),
            ("started", "In Progress"),
            ("unstarted", "To Do"),
            ("backlog", "Backlog"),
            ("triage", "Triage"),
            ("canceled", "Canceled"),
        ] {
            let value = count(kind);
            if value > 0 {
                lines.push(format!("**{label}:** {value}"));
            }
        }
        lines.extend([String::new(), "**Issues:**".to_owned(), String::new()]);
        for issue in issues.iter().take(10) {
            lines.push(format!(
                "- {}: {} ({})",
                issue.identifier, issue.title, issue.state.name
            ));
        }
        if total > 10 {
            lines.extend([
                String::new(),
                format!("_...and {} more issues_", total - 10),
            ]);
        }
    }
    lines.join("\n")
}
