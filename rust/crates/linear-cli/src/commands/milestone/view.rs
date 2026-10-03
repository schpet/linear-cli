//! `milestone view`: one milestone and its issues, as Markdown or JSON.
use chrono::{DateTime, TimeZone, Utc};
use cynic::QueryBuilder;
use serde::Serialize;

use crate::cli::milestone::MilestoneView;
use crate::client::LinearClient;
use crate::commands::json;
use crate::commands::relative_time::format_relative_time;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::LegacyRequest;
use crate::graphql::operations::milestone_view::{
    DetailMilestone, DetailVariables, GetMilestoneDetails,
};
use crate::graphql::operations::number::Float;
use crate::graphql::pagination::{self, Page};
use crate::refs::{
    is_linear_uuid, prepare_project_lookup, reject_linear_url, resolve_project_with_transport,
};

const LIST_PREVIEW: usize = 10;

pub fn run(ctx: &Ctx, args: &MilestoneView) -> Result<()> {
    view(ctx, args).context("Failed to view milestone")
}

fn view(ctx: &Ctx, args: &MilestoneView) -> Result<()> {
    reject_linear_url(&args.milestone, "a milestone name or UUID")?;
    let project = match args.project.as_deref() {
        Some(project) => Some((prepare_project_lookup(project, &ctx.scope()?)?, project)),
        None => None,
    };
    let by_name = !is_linear_uuid(&args.milestone);
    if by_name && project.is_none() {
        return Err(
            Error::new(format!("\"{}\" is not a milestone UUID", args.milestone))
                .with_hint("Pass --project to look up a milestone by name."),
        );
    }
    let client = ctx.client()?;
    let milestone = ctx.spin(!args.json, async {
        let id = match &project {
            Some((reference, original)) if by_name => {
                let project_id =
                    resolve_project_with_transport(reference, original, client).await?;
                super::id_by_name(client, &project_id, &args.milestone).await?
            }
            Some(_) | None => args.milestone.clone(),
        };
        fetch(client, &args.milestone, &id).await
    })?;
    if args.json {
        ctx.print(render_json(&milestone))
    } else {
        let markdown = markdown(&milestone, args.all, Utc::now(), &chrono::Local);
        ctx.show_markdown(&markdown, false)
    }
}

/// The milestone with every one of its issues.
async fn fetch(client: &LinearClient, original: &str, id: &str) -> Result<DetailMilestone> {
    pagination::collect_within(
        None,
        |after, first| {
            let request =
                LegacyRequest::with_variables(GetMilestoneDetails::build(DetailVariables {
                    id: id.to_owned(),
                    first,
                    after,
                }));
            async move {
                let data: GetMilestoneDetails = client.execute_legacy(&request).await?;
                data.project_milestone
                    .ok_or_else(|| Error::not_found("Milestone", original))
            }
        },
        |milestone| Page {
            nodes: std::mem::take(&mut milestone.issues.nodes),
            page_info: milestone.issues.page_info.clone(),
        },
        |milestone, page| {
            milestone.issues.nodes = page.nodes;
            milestone.issues.page_info = page.page_info;
        },
    )
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonMilestone<'a> {
    id: &'a cynic::Id,
    name: &'a str,
    description: &'a Option<String>,
    target_date: &'a Option<crate::graphql::scalars::TimelessDate>,
    sort_order: &'a Float,
    created_at: &'a crate::graphql::scalars::DateTime,
    updated_at: &'a crate::graphql::scalars::DateTime,
    project: JsonProject<'a>,
    issues: Vec<JsonIssue<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonProject<'a> {
    id: &'a cynic::Id,
    name: &'a str,
    slug_id: &'a str,
    url: &'a str,
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

fn render_json(milestone: &DetailMilestone) -> Vec<u8> {
    let output = JsonMilestone {
        id: &milestone.id,
        name: &milestone.name,
        description: &milestone.description,
        target_date: &milestone.target_date,
        sort_order: &milestone.sort_order,
        created_at: &milestone.created_at,
        updated_at: &milestone.updated_at,
        project: JsonProject {
            id: &milestone.project.id,
            name: &milestone.project.name,
            slug_id: &milestone.project.slug_id,
            url: &milestone.project.url,
        },
        issues: milestone
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
    json::render(&output)
}

fn markdown<Tz: TimeZone>(
    milestone: &DetailMilestone,
    all: bool,
    now: DateTime<Utc>,
    zone: &Tz,
) -> String {
    let issues = &milestone.issues.nodes;
    let mut lines = vec![
        format!("# {}", milestone.name),
        String::new(),
        format!("**ID:** {}", milestone.id.inner()),
        format!(
            "**Target Date:** {}",
            milestone
                .target_date
                .as_ref()
                .map(|date| date.0.as_str())
                .filter(|date| !date.is_empty())
                .unwrap_or("Not set")
        ),
        format!(
            "**Project:** {} ({})",
            milestone.project.name, milestone.project.slug_id
        ),
        format!("**Project URL:** {}", milestone.project.url),
        String::new(),
        format!(
            "**Created:** {}",
            format_relative_time(&milestone.created_at.0, now, zone)
        ),
        format!(
            "**Updated:** {}",
            format_relative_time(&milestone.updated_at.0, now, zone)
        ),
    ];
    if let Some(description) = milestone
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
    if issues.is_empty() {
        lines.extend([
            String::new(),
            "_No issues in this milestone yet._".to_owned(),
        ]);
        return lines.join("\n");
    }
    lines.extend([String::new(), "## Issues".to_owned(), String::new()]);
    lines.push(format!("**Total Issues:** {}", issues.len()));
    for (kind, label) in [
        ("completed", "Completed"),
        ("started", "In Progress"),
        ("unstarted", "To Do"),
        ("backlog", "Backlog"),
        ("triage", "Triage"),
        ("canceled", "Canceled"),
    ] {
        let count = issues
            .iter()
            .filter(|issue| issue.state.state_type == kind)
            .count();
        if count > 0 {
            lines.push(format!("**{label}:** {count}"));
        }
    }
    lines.extend([
        String::new(),
        if all {
            "**All Issues:**"
        } else {
            "**Recent Issues:**"
        }
        .to_owned(),
        String::new(),
    ]);
    for issue in issues
        .iter()
        .take(if all { usize::MAX } else { LIST_PREVIEW })
    {
        lines.push(format!(
            "- {}: {} ({})",
            issue.identifier, issue.title, issue.state.name
        ));
    }
    let hidden = issues.len().saturating_sub(LIST_PREVIEW);
    if !all && hidden > 0 {
        lines.extend([
            String::new(),
            format!(
                "_...and {hidden} more issue{}. Re-run with `--all` to list them._",
                if hidden == 1 { "" } else { "s" }
            ),
        ]);
    }
    lines.join("\n")
}
