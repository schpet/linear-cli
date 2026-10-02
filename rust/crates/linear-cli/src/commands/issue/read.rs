//! Command-local issue read filters, lookups, pagination and presentation.
use crate::error::Error;
use crate::graphql::operations::number::{Float, WholeNumber};
use crate::graphql::scalars::DateTimeOrDuration;
use crate::graphql::{
    bulk_error,
    envelope::GraphQlRequest,
    operations::issue_read::*,
    transport::{GraphQlTransport, classify_typed},
};
use crate::refs::{ProjectReference, is_linear_uuid, reject_linear_url};
use chrono::{Datelike, FixedOffset, NaiveDate, TimeZone, Utc};
use cynic::QueryBuilder;
use serde::{Serialize, de::DeserializeOwned};
use std::collections::HashSet;
use std::num::NonZeroU32;

pub async fn exchange<T: DeserializeOwned, V: Serialize>(
    transport: &GraphQlTransport,
    request: &GraphQlRequest<V>,
) -> Result<T, Error> {
    let response = transport.send_request(request).await.map_err(Error::from)?;
    if let Some(error) = bulk_error::observe_source_error(&response, request)
        .map_err(bulk_error::BulkExchangeFailure::into_error)?
    {
        return Err(Error::new(error.preferred_message.unwrap_or(error.message)));
    }
    classify_typed(response).map_err(|error| match error {
        crate::graphql::transport::TransportFailure::Response(
            crate::graphql::envelope::ResponseError::UnexpectedShape(error),
        ) => Error::new("Linear returned issue read data with an unexpected shape")
            .with_source(error),
        error => Error::from(error),
    })
}
fn validation(message: impl Into<String>) -> Error {
    Error::new(message)
}
const DATE_SUGGESTION: &str =
    "Use YYYY-MM-DD or ISO 8601 format (e.g. 2024-01-15 or 2024-01-15T09:00:00Z).";
/// ISSUE-READ-DATE-STRICT is invoked at filter construction, after resolver reads.
pub fn date_filter(value: &str, flag: &str) -> Result<DateTimeOrDuration, Error> {
    let bytes = value.as_bytes();
    let date_shape = bytes.len() >= 10
        && bytes.get(4) == Some(&b'-')
        && bytes.get(7) == Some(&b'-')
        && bytes
            .iter()
            .take(10)
            .enumerate()
            .all(|(i, b)| matches!(i, 4 | 7) || b.is_ascii_digit());
    let time_shape = if bytes.len() == 10 {
        true
    } else if bytes.len() >= 20
        && bytes.get(10) == Some(&b'T')
        && bytes.get(13) == Some(&b':')
        && bytes.get(16) == Some(&b':')
    {
        let zone = if bytes.last() == Some(&b'Z') {
            bytes.len() - 1
        } else {
            bytes.len() - 6
        };
        zone >= 19
            && bytes.get(11..19).is_some_and(|v| {
                v.iter()
                    .enumerate()
                    .all(|(i, b)| matches!(i, 2 | 5) || b.is_ascii_digit())
            })
            && (zone == 19
                || (bytes.get(19) == Some(&b'.')
                    && zone > 20
                    && bytes
                        .get(20..zone)
                        .is_some_and(|v| v.iter().all(u8::is_ascii_digit))))
            && (bytes.last() == Some(&b'Z')
                || (matches!(bytes.get(zone), Some(b'+' | b'-'))
                    && bytes.get(zone + 3) == Some(&b':')
                    && bytes.get(zone + 1..).is_some_and(|v| {
                        v.iter()
                            .enumerate()
                            .all(|(i, b)| i == 2 || b.is_ascii_digit())
                    })))
    } else {
        false
    };
    let err = |format: bool| {
        validation(format!(
            "Invalid date{} for {flag}: \"{value}\"",
            if format { " format" } else { "" }
        ))
        .with_hint(DATE_SUGGESTION)
    };
    if !date_shape || !time_shape {
        return Err(err(true));
    }
    let number = |start: usize, end: usize| {
        value
            .get(start..end)
            .and_then(|s| s.parse::<u32>().ok())
            .ok_or_else(|| err(false))
    };
    let year = i32::try_from(number(0, 4)?).map_err(|_| err(false))?;
    let date =
        NaiveDate::from_ymd_opt(year, number(5, 7)?, number(8, 10)?).ok_or_else(|| err(false))?;
    let (hour, minute, second, millis, offset) = if value.len() == 10 {
        (0, 0, 0, 0, 0)
    } else {
        let zone_start = if value.ends_with('Z') {
            value.len() - 1
        } else {
            value.len() - 6
        };
        let fraction = value.get(19..zone_start).ok_or_else(|| err(false))?;
        let mut digits = fraction
            .strip_prefix('.')
            .unwrap_or("")
            .chars()
            .take(3)
            .collect::<String>();
        while digits.len() < 3 {
            digits.push('0');
        }
        let millis = digits.parse::<u32>().map_err(|_| err(false))?;
        let offset = if value.ends_with('Z') {
            0
        } else {
            let h = number(zone_start + 1, zone_start + 3)?;
            let m = number(zone_start + 4, zone_start + 6)?;
            if h > 23 || m > 59 {
                return Err(err(false));
            }
            let seconds = i32::try_from(h * 3600 + m * 60).map_err(|_| err(false))?;
            if value.as_bytes().get(zone_start) == Some(&b'-') {
                -seconds
            } else {
                seconds
            }
        };
        (
            number(11, 13)?,
            number(14, 16)?,
            number(17, 19)?,
            millis,
            offset,
        )
    };
    let local = date
        .and_hms_milli_opt(hour, minute, second, millis)
        .ok_or_else(|| err(false))?;
    let utc = FixedOffset::east_opt(offset)
        .and_then(|offset| offset.from_local_datetime(&local).single())
        .ok_or_else(|| err(false))?
        .with_timezone(&Utc);
    if !(0..=9999).contains(&utc.year()) {
        return Err(err(true));
    }
    Ok(DateTimeOrDuration(format!(
        "{}Z",
        utc.format("%Y-%m-%dT%H:%M:%S%.3f")
    )))
}
pub fn apply_dates(
    filter: &mut IssueFilter,
    created: Option<&str>,
    updated: Option<&str>,
) -> Result<(), Error> {
    if let Some(value) = created.filter(|s| !s.is_empty()) {
        filter.created_at = Some(DateComparator {
            gte: Some(date_filter(value, "--created-after")?),
        });
    }
    if let Some(value) = updated.filter(|s| !s.is_empty()) {
        filter.updated_at = Some(DateComparator {
            gte: Some(date_filter(value, "--updated-after")?),
        });
    }
    Ok(())
}
pub fn team_filter(keys: &[String], mine: bool) -> TeamFilter {
    TeamFilter {
        key: Some(StringComparator {
            eq: if mine { keys.first().cloned() } else { None },
            r#in: (!mine).then(|| keys.to_vec()),
            ..Default::default()
        }),
        ..Default::default()
    }
}
pub fn query_team_filter(keys: &[String]) -> TeamFilter {
    if keys.len() == 1 {
        team_filter(keys, true)
    } else {
        TeamFilter {
            or: Some(
                keys.iter()
                    .map(|k| team_filter(std::slice::from_ref(k), true))
                    .collect(),
            ),
            ..Default::default()
        }
    }
}
pub fn sort_payload(priority: bool) -> Vec<IssueSortInput> {
    let mut sort = vec![IssueSortInput {
        workflow_state: Some(WorkflowStateSort {
            order: Some(PaginationSortOrder::Ascending),
        }),
        ..Default::default()
    }];
    if priority {
        sort.push(IssueSortInput {
            priority: Some(PrioritySort {
                order: Some(PaginationSortOrder::Descending),
                nulls: Some(PaginationNulls::Last),
            }),
            ..Default::default()
        });
    }
    sort.push(IssueSortInput {
        manual: Some(ManualSort {
            order: Some(PaginationSortOrder::Ascending),
            nulls: Some(PaginationNulls::Last),
        }),
        ..Default::default()
    });
    sort
}
pub const STATE_TYPES: [&str; 6] = [
    "triage",
    "backlog",
    "unstarted",
    "started",
    "completed",
    "canceled",
];
pub async fn state_filter(
    transport: &GraphQlTransport,
    values: &[String],
    keys: Option<&[String]>,
) -> Result<Option<WorkflowStateFilter>, Error> {
    if values.is_empty() {
        return Ok(None);
    }
    let mut types = vec![];
    let mut lookups = vec![];
    for value in values {
        reject_linear_url(value, "a workflow state name, type, or ID")?;
        if value.chars().all(char::is_whitespace) {
            return Err(validation("--state value is empty").with_hint(format!(
                "Pass a state type ({}), name, or ID.",
                STATE_TYPES.join(", ")
            )));
        }
        let selected = if STATE_TYPES.contains(&value.as_str()) {
            &mut types
        } else {
            &mut lookups
        };
        if !selected.contains(value) {
            selected.push(value.clone());
        }
    }
    let mut states = vec![];
    let mut after = None;
    if !lookups.is_empty() {
        loop {
            let request = GraphQlRequest::with_variables(GetWorkflowStatesInScope::build(
                GetWorkflowStatesInScopeVariables {
                    filter: keys.map(|keys| WorkflowStateFilter {
                        team: Some(team_filter(keys, false)),
                        ..Default::default()
                    }),
                    first: Some(250),
                    after: after.clone(),
                },
            ));
            let page: GetWorkflowStatesInScope = exchange(transport, &request).await?;
            states.extend(page.workflow_states.nodes);
            if !page.workflow_states.page_info.has_next_page {
                break;
            }
            let next = page.workflow_states.page_info.end_cursor;
            if next.is_none() || next == after {
                return Err(validation(
                    "Linear reported more workflow states but returned no new pagination cursor",
                )
                .with_hint("Retry the command."));
            }
            after = next;
        }
    }
    let mut ids = vec![];
    for value in lookups {
        let matches = states
            .iter()
            .filter(|s| {
                if is_linear_uuid(&value) {
                    s.id.inner().eq_ignore_ascii_case(&value)
                } else {
                    s.name.to_lowercase() == value.to_lowercase()
                }
            })
            .collect::<Vec<_>>();
        if matches.is_empty() {
            let where_text = keys.map_or_else(
                || "any team".to_owned(),
                |k| {
                    format!(
                        "team{} {}",
                        if k.len() == 1 { "" } else { "s" },
                        k.join(", ")
                    )
                },
            );
            states.sort_by(|a, b| {
                crate::platform::collation::compare(&a.team.key, &b.team.key)
                    .then_with(|| crate::platform::collation::compare(&a.name, &b.name))
            });
            let listed = states
                .iter()
                .map(|s| {
                    let name = serde_json::to_string(&s.name).map_err(|error| {
                        Error::new("Could not serialize workflow state name").with_source(error)
                    })?;
                    Ok(format!(
                        "{name} ({})",
                        if keys.is_some_and(|keys| keys.len() == 1) {
                            s.r#type.clone()
                        } else {
                            format!("{}, {}", s.r#type, s.team.key)
                        }
                    ))
                })
                .collect::<Result<Vec<_>, Error>>()?
                .join(", ");
            return Err(Error::not_found(
                "Workflow state",
                &format!("'{value}' in {where_text}"),
            )
            .with_hint(format!(
                "{}State types: {}. Run `linear team states <team>` to list a team's states.",
                if listed.is_empty() {
                    String::new()
                } else {
                    format!("Valid states: {listed}. ")
                },
                STATE_TYPES.join(", ")
            )));
        }
        for state in matches {
            if !ids.contains(&state.id) {
                ids.push(state.id.clone());
            }
        }
    }
    let by_type = (!types.is_empty()).then(|| WorkflowStateFilter {
        r#type: Some(StringComparator {
            r#in: Some(types),
            ..Default::default()
        }),
        ..Default::default()
    });
    let by_id = (!ids.is_empty()).then(|| WorkflowStateFilter {
        id: Some(IDComparator {
            r#in: Some(ids),
            ..Default::default()
        }),
        ..Default::default()
    });
    Ok(Some(match (by_type, by_id) {
        (Some(a), Some(b)) => WorkflowStateFilter {
            or: Some(vec![a, b]),
            ..Default::default()
        },
        (Some(a), None) | (None, Some(a)) => a,
        (None, None) => return Err(validation("--state selection is empty")),
    }))
}
pub fn entity_filters(
    filter: &mut IssueFilter,
    project: Option<String>,
    project_label: Option<&str>,
    cycle: Option<String>,
    milestone: Option<String>,
    labels: &[String],
) {
    filter.project = if let Some(id) = project.filter(|s| !s.is_empty()) {
        Some(NullableProjectFilter {
            id: Some(EntityIdentifierIDComparator {
                eq: Some(cynic::Id::new(id)),
            }),
            ..Default::default()
        })
    } else {
        project_label
            .filter(|s| !s.is_empty())
            .map(|name| NullableProjectFilter {
                labels: Some(ProjectLabelCollectionFilter {
                    name: Some(StringComparator {
                        eq_ignore_case: Some(name.to_owned()),
                        ..Default::default()
                    }),
                }),
                ..Default::default()
            })
    };
    filter.cycle = cycle.map(|id| NullableCycleFilter {
        id: Some(IDComparator {
            eq: Some(cynic::Id::new(id)),
            ..Default::default()
        }),
    });
    filter.project_milestone = milestone.map(|id| NullableProjectMilestoneFilter {
        id: Some(IDComparator {
            eq: Some(cynic::Id::new(id)),
            ..Default::default()
        }),
    });
    let label = |name: &String| IssueLabelCollectionFilter {
        some: Some(IssueLabelFilter {
            name: Some(StringComparator {
                eq_ignore_case: Some(name.clone()),
                ..Default::default()
            }),
        }),
        ..Default::default()
    };
    filter.labels = match labels {
        [] => None,
        [name] => Some(label(name)),
        names => Some(IssueLabelCollectionFilter {
            and: Some(names.iter().map(label).collect()),
            ..Default::default()
        }),
    };
}
pub async fn assignee_filter(
    transport: &GraphQlTransport,
    input: Option<&str>,
    unassigned: bool,
    mine: bool,
) -> Result<Option<NullableUserFilter>, Error> {
    if mine {
        return Ok(Some(NullableUserFilter {
            is_me: Some(BooleanComparator { eq: Some(true) }),
            ..Default::default()
        }));
    }
    if unassigned {
        return Ok(Some(NullableUserFilter {
            r#null: Some(true),
            ..Default::default()
        }));
    }
    let Some(input) = input.filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    reject_linear_url(input, "an email, username, display name, or @me")?;
    use crate::graphql::operations::initiatives::{
        GetViewerId, GetViewerIdVariables, LookupUser, LookupUserVariables,
    };
    let id = if input == "self" || input == "@me" {
        let data: GetViewerId = exchange(
            transport,
            &GraphQlRequest::with_variables(GetViewerId::build(GetViewerIdVariables {})),
        )
        .await?;
        data.viewer.id
    } else {
        let data: LookupUser = exchange(
            transport,
            &GraphQlRequest::with_variables(LookupUser::build(LookupUserVariables {
                input: input.to_owned(),
            })),
        )
        .await?;
        crate::commands::initiative_list::select_owner(&data.users.nodes, input)
            .ok_or_else(|| Error::not_found("User", input))?
    };
    Ok(Some(NullableUserFilter {
        id: Some(IDComparator {
            eq: Some(id),
            ..Default::default()
        }),
        ..Default::default()
    }))
}
fn issue_write_query_ending<V>(
    request: GraphQlRequest<V>,
    _terminal_lf: bool,
) -> GraphQlRequest<V> {
    request
}
pub async fn project_id(
    transport: &GraphQlTransport,
    reference: &ProjectReference,
) -> Result<Option<String>, Error> {
    project_id_query_ending(transport, reference, true).await
}
pub async fn project_id_without_terminal_lf(
    transport: &GraphQlTransport,
    reference: &ProjectReference,
) -> Result<Option<String>, Error> {
    project_id_query_ending(transport, reference, false).await
}
async fn project_id_query_ending(
    transport: &GraphQlTransport,
    reference: &ProjectReference,
    terminal_lf: bool,
) -> Result<Option<String>, Error> {
    use crate::graphql::operations::project_view::{
        GetProjectIdByName, GetProjectIdBySlugId, ProjectReferenceVariables, ProjectSlugVariables,
    };
    let slug = match reference {
        ProjectReference::Id(id) => return Ok(Some(id.clone())),
        ProjectReference::Slug(slug) => slug,
        ProjectReference::NameOrSlug(name) => {
            let data: GetProjectIdByName = exchange(
                transport,
                &issue_write_query_ending(
                    GraphQlRequest::with_variables(GetProjectIdByName::build(
                        ProjectReferenceVariables { name: name.clone() },
                    )),
                    terminal_lf,
                ),
            )
            .await?;
            if data.projects.nodes.len() > 1 {
                return Err(validation(format!(
                    "Project \"{name}\" is ambiguous; it matches {} projects:\n{}",
                    data.projects.nodes.len(),
                    data.projects
                        .nodes
                        .iter()
                        .map(|p| format!("  {}", p.id.inner()))
                        .collect::<Vec<_>>()
                        .join("\n")
                ))
                .with_hint(
                    "Pass the project's UUID or slug ID instead. `linear project list` shows both.",
                ));
            }
            if let Some(project) = data
                .projects
                .nodes
                .into_iter()
                .next()
                .filter(|project| !project.id.inner().is_empty())
            {
                return Ok(Some(project.id.into_inner()));
            }
            name
        }
    };
    let data: GetProjectIdBySlugId = exchange(
        transport,
        &issue_write_query_ending(
            GraphQlRequest::with_variables(GetProjectIdBySlugId::build(ProjectSlugVariables {
                slug_id: slug.clone(),
            })),
            terminal_lf,
        ),
    )
    .await?;
    Ok(data
        .projects
        .nodes
        .into_iter()
        .next()
        .map(|p| p.id.into_inner()))
}
pub async fn milestone_id(
    transport: &GraphQlTransport,
    value: &str,
    project: Option<&str>,
) -> Result<String, Error> {
    milestone_id_query_ending(transport, value, project, true).await
}
pub async fn milestone_id_without_terminal_lf(
    transport: &GraphQlTransport,
    value: &str,
    project: Option<&str>,
) -> Result<String, Error> {
    milestone_id_query_ending(transport, value, project, false).await
}
async fn milestone_id_query_ending(
    transport: &GraphQlTransport,
    value: &str,
    project: Option<&str>,
    terminal_lf: bool,
) -> Result<String, Error> {
    if is_linear_uuid(value) {
        return Ok(value.to_owned());
    }
    reject_linear_url(value, "a milestone name or UUID")?;
    let project = project.ok_or_else(|| validation(format!("Cannot resolve milestone \"{value}\" without --project")).with_hint("Pass a milestone UUID, or specify --project so the milestone name can be looked up within that project."))?;
    use crate::graphql::operations::milestone_view::{
        GetProjectMilestonesForLookup, LookupVariables,
    };
    let result: GetProjectMilestonesForLookup = exchange(
        transport,
        &issue_write_query_ending(
            GraphQlRequest::with_variables(GetProjectMilestonesForLookup::build(LookupVariables {
                project_id: project.to_owned(),
            })),
            terminal_lf,
        ),
    )
    .await?;
    let rows = result
        .project
        .ok_or_else(|| Error::not_found("Project", project))?
        .project_milestones
        .map(|p| p.nodes)
        .unwrap_or_default();
    rows.into_iter()
        .find(|m| m.name.to_lowercase() == value.to_lowercase())
        .map(|m| m.id.into_inner())
        .ok_or_else(|| Error::not_found("Milestone", value))
}
/// The page size to request: what is still wanted, capped at Linear's maximum of 100.
fn page_size(limit: Option<NonZeroU32>, fetched: usize, unlimited: i32) -> i32 {
    match limit {
        None => unlimited,
        Some(limit) => {
            let remaining =
                u64::from(limit.get()).saturating_sub(u64::try_from(fetched).unwrap_or(u64::MAX));
            i32::try_from(remaining.min(100)).unwrap_or(100)
        }
    }
}
fn reached(limit: Option<NonZeroU32>, fetched: usize) -> bool {
    limit.is_some_and(|limit| usize::try_from(limit.get()).is_ok_and(|limit| fetched >= limit))
}
fn truncate_to<T>(rows: &mut Vec<T>, limit: Option<NonZeroU32>) {
    if let Some(limit) = limit {
        rows.truncate(usize::try_from(limit.get()).unwrap_or(usize::MAX));
    }
}
fn next_cursor(next: Option<String>, seen: &mut HashSet<String>) -> Result<String, Error> {
    let next = next.ok_or_else(|| {
        validation("Linear reported more issues but returned no pagination cursor")
    })?;
    if !seen.insert(next.clone()) {
        return Err(validation("Linear repeated an issue pagination cursor"));
    }
    Ok(next)
}
pub async fn mine(
    transport: &GraphQlTransport,
    filter: IssueFilter,
    priority: bool,
    limit: Option<NonZeroU32>,
) -> Result<Vec<GetIssuesForStateIssuesNodes>, Error> {
    mine_with_requests(transport, filter, priority, limit, |variables| {
        GraphQlRequest::with_variables(GetIssuesForState::build(variables))
    })
    .await
}
/// `mine` with a caller-built request, used by `issue start`.
pub(crate) async fn mine_with_requests(
    transport: &GraphQlTransport,
    filter: IssueFilter,
    priority: bool,
    limit: Option<NonZeroU32>,
    mut request: impl FnMut(GetIssuesForStateVariables) -> GraphQlRequest<GetIssuesForStateVariables>,
) -> Result<Vec<GetIssuesForStateIssuesNodes>, Error> {
    let page_size = page_size(limit, 0, 50);
    let mut after = None;
    let mut seen = HashSet::new();
    let mut rows = vec![];
    loop {
        let data: GetIssuesForState = exchange(
            transport,
            &request(GetIssuesForStateVariables {
                sort: Some(sort_payload(priority)),
                filter: filter.clone(),
                first: Some(page_size),
                after: after.clone(),
            }),
        )
        .await?;
        rows.extend(data.issues.nodes);
        if reached(limit, rows.len()) {
            break;
        }
        if !data.issues.page_info.has_next_page {
            break;
        }
        after = Some(next_cursor(data.issues.page_info.end_cursor, &mut seen)?);
    }
    truncate_to(&mut rows, limit);
    sort_mine(&mut rows);
    Ok(rows)
}
pub async fn query(
    transport: &GraphQlTransport,
    filter: Option<IssueFilter>,
    priority: bool,
    limit: Option<NonZeroU32>,
    archived: bool,
) -> Result<GetIssuesForQueryIssues, Error> {
    let size = page_size(limit, 0, 100);
    let mut after = None;
    let mut seen = HashSet::new();
    let mut rows = vec![];
    let info = loop {
        let data: GetIssuesForQuery = exchange(
            transport,
            &GraphQlRequest::with_variables(GetIssuesForQuery::build(GetIssuesForQueryVariables {
                sort: Some(sort_payload(priority)),
                filter: filter.clone(),
                first: Some(size),
                after: after.clone(),
                include_archived: archived.then_some(true),
            })),
        )
        .await?;
        rows.extend(data.issues.nodes);
        let info = data.issues.page_info;
        if reached(limit, rows.len()) || !info.has_next_page {
            break info;
        }
        after = Some(next_cursor(info.end_cursor, &mut seen)?);
    };
    truncate_to(&mut rows, limit);
    sort_query(&mut rows);
    Ok(GetIssuesForQueryIssues {
        nodes: rows,
        page_info: info,
    })
}
pub async fn search(
    transport: &GraphQlTransport,
    filter: Option<IssueFilter>,
    term: String,
    limit: Option<NonZeroU32>,
    archived: bool,
    comments: bool,
) -> Result<SearchIssuesSearchIssues, Error> {
    let mut after = None;
    let mut seen = HashSet::new();
    let mut rows = vec![];
    let (info, total) = loop {
        let data: SearchIssues = exchange(
            transport,
            &GraphQlRequest::with_variables(SearchIssues::build(SearchIssuesVariables {
                term: term.clone(),
                filter: filter.clone(),
                first: Some(page_size(limit, rows.len(), 100)),
                after: after.clone(),
                include_archived: archived.then_some(true),
                include_comments: comments.then_some(true),
                order_by: None,
            })),
        )
        .await?;
        rows.extend(data.search_issues.nodes);
        let info = data.search_issues.page_info;
        if reached(limit, rows.len()) || !info.has_next_page {
            break (info, data.search_issues.total_count);
        }
        after = Some(next_cursor(info.end_cursor, &mut seen)?);
    };
    Ok(SearchIssuesSearchIssues {
        nodes: rows,
        page_info: info,
        total_count: total,
    })
}
fn state_rank(value: &str) -> usize {
    [
        "triage",
        "started",
        "unstarted",
        "backlog",
        "completed",
        "canceled",
        "duplicate",
    ]
    .iter()
    .position(|v| *v == value)
    .unwrap_or(7)
}
fn type_order(a: &str, b: &str) -> std::cmp::Ordering {
    state_rank(a).cmp(&state_rank(b)).then_with(|| {
        if state_rank(a) == 7 {
            crate::platform::collation::compare(a, b)
        } else {
            std::cmp::Ordering::Equal
        }
    })
}
pub fn sort_mine(rows: &mut [GetIssuesForStateIssuesNodes]) {
    let multi = rows
        .first()
        .is_some_and(|first| rows.iter().any(|r| r.team.key != first.team.key));
    rows.sort_by(|a, b| {
        type_order(&a.state.r#type, &b.state.r#type).then_with(|| {
            if multi {
                std::cmp::Ordering::Equal
            } else {
                b.state.position.get().total_cmp(&a.state.position.get())
            }
        })
    });
}
pub fn sort_query(rows: &mut [GetIssuesForQueryIssuesNodes]) {
    let multi = rows
        .first()
        .is_some_and(|first| rows.iter().any(|r| r.team.key != first.team.key));
    rows.sort_by(|a, b| {
        type_order(&a.state.r#type, &b.state.r#type).then_with(|| {
            if multi {
                std::cmp::Ordering::Equal
            } else {
                b.state.position.get().total_cmp(&a.state.position.get())
            }
        })
    });
}
pub fn priority(value: WholeNumber) -> String {
    match value.0 {
        0 => "---".to_owned(),
        1 => "⚠⚠⚠".to_owned(),
        2 => "▄▆█".to_owned(),
        3 => "▄▆ ".to_owned(),
        4 => "▄  ".to_owned(),
        n => n.to_string(),
    }
}
pub fn cycle_short(
    cycle: Option<&GetIssuesForStateIssuesNodesCycle>,
    anchor: Option<WholeNumber>,
) -> (String, &'static str) {
    let Some(c) = cycle else {
        return ("-".to_owned(), "none");
    };
    if c.is_active {
        return ("now".to_owned(), "active");
    }
    if c.is_next {
        return ("+1".to_owned(), "future");
    }
    if c.is_previous {
        return ("-1".to_owned(), "past");
    }
    if let Some(anchor) = anchor {
        let offset = i64::from(c.number.0) - i64::from(anchor.0);
        return if offset == 0 {
            ("now".to_owned(), "active")
        } else {
            (
                format!("{}{offset}", if offset > 0 { "+" } else { "" }),
                if offset > 0 { "future" } else { "past" },
            )
        };
    }
    (
        format!("#{}", c.number),
        if c.is_past { "past" } else { "future" },
    )
}
#[derive(Clone, Debug)]
pub struct TableRow {
    pub identifier: String,
    pub title: String,
    pub priority: WholeNumber,
    pub estimate: Option<Float>,
    pub initials: Option<String>,
    pub state_name: String,
    pub state_color: String,
    pub cycle: Option<GetIssuesForStateIssuesNodesCycle>,
    pub team: String,
    pub cycles_enabled: bool,
    pub anchor: Option<WholeNumber>,
    pub labels: Vec<GetIssuesForStateIssuesNodesLabelsNodes>,
    pub blocked: bool,
    pub updated: String,
}
impl From<GetIssuesForStateIssuesNodes> for TableRow {
    fn from(r: GetIssuesForStateIssuesNodes) -> Self {
        Self {
            identifier: r.identifier,
            title: r.title,
            priority: r.priority,
            estimate: r.estimate,
            initials: r.assignee.map(|a| a.initials),
            state_name: r.state.name,
            state_color: r.state.color,
            cycle: r.cycle,
            team: r.team.key,
            cycles_enabled: r.team.cycles_enabled,
            anchor: r.team.active_cycle.map(|c| c.number),
            labels: r.labels.nodes,
            blocked: r.inverse_relations.nodes.iter().any(|x| {
                x.r#type == "blocks"
                    && !matches!(x.issue.state.r#type.as_str(), "completed" | "canceled")
            }),
            updated: r.updated_at.0,
        }
    }
}
impl From<GetIssuesForQueryIssuesNodes> for TableRow {
    fn from(r: GetIssuesForQueryIssuesNodes) -> Self {
        Self {
            identifier: r.identifier,
            title: r.title,
            priority: r.priority,
            estimate: r.estimate,
            initials: r.assignee.map(|a| a.initials),
            state_name: r.state.name,
            state_color: r.state.color,
            cycle: r.cycle,
            team: r.team.key,
            cycles_enabled: r.team.cycles_enabled,
            anchor: r.team.active_cycle.map(|c| c.number),
            labels: r.labels.nodes,
            blocked: r.inverse_relations.nodes.iter().any(|x| {
                x.r#type == "blocks"
                    && !matches!(x.issue.state.r#type.as_str(), "completed" | "canceled")
            }),
            updated: r.updated_at.0,
        }
    }
}
impl From<SearchIssuesSearchIssuesNodes> for TableRow {
    fn from(r: SearchIssuesSearchIssuesNodes) -> Self {
        Self {
            identifier: r.identifier,
            title: r.title,
            priority: r.priority,
            estimate: r.estimate,
            initials: r.assignee.map(|a| a.initials),
            state_name: r.state.name,
            state_color: r.state.color,
            cycle: r.cycle,
            team: r.team.key,
            cycles_enabled: r.team.cycles_enabled,
            anchor: r.team.active_cycle.map(|c| c.number),
            labels: r.labels.nodes,
            blocked: r.inverse_relations.nodes.iter().any(|x| {
                x.r#type == "blocks"
                    && !matches!(x.issue.state.r#type.as_str(), "completed" | "canceled")
            }),
            updated: r.updated_at.0,
        }
    }
}
fn style(text: &str, code: &str, color: bool) -> String {
    if color {
        format!(
            "\x1b[{code}m{text}\x1b[{}m",
            if code == "1" {
                "22"
            } else if code == "4" {
                "24"
            } else {
                "39"
            }
        )
    } else {
        text.to_owned()
    }
}
fn rgb(text: &str, color: &str, enabled: bool) -> String {
    if enabled {
        let start = crate::commands::table::terminal_color(color)
            .unwrap_or_else(|| "\x1b[38;2;0;0;0m".to_owned());
        format!("{start}{text}\x1b[39m")
    } else {
        text.to_owned()
    }
}
pub fn table(
    rows: &[TableRow],
    mine: bool,
    team: bool,
    assignee: bool,
    columns: usize,
    color: bool,
    now: std::time::SystemTime,
) -> Result<String, Error> {
    use crate::commands::display::{display_width, pad, truncate_text};
    if rows.is_empty() {
        return Ok("No issues found.".to_owned());
    }
    let id = rows
        .iter()
        .map(|r| display_width(&r.identifier))
        .max()
        .unwrap_or(2)
        .max(2);
    let tw = if team {
        rows.iter()
            .map(|r| display_width(&r.team))
            .max()
            .unwrap_or(4)
            .max(4)
    } else {
        0
    };
    let lw = rows
        .iter()
        .map(|r| {
            display_width(
                &r.labels
                    .iter()
                    .map(|l| l.name.clone())
                    .collect::<Vec<_>>()
                    .join(", "),
            )
        })
        .max()
        .unwrap_or(6)
        .clamp(6, 25);
    let show_cycle = rows.iter().any(|r| r.cycle.is_some() || r.cycles_enabled);
    let cycles = rows
        .iter()
        .map(|r| cycle_short(r.cycle.as_ref(), r.anchor))
        .collect::<Vec<_>>();
    let cw = if show_cycle {
        cycles
            .iter()
            .map(|c| display_width(&c.0))
            .max()
            .unwrap_or(3)
            .max(3)
    } else {
        0
    };
    let aw = if assignee { 2 } else { 0 };
    let sw = rows
        .iter()
        .map(|r| display_width(&r.state_name))
        .max()
        .unwrap_or(5)
        .clamp(5, 20);
    let times = rows
        .iter()
        .map(|r| {
            crate::commands::relative_time::format_relative_time(
                &r.updated,
                now.into(),
                &chrono::Local,
            )
        })
        .collect::<Vec<_>>();
    let uw = times
        .iter()
        .map(|t| display_width(t))
        .max()
        .unwrap_or(7)
        .max(7);
    let fixed_cells = 7 + usize::from(team) + usize::from(show_cycle) + usize::from(assignee);
    let fixed = 3
        + id
        + tw
        + lw
        + 1
        + 1
        + cw
        + aw
        + sw
        + uw
        + if mine {
            9 + usize::from(show_cycle)
        } else {
            fixed_cells + 1
        };
    let title = rows
        .iter()
        .map(|r| display_width(&r.title))
        .max()
        .unwrap_or(0)
        .min(columns.saturating_sub(fixed))
        .max(if mine { 0 } else { 10 });
    let mut headers = vec![pad("◌", 3), pad("ID", id)];
    if team {
        headers.push(pad("TEAM", tw));
    }
    headers.extend([
        pad("TITLE", title),
        pad("LABELS", lw),
        "B".to_owned(),
        "E".to_owned(),
    ]);
    if show_cycle {
        headers.push(pad("CYC", cw));
    }
    if assignee {
        headers.push("A ".to_owned());
    }
    headers.extend([pad("STATE", sw), pad("UPDATED", uw)]);
    let mut lines = vec![style(&style(&headers.join(" "), "4", color), "1", color)];
    for (i, r) in rows.iter().enumerate() {
        let mut cells = vec![pad(&priority(r.priority), 3), pad(&r.identifier, id)];
        if team {
            cells.push(pad(&r.team, tw));
        }
        cells.push(pad(&truncate_text(&r.title, title), title));
        let mut label = String::new();
        let mut used = 0;
        for (index, l) in r.labels.iter().enumerate() {
            let sep = if index == 0 { "" } else { ", " };
            let width = display_width(sep) + display_width(&l.name);
            if used + width > lw {
                let remaining = lw - used;
                if remaining >= 4 {
                    let truncated = truncate_text(&l.name, remaining.saturating_sub(sep.len()));
                    label.push_str(sep);
                    label.push_str(&rgb(&truncated, &l.color, color));
                    used += sep.len() + display_width(&truncated);
                }
                break;
            }
            label.push_str(sep);
            label.push_str(&rgb(&l.name, &l.color, color));
            used += width;
        }
        label.push_str(&" ".repeat(lw.saturating_sub(used)));
        cells.push(label);
        cells.push(if r.blocked {
            style("⊘", "33", color)
        } else {
            " ".to_owned()
        });
        cells.push(
            r.estimate
                .as_ref()
                .map_or_else(|| "-".to_owned(), ToString::to_string),
        );
        if show_cycle {
            let (c, kind) = cycles
                .get(i)
                .ok_or_else(|| Error::new("Missing cycle display"))?;
            cells.push(format!(
                "{}{}",
                match *kind {
                    "active" => style(c, "32", color),
                    "past" | "none" => style(c, "90", color),
                    "future" => c.clone(),
                    _ => return Err(Error::new("Unknown cycle display kind")),
                },
                " ".repeat(cw.saturating_sub(display_width(c)))
            ));
        }
        if assignee {
            let initials = r
                .initials
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or("-");
            let initial = initials.chars().take(2).collect::<String>();
            cells.push(pad(&initial, aw));
        }
        let state = truncate_text(&r.state_name, sw);
        cells.push(format!(
            "{}{}",
            rgb(&state, &r.state_color, color),
            " ".repeat(sw.saturating_sub(display_width(&state)))
        ));
        cells.push(style(
            &pad(
                times
                    .get(i)
                    .ok_or_else(|| Error::new("Missing time display"))?,
                uw,
            ),
            "90",
            color,
        ));
        lines.push(cells.join(" "));
    }
    Ok(lines.join("\n"))
}

/// Refuse menu text the selector cannot display (control characters).
pub fn project_menu_text(message: &str, labels: &[&str]) -> Result<(), Error> {
    if std::iter::once(message)
        .chain(labels.iter().copied())
        .any(|s| s.trim().is_empty() || s.chars().any(char::is_control))
    {
        return Err(validation(
            "Project menu text must be nonempty and contain no control characters",
        )
        .with_hint("Use an exact project name or UUID to avoid selecting similar projects."));
    }
    Ok(())
}

/// The team `reference` (a key, name, ID or URL) names.
pub(super) fn resolve_team(
    ctx: &crate::ctx::Ctx,
    client: &GraphQlTransport,
    reference: &str,
) -> Result<crate::refs::ResolvedTeam, Error> {
    let lookup = crate::refs::prepare_team_lookup(reference, &ctx.scope()?)?;
    ctx.block_on(crate::refs::resolve_team(
        &lookup,
        |request| async move { exchange(client, &request).await },
        |request| async move { exchange(client, &request).await },
    ))
}

/// The project `--project` names. When no project matches exactly, a terminal
/// user may pick one of the similarly named projects.
pub(super) fn resolve_project(
    ctx: &crate::ctx::Ctx,
    client: &GraphQlTransport,
    value: Option<&str>,
) -> Result<Option<String>, Error> {
    use crate::platform::prompt::{PlainOption, PlainSelect, PromptOutcome};
    let Some(value) = value else { return Ok(None) };
    let reference = crate::refs::prepare_project_lookup(value, &ctx.scope()?)?;
    if let Some(id) = ctx.block_on(project_id(client, &reference))? {
        return Ok(Some(id));
    }
    let data: GetProjectIdOptionsByName = ctx.block_on(exchange(
        client,
        &GraphQlRequest::with_variables(GetProjectIdOptionsByName::build(
            GetProjectIdOptionsByNameVariables {
                name: value.to_owned(),
            },
        )),
    ))?;
    let mut rows: Vec<(String, String)> = vec![];
    for row in data.projects.nodes {
        match rows.iter_mut().find(|(id, _)| id == row.id.inner()) {
            Some(existing) => existing.1 = row.name,
            None => rows.push((row.id.into_inner(), row.name)),
        }
    }
    let names = || {
        rows.iter()
            .map(|(_, name)| name.as_str())
            .collect::<Vec<_>>()
    };
    match rows.as_slice() {
        [] => return Err(Error::not_found("Project", value)),
        _ if !ctx.stdin_tty() => {
            return Err(Error::new(format!(
                "Project \"{value}\" not found. Similar projects: {}",
                names().join(", ")
            )));
        }
        _ => {}
    }
    let single = rows.len() == 1;
    let message = match rows.as_slice() {
        [(_, name)] => format!(
            "Project named {value} does not exist, but {name} exists. Is this what you meant?"
        ),
        _ => format!(
            "Project with {value} does not exist, but the following exist. Is any of these what you meant?"
        ),
    };
    let mut options = rows
        .iter()
        .enumerate()
        .map(|(index, (_, name))| PlainOption {
            label: if single {
                "yes".to_owned()
            } else {
                name.clone()
            },
            value: index.to_string(),
            script_token: index.to_string(),
        })
        .collect::<Vec<_>>();
    options.push(PlainOption {
        label: if single { "no" } else { "none of the above" }.to_owned(),
        value: "none".to_owned(),
        script_token: "none".to_owned(),
    });
    project_menu_text(
        &message,
        &options
            .iter()
            .map(|option| option.label.as_str())
            .collect::<Vec<_>>(),
    )?;
    let mut session = ctx.prompts()?;
    let selected = session.select(&PlainSelect {
        message: &message,
        options: &options,
        default_index: 0,
        default_hint: None,
    });
    match session.finish_result(selected)? {
        PromptOutcome::Submitted(selected) if selected == "none" => Ok(None),
        PromptOutcome::Submitted(selected) => {
            let index: usize = selected.parse().expect("menu values are row indexes");
            Ok(Some(
                rows.get(index)
                    .expect("menu values are row indexes")
                    .0
                    .clone(),
            ))
        }
        PromptOutcome::Interrupted => Err(Error::cancelled()),
        PromptOutcome::EndOfInput => Err(Error::new("unexpected EOF while selecting project")),
    }
}

/// The cycle `--cycle` names in the one team in scope.
pub(super) fn resolve_cycle(
    ctx: &crate::ctx::Ctx,
    client: &GraphQlTransport,
    value: Option<&str>,
    team_key: Option<&str>,
    team_id: Option<&str>,
) -> Result<Option<String>, Error> {
    let Some(value) = value else { return Ok(None) };
    let team_id = match team_id {
        Some(id) => id.to_owned(),
        None => {
            let key = team_key.ok_or_else(|| Error::new("--cycle requires a single team scope"))?;
            resolve_team(ctx, client, key)?.id
        }
    };
    let url = crate::refs::expect_url_kind(
        value,
        crate::refs::LinearUrlKind::Cycle,
        "a cycle URL, number, or name",
        &ctx.scope()?,
    )?;
    ctx.block_on(crate::commands::cycle::view::resolve_id_with(
        &team_id,
        value,
        url.as_ref(),
        |request| async move { exchange(client, &request).await },
    ))
    .map(Some)
}

/// Whether issues sort by priority: `--sort`, else the configured sort.
pub(super) fn priority_sort(ctx: &crate::ctx::Ctx, sort: Option<crate::cli::Sort>) -> bool {
    use crate::config::IssueSort;
    let value = sort.map(|value| match value {
        crate::cli::Sort::Manual => IssueSort::Manual,
        crate::cli::Sort::Priority => IssueSort::Priority,
    });
    ctx.options().issue_sort(value).0 == IssueSort::Priority
}

/// Prints an issue table, through the pager on a terminal.
pub(super) fn print_table(ctx: &crate::ctx::Ctx, table: &str, paging: bool) -> Result<(), Error> {
    if ctx.stdout_tty() {
        ctx.page(table, paging)
    } else {
        ctx.print(format!("{table}\n"))
    }
}

/// The table width: the terminal's, or a fixed width when piped.
pub(super) fn table_columns(ctx: &crate::ctx::Ctx) -> usize {
    if ctx.stdout_tty() {
        crate::platform::pager::stdout_size().map_or(80, |size| usize::from(size.columns))
    } else {
        120
    }
}
