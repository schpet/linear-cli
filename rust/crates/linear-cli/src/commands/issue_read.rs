//! Command-local issue read filters, lookups, pagination and presentation.
use crate::error::{AppError, AppErrorKind};
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

pub async fn exchange<T: DeserializeOwned, V: Serialize>(
    transport: &GraphQlTransport,
    request: &GraphQlRequest<V>,
) -> Result<T, AppError> {
    let response = transport
        .send_request(request)
        .await
        .map_err(AppError::from)?;
    if let Some(error) = bulk_error::observe_source_error(&response, request)
        .map_err(bulk_error::BulkExchangeFailure::into_error)?
    {
        return Err(AppError::new(
            AppErrorKind::GraphQl,
            error.preferred_message.unwrap_or(error.message),
        ));
    }
    classify_typed(response).map_err(|error| match error {
        crate::graphql::transport::TransportFailure::Response(
            crate::graphql::envelope::ResponseError::UnexpectedShape(error),
        ) => AppError::new(
            AppErrorKind::GraphQl,
            "Linear returned issue read data with an unexpected shape",
        )
        .with_source(error),
        error => AppError::from(error),
    })
}
fn validation(message: impl Into<String>) -> AppError {
    AppError::new(AppErrorKind::Validation, message)
}
const DATE_SUGGESTION: &str =
    "Use YYYY-MM-DD or ISO 8601 format (e.g. 2024-01-15 or 2024-01-15T09:00:00Z).";
/// ISSUE-READ-DATE-STRICT is invoked at filter construction, after resolver reads.
pub fn date_filter(value: &str, flag: &str) -> Result<DateTimeOrDuration, AppError> {
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
        .with_suggestion(DATE_SUGGESTION)
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
) -> Result<(), AppError> {
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
) -> Result<Option<WorkflowStateFilter>, AppError> {
    if values.is_empty() {
        return Ok(None);
    }
    let mut types = vec![];
    let mut lookups = vec![];
    for value in values {
        reject_linear_url(value, "a workflow state name, type, or ID")?;
        if value.chars().all(crate::text::js_space) {
            return Err(
                validation("--state value is empty").with_suggestion(format!(
                    "Pass a state type ({}), name, or ID.",
                    STATE_TYPES.join(", ")
                )),
            );
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
                .with_suggestion("Retry the command."));
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
            let collator = crate::platform::collation::root()?;
            states.sort_by(|a, b| {
                collator
                    .compare(&a.team.key, &b.team.key)
                    .then_with(|| collator.compare(&a.name, &b.name))
            });
            let listed = states
                .iter()
                .map(|s| {
                    let name = serde_json::to_string(&s.name).map_err(|error| {
                        AppError::new(
                            AppErrorKind::Invariant,
                            "Could not serialize workflow state name",
                        )
                        .with_source(error)
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
                .collect::<Result<Vec<_>, AppError>>()?
                .join(", ");
            return Err(AppError::not_found(
                "Workflow state",
                &format!("'{value}' in {where_text}"),
            )
            .with_suggestion(format!(
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
) -> Result<Option<NullableUserFilter>, AppError> {
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
            .ok_or_else(|| AppError::not_found("User", input))?
    };
    Ok(Some(NullableUserFilter {
        id: Some(IDComparator {
            eq: Some(id),
            ..Default::default()
        }),
        ..Default::default()
    }))
}
pub async fn project_id(
    transport: &GraphQlTransport,
    reference: &ProjectReference,
) -> Result<Option<String>, AppError> {
    use crate::graphql::operations::project_view::{
        GetProjectIdByName, GetProjectIdBySlugId, ProjectReferenceVariables, ProjectSlugVariables,
    };
    let slug = match reference {
        ProjectReference::Id(id) => return Ok(Some(id.clone())),
        ProjectReference::Slug(slug) => slug,
        ProjectReference::NameOrSlug(name) => {
            let data: GetProjectIdByName = exchange(
                transport,
                &GraphQlRequest::with_variables(GetProjectIdByName::build(
                    ProjectReferenceVariables { name: name.clone() },
                )),
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
                .with_suggestion(
                    "Pass the project's UUID or slug ID instead. `linear project list` shows both.",
                ));
            }
            if let Some(project) = data.projects.nodes.into_iter().next() {
                return Ok(Some(project.id.into_inner()));
            }
            name
        }
    };
    let data: GetProjectIdBySlugId = exchange(
        transport,
        &GraphQlRequest::with_variables(GetProjectIdBySlugId::build(ProjectSlugVariables {
            slug_id: slug.clone(),
        })),
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
) -> Result<String, AppError> {
    if is_linear_uuid(value) {
        return Ok(value.to_owned());
    }
    reject_linear_url(value, "a milestone name or UUID")?;
    let project = project.ok_or_else(|| validation(format!("Cannot resolve milestone \"{value}\" without --project")).with_suggestion("Pass a milestone UUID, or specify --project so the milestone name can be looked up within that project."))?;
    use crate::graphql::operations::milestone_view::{
        GetProjectMilestonesForLookup, LookupVariables,
    };
    let result: GetProjectMilestonesForLookup = exchange(
        transport,
        &GraphQlRequest::with_variables(GetProjectMilestonesForLookup::build(LookupVariables {
            project_id: project.to_owned(),
        })),
    )
    .await?;
    let rows = result
        .project
        .ok_or_else(|| AppError::not_found("Project", project))?
        .project_milestones
        .map(|p| p.nodes)
        .unwrap_or_default();
    rows.into_iter()
        .find(|m| m.name.to_lowercase() == value.to_lowercase())
        .map(|m| m.id.into_inner())
        .ok_or_else(|| AppError::not_found("Milestone", value))
}
fn first(value: f64) -> Result<i32, AppError> {
    if value.fract() != 0.0 {
        return Err(validation("Issue page size must be a GraphQL integer"));
    }
    value.to_string().parse().map_err(|e| {
        validation("Issue page size is outside the GraphQL integer range").with_source(e)
    })
}
fn next_cursor(next: Option<String>, seen: &mut HashSet<String>) -> Result<String, AppError> {
    let next = next.ok_or_else(|| {
        validation("Linear reported more issues but returned no pagination cursor")
    })?;
    if !seen.insert(next.clone()) {
        return Err(validation("Linear repeated an issue pagination cursor"));
    }
    Ok(next)
}
fn length_number(length: usize) -> Result<f64, AppError> {
    length.to_string().parse().map_err(|e| {
        AppError::new(AppErrorKind::Invariant, "Could not represent issue count").with_source(e)
    })
}
fn slice<T>(rows: &mut Vec<T>, limit: f64) -> Result<(), AppError> {
    if limit == 0.0 {
        return Ok(());
    }
    let len = length_number(rows.len())?;
    let cutoff = if limit < 0.0 {
        (len + limit).max(0.0)
    } else {
        limit.min(len)
    }
    .trunc();
    let cutoff = cutoff.to_string().parse::<usize>().map_err(|e| {
        AppError::new(
            AppErrorKind::Invariant,
            "Could not represent issue slice length",
        )
        .with_source(e)
    })?;
    rows.truncate(cutoff);
    Ok(())
}
pub async fn mine(
    transport: &GraphQlTransport,
    filter: IssueFilter,
    priority: bool,
    limit: f64,
) -> Result<Vec<GetIssuesForStateIssuesNodes>, AppError> {
    let page_size = first(if limit == 0.0 { 50.0 } else { limit.min(100.0) })?;
    let mut after = None;
    let mut seen = HashSet::new();
    let mut rows = vec![];
    loop {
        let data: GetIssuesForState = exchange(
            transport,
            &GraphQlRequest::with_variables(GetIssuesForState::build(GetIssuesForStateVariables {
                sort: Some(sort_payload(priority)),
                filter: filter.clone(),
                first: Some(page_size),
                after: after.clone(),
            })),
        )
        .await?;
        rows.extend(data.issues.nodes);
        if limit != 0.0 && length_number(rows.len())? >= limit {
            break;
        }
        if !data.issues.page_info.has_next_page {
            break;
        }
        after = Some(next_cursor(data.issues.page_info.end_cursor, &mut seen)?);
    }
    slice(&mut rows, limit)?;
    sort_mine(&mut rows)?;
    Ok(rows)
}
pub async fn query(
    transport: &GraphQlTransport,
    filter: Option<IssueFilter>,
    priority: bool,
    limit: f64,
    archived: bool,
) -> Result<GetIssuesForQueryIssues, AppError> {
    let size = first(if limit == 0.0 {
        100.0
    } else {
        limit.min(100.0)
    })?;
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
        if (limit != 0.0 && length_number(rows.len())? >= limit) || !info.has_next_page {
            break info;
        }
        after = Some(next_cursor(info.end_cursor, &mut seen)?);
    };
    slice(&mut rows, limit)?;
    sort_query(&mut rows)?;
    Ok(GetIssuesForQueryIssues {
        nodes: rows,
        page_info: info,
    })
}
pub async fn search(
    transport: &GraphQlTransport,
    filter: Option<IssueFilter>,
    term: String,
    limit: f64,
    archived: bool,
    comments: bool,
) -> Result<SearchIssuesSearchIssues, AppError> {
    let mut after = None;
    let mut seen = HashSet::new();
    let mut rows = vec![];
    let (info, total) = loop {
        let remaining = if limit == 0.0 {
            100.0
        } else {
            (limit - length_number(rows.len())?).min(100.0)
        };
        let data: SearchIssues = exchange(
            transport,
            &GraphQlRequest::with_variables(SearchIssues::build(SearchIssuesVariables {
                term: term.clone(),
                filter: filter.clone(),
                first: Some(first(remaining)?),
                after: after.clone(),
                include_archived: archived.then_some(true),
                include_comments: comments.then_some(true),
                order_by: None,
            })),
        )
        .await?;
        rows.extend(data.search_issues.nodes);
        let info = data.search_issues.page_info;
        if (limit != 0.0 && length_number(rows.len())? >= limit) || !info.has_next_page {
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
fn type_order(
    a: &str,
    b: &str,
    collator: &icu_collator::CollatorBorrowed<'_>,
) -> std::cmp::Ordering {
    state_rank(a).cmp(&state_rank(b)).then_with(|| {
        if state_rank(a) == 7 {
            collator.compare(a, b)
        } else {
            std::cmp::Ordering::Equal
        }
    })
}
pub fn sort_mine(rows: &mut [GetIssuesForStateIssuesNodes]) -> Result<(), AppError> {
    let multi = rows
        .first()
        .is_some_and(|first| rows.iter().any(|r| r.team.key != first.team.key));
    let collator = crate::platform::collation::root()?;
    if rows.iter().any(|r| !r.state.position.is_finite()) {
        return Err(validation("Workflow state position must be finite"));
    }
    rows.sort_by(|a, b| {
        type_order(&a.state.r#type, &b.state.r#type, &collator).then_with(|| {
            if multi {
                std::cmp::Ordering::Equal
            } else {
                b.state.position.total_cmp(&a.state.position)
            }
        })
    });
    Ok(())
}
pub fn sort_query(rows: &mut [GetIssuesForQueryIssuesNodes]) -> Result<(), AppError> {
    let multi = rows
        .first()
        .is_some_and(|first| rows.iter().any(|r| r.team.key != first.team.key));
    let collator = crate::platform::collation::root()?;
    if rows.iter().any(|r| !r.state.position.is_finite()) {
        return Err(validation("Workflow state position must be finite"));
    }
    rows.sort_by(|a, b| {
        type_order(&a.state.r#type, &b.state.r#type, &collator).then_with(|| {
            if multi {
                std::cmp::Ordering::Equal
            } else {
                b.state.position.total_cmp(&a.state.position)
            }
        })
    });
    Ok(())
}
pub fn priority(value: f64) -> Result<String, AppError> {
    Ok(match value {
        0.0 => "---".to_owned(),
        1.0 => "⚠⚠⚠".to_owned(),
        2.0 => "▄▆█".to_owned(),
        3.0 => "▄▆ ".to_owned(),
        4.0 => "▄  ".to_owned(),
        n => crate::json_number::finite_js_number(n)?.get().to_owned(),
    })
}
pub fn cycle_short(
    cycle: Option<&GetIssuesForStateIssuesNodesCycle>,
    anchor: Option<f64>,
) -> Result<(String, &'static str), AppError> {
    let Some(c) = cycle else {
        return Ok(("-".to_owned(), "none"));
    };
    let check = |n: f64, label: &str| {
        if n.is_finite() && n.fract() == 0.0 && n.abs() <= 9_007_199_254_740_991.0 {
            Ok(())
        } else {
            Err(validation(format!(
                "Expected {label} to be a safe integer, got {n}"
            )))
        }
    };
    check(c.number, "cycle number")?;
    if c.is_active {
        return Ok(("now".to_owned(), "active"));
    }
    if c.is_next {
        return Ok(("+1".to_owned(), "future"));
    }
    if c.is_previous {
        return Ok(("-1".to_owned(), "past"));
    }
    if let Some(anchor) = anchor {
        check(anchor, "active cycle number")?;
        let offset = c.number - anchor;
        return Ok(if offset == 0.0 {
            ("now".to_owned(), "active")
        } else {
            (
                format!("{}{offset}", if offset > 0.0 { "+" } else { "" }),
                if offset > 0.0 { "future" } else { "past" },
            )
        });
    }
    Ok((
        format!("#{}", crate::json_number::finite_js_number(c.number)?.get()),
        if c.is_past { "past" } else { "future" },
    ))
}
#[derive(Clone, Debug)]
pub struct TableRow {
    pub identifier: String,
    pub title: String,
    pub priority: f64,
    pub estimate: Option<f64>,
    pub initials: Option<String>,
    pub state_name: String,
    pub state_color: String,
    pub cycle: Option<GetIssuesForStateIssuesNodesCycle>,
    pub team: String,
    pub cycles_enabled: bool,
    pub anchor: Option<f64>,
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
) -> Result<String, AppError> {
    use crate::commands::display::{display_width, pad, truncate_text};
    if rows.is_empty() {
        return Ok("No issues found.".to_owned());
    }
    let id = rows
        .iter()
        .map(|r| r.identifier.encode_utf16().count())
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
        .collect::<Result<Vec<_>, _>>()?;
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
        .map(|r| crate::commands::table::time_ago(&r.updated, now))
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
        let mut cells = vec![pad(&priority(r.priority)?, 3), pad(&r.identifier, id)];
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
                .map(|n| crate::json_number::finite_js_number(n).map(|n| n.get().to_owned()))
                .transpose()?
                .unwrap_or_else(|| "-".to_owned()),
        );
        if show_cycle {
            let (c, kind) = cycles
                .get(i)
                .ok_or_else(|| AppError::new(AppErrorKind::Invariant, "Missing cycle display"))?;
            cells.push(format!(
                "{}{}",
                match *kind {
                    "active" => style(c, "32", color),
                    "past" | "none" => style(c, "90", color),
                    "future" => c.clone(),
                    _ =>
                        return Err(AppError::new(
                            AppErrorKind::Invariant,
                            "Unknown cycle display kind"
                        )),
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
            let initial =
                String::from_utf16_lossy(&initials.encode_utf16().take(2).collect::<Vec<_>>());
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
                times.get(i).ok_or_else(|| {
                    AppError::new(AppErrorKind::Invariant, "Missing time display")
                })?,
                uw,
            ),
            "90",
            color,
        ));
        lines.push(cells.join(" "));
    }
    Ok(lines.join("\n"))
}

/// Command-local typed preflight for source-valid text refused by the native menu.
pub fn project_menu_text(message: &str, labels: &[&str]) -> Result<(), AppError> {
    if std::iter::once(message)
        .chain(labels.iter().copied())
        .any(|s| s.trim().is_empty() || s.chars().any(char::is_control))
    {
        return Err(validation(
            "Project menu text must be nonempty and contain no control characters",
        )
        .with_suggestion(
            "Use an exact project name or UUID to avoid selecting similar projects.",
        ));
    }
    Ok(())
}
