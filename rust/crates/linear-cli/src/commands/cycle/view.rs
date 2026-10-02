//! `cycle view`: one cycle by number, name, URL or relative offset, and the
//! cycle lookup shared with issue and document commands.

use std::cell::RefCell;
use std::collections::HashSet;
use std::future::Future;
use std::rc::Rc;

use chrono::{DateTime, TimeZone, Utc};
use cynic::QueryBuilder;
use serde::Serialize;

use crate::cli::cycle::CycleView;
use crate::commands::relative_time::format_relative_time;
use crate::commands::team_key::{configured_team_key, no_team};
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::cycle_view::{
    ActiveCycle, DetailCycle, DetailVariables, GetCycleDetails, GetTeamCyclesForLookup,
    LookupCycle, LookupVariables,
};
use crate::graphql::operations::number::WholeNumber;
use crate::graphql::transport::{GraphQlTransport, RawHttpResponse};
use crate::refs::{
    CycleSelector, LinearUrlKind, LinearUrlRef, expect_url_kind, prepare_team_lookup,
    resolve_team_with_transport,
};

pub fn run(ctx: &Ctx, args: &CycleView) -> Result<()> {
    view(ctx, args).context("Failed to fetch cycle details")
}

fn view(ctx: &Ctx, args: &CycleView) -> Result<()> {
    let reference = &args.cycle_ref;
    let scope = ctx.scope()?;
    let url = expect_url_kind(
        reference,
        LinearUrlKind::Cycle,
        "a cycle URL, number, or name",
        &scope,
    )?;
    let url_team = match &url {
        Some(LinearUrlRef::Cycle { team_key, .. }) => Some(team_key.clone()),
        Some(other) => unreachable!("expect_url_kind returned a {:?} URL", other.kind()),
        None => None,
    };
    let team = args
        .team
        .clone()
        .or(url_team)
        .or_else(|| configured_team_key(ctx.options()))
        .ok_or_else(no_team)?;
    let lookup = prepare_team_lookup(&team, &scope)?;
    let client = ctx.client()?;
    let cycle = ctx.spin(!args.json, async {
        let team = resolve_team_with_transport(&lookup, client).await?;
        let id = resolve_id(client, &team.id, reference, url.as_ref()).await?;
        let details: GetCycleDetails = client.execute(&detail_request(&id)).await?;
        details
            .cycle
            .ok_or_else(|| Error::not_found("Cycle", reference))
    })?;
    if args.json {
        ctx.print(json(&cycle))
    } else {
        ctx.show_markdown(&markdown(&cycle, Utc::now(), &chrono::Local), false)
    }
}
const SIMPLE_SUGGESTION: &str = "Use a cycle number or name instead.";

fn protocol(message: String) -> Error {
    Error::new(message)
}

fn lookup_request(team_id: &str, after: Option<String>) -> GraphQlRequest<LookupVariables> {
    GraphQlRequest::with_variables(GetTeamCyclesForLookup::build(LookupVariables {
        team_id: team_id.to_owned(),
        after,
    }))
}

fn detail_request(id: &str) -> GraphQlRequest<DetailVariables> {
    GraphQlRequest::with_variables(GetCycleDetails::build(DetailVariables {
        id: id.to_owned(),
    }))
}

fn validate_first_team(key: &str, enabled: bool, url: Option<&LinearUrlRef>) -> Result<(), Error> {
    if let Some(LinearUrlRef::Cycle { team_key, .. }) = url
        && team_key.to_uppercase() != key.to_uppercase()
    {
        return Err(protocol(format!(
            "That cycle URL is for team {team_key}, but this command is working in team {key}."
        ))
        .with_hint(format!("Pass --team {team_key}.")));
    }
    if !enabled {
        return Err(protocol(format!("Cycles are not enabled for team {key}"))
            .with_hint("Enable cycles for the team in Linear's settings before filtering or assigning by cycle."));
    }
    Ok(())
}

/// Fetch every lookup page before choosing a cycle.
pub async fn resolve_id_with<F, Fut>(
    team_id: &str,
    reference: &str,
    url: Option<&LinearUrlRef>,
    mut fetch: F,
) -> Result<String, Error>
where
    F: FnMut(GraphQlRequest<LookupVariables>) -> Fut,
    Fut: Future<Output = Result<GetTeamCyclesForLookup, Error>>,
{
    let first = fetch(lookup_request(team_id, None)).await?;
    let team = first
        .team
        .ok_or_else(|| Error::not_found("Team", team_id))?;
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
        let next_team = next.team.ok_or_else(|| Error::not_found("Team", team_id))?;
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
) -> Result<GetTeamCyclesForLookup, Error> {
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
                // errors. On page one, the URL and cycles-enabled checks come first.
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
    crate::graphql::transport::classify_typed(response).map_err(Error::from)
}

pub async fn resolve_id(
    transport: &GraphQlTransport,
    team_id: &str,
    reference: &str,
    url: Option<&LinearUrlRef>,
) -> Result<String, Error> {
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
                .map_err(Error::from)?;
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
) -> Result<String, Error> {
    let reference = match url {
        Some(LinearUrlRef::Cycle {
            cycle: CycleSelector::Number(number),
            ..
        }) => {
            return cycles
                .iter()
                .find(|cycle| u64::from(cycle.number.0) == *number)
                .map(|cycle| cycle.id.inner().to_owned())
                .ok_or_else(|| Error::not_found("Cycle", &format!("#{number} in team {key}")));
        }
        Some(LinearUrlRef::Cycle {
            cycle: CycleSelector::Active,
            ..
        }) => "active",
        Some(LinearUrlRef::Cycle {
            cycle: CycleSelector::Next,
            ..
        }) => "next",
        Some(_) => return Err(Error::new("expected cycle URL")),
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
                    next.number,
                    next.starts_at.0.chars().take(10).collect::<String>()
                )
            } else {
                SIMPLE_SUGGESTION.to_owned()
            };
            return Err(protocol(format!("Team {key} has no active cycle")).with_hint(suggestion));
        }
        "next" => {
            return cycles
                .iter()
                .find(|cycle| cycle.is_next)
                .map(|cycle| cycle.id.inner().to_owned())
                .ok_or_else(|| {
                    protocol(format!("Team {key} has no upcoming cycle"))
                        .with_hint(SIMPLE_SUGGESTION)
                });
        }
        "previous" => {
            return cycles
                .iter()
                .find(|cycle| cycle.is_previous)
                .map(|cycle| cycle.id.inner().to_owned())
                .ok_or_else(|| {
                    protocol(format!("Team {key} has no previous cycle"))
                        .with_hint(SIMPLE_SUGGESTION)
                });
        }
        _ => {}
    }
    if offset(reference) {
        let magnitude = reference
            .get(1..)
            .and_then(|digits| digits.parse::<u64>().ok());
        let magnitude = magnitude
            .and_then(|value| i64::try_from(value).ok())
            .ok_or_else(|| protocol(format!("Cycle offset {reference} is out of range")))?;
        let active = active.ok_or_else(|| {
            protocol(format!(
                "Cannot resolve relative cycle {reference}: the team has no active cycle"
            ))
            .with_hint("Use 'next', a cycle number, or a cycle name while no cycle is active.")
        })?;
        let signed = if reference.starts_with('-') {
            -magnitude
        } else {
            magnitude
        };
        let target = i64::from(active.number.0).saturating_add(signed);
        return cycles
            .iter()
            .find(|cycle| i64::from(cycle.number.0) == target)
            .map(|cycle| cycle.id.inner().to_owned())
            .ok_or_else(|| Error::not_found("Cycle", &format!("{reference} (cycle {target})")));
    }
    for cycle in cycles {
        if cycle
            .name
            .as_deref()
            .is_some_and(|name| name.to_lowercase() == keyword)
            || cycle.number.to_string() == reference
        {
            return Ok(cycle.id.inner().to_owned());
        }
    }
    Err(Error::not_found("Cycle", reference))
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

fn json(cycle: &DetailCycle) -> Vec<u8> {
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
    let mut bytes = serde_json::to_vec_pretty(&projected).expect("cycle JSON always serializes");
    bytes.push(b'\n');
    bytes
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
