//! Typed lookup and detail rendering for `cycle view`.

use std::cell::RefCell;
use std::collections::HashSet;
use std::future::Future;
use std::rc::Rc;

use chrono::{DateTime, TimeZone, Utc};
use cynic::QueryBuilder;
use serde::Serialize;
use serde_json::value::RawValue;

use crate::commands::relative_time::format_relative_time;
use crate::error::{AppError, AppErrorKind};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::cycle_view::{
    ActiveCycle, DetailCycle, DetailVariables, GetCycleDetails, GetTeamCyclesForLookup,
    LookupCycle, LookupVariables,
};
use crate::graphql::transport::{GraphQlTransport, RawHttpResponse};
use crate::json_number::finite_js_number;
use crate::refs::{CycleSelector, LinearUrlRef};

pub const CONTEXT: &str = "Failed to fetch cycle details";
const SIMPLE_SUGGESTION: &str = "Use a cycle number or name instead.";

fn js_number(number: f64) -> Result<String, AppError> {
    Ok(finite_js_number(number)?.get().to_owned())
}

fn integer_as_f64(value: impl ToString) -> Result<f64, AppError> {
    value.to_string().parse::<f64>().map_err(|error| {
        AppError::new(
            AppErrorKind::Invariant,
            "could not convert integer to number",
        )
        .with_source(error)
    })
}

fn protocol(message: String) -> AppError {
    AppError::new(AppErrorKind::Validation, message)
}

pub fn lookup_request(team_id: &str, after: Option<String>) -> GraphQlRequest<LookupVariables> {
    GraphQlRequest::with_variables(GetTeamCyclesForLookup::build(LookupVariables {
        team_id: team_id.to_owned(),
        after,
    }))
}

pub fn detail_request(id: &str) -> GraphQlRequest<DetailVariables> {
    GraphQlRequest::with_variables(GetCycleDetails::build(DetailVariables {
        id: id.to_owned(),
    }))
}

fn validate_first_team(
    key: &str,
    enabled: bool,
    url: Option<&LinearUrlRef>,
) -> Result<(), AppError> {
    if let Some(LinearUrlRef::Cycle { team_key, .. }) = url
        && team_key.to_uppercase() != key.to_uppercase()
    {
        return Err(protocol(format!(
            "That cycle URL is for team {team_key}, but this command is working in team {key}."
        ))
        .with_suggestion(format!("Pass --team {team_key}.")));
    }
    if !enabled {
        return Err(protocol(format!("Cycles are not enabled for team {key}"))
            .with_suggestion("Enable cycles for the team in Linear's settings before filtering or assigning by cycle."));
    }
    Ok(())
}

/// Fetch every lookup page before choosing a cycle, as the source does.
pub async fn resolve_id_with<F, Fut>(
    team_id: &str,
    reference: &str,
    url: Option<&LinearUrlRef>,
    mut fetch: F,
) -> Result<String, AppError>
where
    F: FnMut(GraphQlRequest<LookupVariables>) -> Fut,
    Fut: Future<Output = Result<GetTeamCyclesForLookup, AppError>>,
{
    let first = fetch(lookup_request(team_id, None)).await?;
    let team = first
        .team
        .ok_or_else(|| AppError::not_found("Team", team_id))?;
    validate_first_team(&team.key, team.cycles_enabled, url)?;
    let key = team.key;
    let active = team.active_cycle;
    let mut cycles = team.cycles.nodes;
    let mut page_info = team.cycles.page_info;
    let mut seen = HashSet::new();
    let mut page = 1;
    while page_info.has_next_page {
        let cursor = page_info.end_cursor.ok_or_else(|| {
            protocol(format!(
                "Linear returned no cycle pagination cursor for team {key} on page {page}"
            ))
        })?;
        if !seen.insert(cursor.clone()) {
            return Err(protocol(format!(
                "Linear repeated a cycle pagination cursor for team {key} on page {page}"
            )));
        }
        page += 1;
        let next = fetch(lookup_request(team_id, Some(cursor))).await?;
        let next_team = next
            .team
            .ok_or_else(|| AppError::not_found("Team", team_id))?;
        cycles.extend(next_team.cycles.nodes);
        page_info = next_team.cycles.page_info;
    }
    select(&cycles, active.as_ref(), &key, reference, url)
}

/// Classify one completed lookup HTTP response. The raw inspection is limited
/// to the schema-invalid `cycles: null` shape; all other data uses Cynic.
pub fn classify_lookup_page(
    response: RawHttpResponse,
    page: usize,
    url: Option<&LinearUrlRef>,
    first_key: &mut Option<String>,
) -> Result<GetTeamCyclesForLookup, AppError> {
    if response.status.is_success()
        && let Ok(value) = serde_json::from_slice::<serde_json::Value>(&response.body)
    {
        let errors = value.get("errors").and_then(serde_json::Value::as_array);
        let has_errors = errors.is_some_and(|entries| !entries.is_empty());
        if !has_errors {
            let raw_team = value.pointer("/data/team");
            let key = raw_team
                .and_then(|team| team.get("key"))
                .and_then(serde_json::Value::as_str);
            if page == 1
                && let Some(key) = key
            {
                *first_key = Some(key.to_owned());
            }
            if raw_team
                .and_then(|team| team.get("cycles"))
                .is_some_and(serde_json::Value::is_null)
            {
                // A null team and invalid metadata remain typed decode/team
                // errors. For page one, Deno checks URL and enabled first.
                if page == 1 {
                    if let (Some(key), Some(enabled)) = (
                        key,
                        raw_team
                            .and_then(|team| team.get("cyclesEnabled"))
                            .and_then(serde_json::Value::as_bool),
                    ) {
                        validate_first_team(key, enabled, url)?;
                        return Err(protocol(format!(
                            "Linear returned a null cycle connection for team {key} on page {page}"
                        )));
                    }
                } else if let Some(key) = first_key.as_deref() {
                    return Err(protocol(format!(
                        "Linear returned a null cycle connection for team {key} on page {page}"
                    )));
                }
            }
        }
    }
    crate::graphql::transport::classify_typed(response).map_err(AppError::from)
}

pub async fn resolve_id(
    transport: &GraphQlTransport,
    team_id: &str,
    reference: &str,
    url: Option<&LinearUrlRef>,
) -> Result<String, AppError> {
    let mut page = 0;
    let first_key = Rc::new(RefCell::new(None));
    resolve_id_with(team_id, reference, url, |request| {
        page += 1;
        let current_page = page;
        let first_key = Rc::clone(&first_key);
        async move {
            let response = transport
                .send_request(&request)
                .await
                .map_err(AppError::from)?;
            classify_lookup_page(response, current_page, url, &mut first_key.borrow_mut())
        }
    })
    .await
}

fn offset(reference: &str) -> bool {
    let Some(digits) = reference.strip_prefix(['+', '-']) else {
        return false;
    };
    !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
}

fn select(
    cycles: &[LookupCycle],
    active: Option<&ActiveCycle>,
    key: &str,
    original: &str,
    url: Option<&LinearUrlRef>,
) -> Result<String, AppError> {
    let reference = match url {
        Some(LinearUrlRef::Cycle {
            cycle: CycleSelector::Number(number),
            ..
        }) => {
            let target_number = integer_as_f64(*number)?;
            return cycles
                .iter()
                .find(|cycle| cycle.number == target_number)
                .map(|cycle| cycle.id.inner().to_owned())
                .ok_or_else(|| AppError::not_found("Cycle", &format!("#{number} in team {key}")));
        }
        Some(LinearUrlRef::Cycle {
            cycle: CycleSelector::Active,
            ..
        }) => "active",
        Some(LinearUrlRef::Cycle {
            cycle: CycleSelector::Next,
            ..
        }) => "next",
        Some(_) => return Err(AppError::new(AppErrorKind::Invariant, "expected cycle URL")),
        None => original,
    };
    let keyword = reference.to_lowercase();
    match keyword.as_str() {
        "active" | "now" => {
            if let Some(active) = active {
                return Ok(active.id.inner().to_owned());
            }
            let suggestion = if let Some(next) = cycles.iter().find(|cycle| cycle.is_next) {
                format!(
                    "The next cycle (#{}) starts {} — use --cycle next, a cycle number, or a name.",
                    js_number(next.number)?,
                    next.starts_at.0.chars().take(10).collect::<String>()
                )
            } else {
                SIMPLE_SUGGESTION.to_owned()
            };
            return Err(
                protocol(format!("Team {key} has no active cycle")).with_suggestion(suggestion)
            );
        }
        "next" => {
            return cycles
                .iter()
                .find(|cycle| cycle.is_next)
                .map(|cycle| cycle.id.inner().to_owned())
                .ok_or_else(|| {
                    protocol(format!("Team {key} has no upcoming cycle"))
                        .with_suggestion(SIMPLE_SUGGESTION)
                });
        }
        "previous" => {
            return cycles
                .iter()
                .find(|cycle| cycle.is_previous)
                .map(|cycle| cycle.id.inner().to_owned())
                .ok_or_else(|| {
                    protocol(format!("Team {key} has no previous cycle"))
                        .with_suggestion(SIMPLE_SUGGESTION)
                });
        }
        _ => {}
    }
    if offset(reference) {
        let magnitude = reference
            .get(1..)
            .and_then(|digits| digits.parse::<u64>().ok());
        let magnitude = magnitude
            .filter(|value| *value <= 9_007_199_254_740_991)
            .ok_or_else(|| protocol(format!("Cycle offset {reference} is out of range")))?;
        let active = active.ok_or_else(|| {
            protocol(format!(
                "Cannot resolve relative cycle {reference}: the team has no active cycle"
            ))
            .with_suggestion(
                "Use 'next', a cycle number, or a cycle name while no cycle is active.",
            )
        })?;
        let magnitude = integer_as_f64(magnitude)?;
        let signed = if reference.starts_with('-') {
            -magnitude
        } else {
            magnitude
        };
        let target = active.number + signed;
        let target_number = js_number(target)?;
        return cycles
            .iter()
            .find(|cycle| cycle.number == target)
            .map(|cycle| cycle.id.inner().to_owned())
            .ok_or_else(|| {
                AppError::not_found("Cycle", &format!("{reference} (cycle {target_number})"))
            });
    }
    for cycle in cycles {
        if cycle
            .name
            .as_deref()
            .is_some_and(|name| name.to_lowercase() == keyword)
            || js_number(cycle.number)? == reference
        {
            return Ok(cycle.id.inner().to_owned());
        }
    }
    Err(AppError::not_found("Cycle", reference))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonCycle<'a> {
    id: &'a cynic::Id,
    number: Box<RawValue>,
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
    issues: JsonIssues<'a>,
}
#[derive(Serialize)]
struct JsonTeam<'a> {
    id: &'a cynic::Id,
    key: &'a str,
    name: &'a str,
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

pub fn json(cycle: &DetailCycle) -> Result<Vec<u8>, AppError> {
    let projected = JsonCycle {
        id: &cycle.id,
        number: finite_js_number(cycle.number)?,
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
        issues: JsonIssues {
            nodes: cycle
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
            page_info: &cycle.issues.page_info,
        },
    };
    let mut bytes = serde_json::to_vec_pretty(&projected).map_err(|error| {
        AppError::new(AppErrorKind::Invariant, "could not serialize cycle").with_source(error)
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub fn markdown<Tz: TimeZone>(
    cycle: &DetailCycle,
    now: DateTime<Utc>,
    zone: &Tz,
) -> Result<String, AppError> {
    let number = js_number(cycle.number)?;
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
        format!(
            "**Start:** {}",
            cycle.starts_at.0.chars().take(10).collect::<String>()
        ),
        format!(
            "**End:** {}",
            cycle.ends_at.0.chars().take(10).collect::<String>()
        ),
        format!("**Status:** {status}"),
        format!("**Team:** {} ({})", cycle.team.name, cycle.team.key),
        String::new(),
        format!(
            "**Created:** {}",
            format_relative_time(&cycle.created_at.0, now, zone)
        ),
        format!(
            "**Updated:** {}",
            format_relative_time(&cycle.updated_at.0, now, zone)
        ),
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
        let percent = ((integer_as_f64(completed)? / integer_as_f64(total)?) * 100.0 + 0.5).floor();
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
    Ok(lines.join("\n"))
}
