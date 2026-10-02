//! Typed milestone lookup, detail pagination, and output rendering.

use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;

use chrono::{DateTime, TimeZone, Utc};
use cynic::QueryBuilder;
use serde::Serialize;

use crate::commands::relative_time::format_relative_time;
use crate::error::Error;
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::milestone_view::{
    DetailIssue, DetailMilestone, DetailVariables, GetMilestoneDetails,
    GetProjectMilestonesForLookup, LookupVariables,
};
use crate::graphql::operations::number::Float;
use crate::graphql::pagination::{self, EmptyCursorPolicy, Page, PaginationError};
use crate::graphql::transport::GraphQlTransport;
use crate::refs::is_linear_uuid;

pub const CONTEXT: &str = "Failed to fetch milestone details";
const PAGE_SIZE: i32 = 50;
const LIST_PREVIEW: usize = 10;

pub fn detail_request(id: &str, after: Option<String>) -> GraphQlRequest<DetailVariables> {
    GraphQlRequest::with_variables(GetMilestoneDetails::build(DetailVariables {
        id: id.to_owned(),
        first: PAGE_SIZE,
        after,
    }))
}

pub async fn resolve_id(
    transport: &GraphQlTransport,
    input: &str,
    project_id: &str,
) -> Result<String, Error> {
    if is_linear_uuid(input) {
        return Ok(input.to_owned());
    }
    let query =
        GraphQlRequest::with_variables(GetProjectMilestonesForLookup::build(LookupVariables {
            project_id: project_id.to_owned(),
        }));
    let data: GetProjectMilestonesForLookup =
        transport.execute(&query).await.map_err(Error::from)?;
    let project = data
        .project
        .ok_or_else(|| Error::not_found("Project", project_id))?;
    let name = input.to_lowercase();
    project
        .project_milestones
        .into_iter()
        .flat_map(|connection| connection.nodes)
        .find(|milestone| milestone.name.to_lowercase() == name)
        .map(|milestone| milestone.id.into_inner())
        .ok_or_else(|| Error::not_found("Milestone", input))
}

pub async fn fetch(
    transport: &GraphQlTransport,
    original: &str,
    request_id: &str,
    all: bool,
) -> Result<DetailMilestone, Error> {
    fetch_with(original, request_id, all, |request| async move {
        transport.execute(&request).await.map_err(Error::from)
    })
    .await
}

/// Exercise the public page contract with a scripted transport in tests.
pub async fn fetch_with<F, Fut>(
    original: &str,
    request_id: &str,
    all: bool,
    mut fetch: F,
) -> Result<DetailMilestone, Error>
where
    F: FnMut(GraphQlRequest<DetailVariables>) -> Fut,
    Fut: Future<Output = Result<GetMilestoneDetails, Error>>,
{
    if !all {
        let data = fetch(detail_request(request_id, None)).await?;
        return data
            .project_milestone
            .ok_or_else(|| Error::not_found("Milestone", original));
    }
    let first: Rc<RefCell<Option<DetailMilestone>>> = Rc::new(RefCell::new(None));
    let captured = Rc::clone(&first);
    let result = pagination::paginate_with_policy(EmptyCursorPolicy::Reject, |after| {
        let captured = Rc::clone(&captured);
        let pending = fetch(detail_request(request_id, after));
        async move {
            let data = pending.await?;
            let milestone = data
                .project_milestone
                .ok_or_else(|| Error::not_found("Milestone", original))?;
            if captured.borrow().is_none() {
                *captured.borrow_mut() = Some(milestone.clone());
            }
            Ok::<Page<DetailIssue>, Error>(Page {
                nodes: milestone.issues.nodes,
                page_info: milestone.issues.page_info.into(),
            })
        }
    })
    .await;
    let result = result.map_err(|error| match error {
        PaginationError::Fetch { source, .. } => source,
        PaginationError::MissingCursor { .. } => {
            let id = first.borrow();
            let suggestion = id.as_ref().map(|milestone| {
                format!(
                    "Retry, or use `linear issue query --milestone {} --json` for the full list.",
                    milestone.id.inner()
                )
            });
            let error = Error::new("Linear reported more issues but returned no pagination cursor");
            match suggestion {
                Some(suggestion) => error.with_hint(suggestion),
                None => error.with_hint("Retry the command."),
            }
        }
        PaginationError::RepeatedCursor { page, .. } => Error::new(format!(
            "Linear repeated an issue pagination cursor on page {page}"
        ))
        .with_hint("Retry the command."),
    })?;
    let mut milestone = first
        .borrow_mut()
        .take()
        .ok_or_else(|| Error::new("pagination returned without a first milestone"))?;
    milestone.issues.nodes = result.nodes;
    milestone.issues.page_info.has_next_page = result.page_info.has_next_page;
    milestone.issues.page_info.end_cursor = result.page_info.end_cursor;
    Ok(milestone)
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
    issues: JsonIssues<'a>,
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
#[serde(rename_all = "camelCase")]
struct JsonIssues<'a> {
    nodes: Vec<JsonIssue<'a>>,
    page_info: &'a crate::graphql::operations::teams::PageInfo,
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

pub fn json(milestone: &DetailMilestone) -> Result<Vec<u8>, Error> {
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
        issues: JsonIssues {
            nodes: milestone
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
            page_info: &milestone.issues.page_info,
        },
    };
    let mut bytes = serde_json::to_vec_pretty(&output)
        .map_err(|error| Error::new("could not serialize milestone").with_source(error))?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub fn markdown<Tz: TimeZone>(
    milestone: &DetailMilestone,
    all: bool,
    now: DateTime<Utc>,
    zone: &Tz,
) -> String {
    let issues = &milestone.issues.nodes;
    let truncated = !all && milestone.issues.page_info.has_next_page;
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
    if truncated {
        lines.push(format!(
            "**Issues fetched:** {} (milestone has more — use `--all` for full counts)",
            issues.len()
        ));
    } else {
        lines.push(format!("**Total Issues:** {}", issues.len()));
    }
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
    if !all {
        let hidden = issues.len().saturating_sub(LIST_PREVIEW);
        if truncated {
            lines.extend([String::new(), format!(
                "_Showing {} of {}+ issues — the milestone contains more than {PAGE_SIZE}. Re-run with `--all` or use `linear issue query --milestone {} --json` for the full list._",
                issues.len().min(LIST_PREVIEW), issues.len(), milestone.id.inner()
            )]);
        } else if hidden > 0 {
            lines.extend([String::new(), format!(
                "_...and {hidden} more issue{}. Re-run with `--all` or use `linear issue query --milestone {} --json` to see them all._",
                if hidden == 1 { "" } else { "s" }, milestone.id.inner()
            )]);
        }
    }
    lines.join("\n")
}
