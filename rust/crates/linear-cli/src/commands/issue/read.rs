//! Command-local issue read filters, lookups, pagination and presentation.
use std::time::SystemTime;

use crate::commands::relative_time;
use crate::commands::table::{Cell, Column, Table};
use crate::error::Error;
use crate::graphql::operations::number::{Float, WholeNumber};
use crate::graphql::pagination::{self, Page};
use crate::graphql::scalars::DateTimeOrDuration;
use crate::graphql::{
    envelope::GraphQlRequest, operations::issue_read::*, transport::GraphQlTransport,
};
use crate::platform::style;
use crate::refs::{ProjectReference, is_linear_uuid, reject_linear_url};
use chrono::{DateTime, SecondsFormat, Utc};
use cynic::QueryBuilder;

use std::num::NonZeroU32;

/// Filters on issues created or updated at or after the given times.
pub fn apply_dates(
    filter: &mut IssueFilter,
    created: Option<DateTime<Utc>>,
    updated: Option<DateTime<Utc>>,
) {
    let at_or_after = |time: DateTime<Utc>| DateComparator {
        gte: Some(DateTimeOrDuration(
            time.to_rfc3339_opts(SecondsFormat::Millis, true),
        )),
    };
    if let Some(time) = created {
        filter.created_at = Some(at_or_after(time));
    }
    if let Some(time) = updated {
        filter.updated_at = Some(at_or_after(time));
    }
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
            return Err(Error::new("--state value is empty").with_hint(format!(
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
            let page: GetWorkflowStatesInScope = transport.execute(&request).await?;
            states.extend(page.workflow_states.nodes);
            if !page.workflow_states.page_info.has_next_page {
                break;
            }
            let next = page.workflow_states.page_info.end_cursor;
            if next.is_none() || next == after {
                return Err(Error::new(
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
        (None, None) => return Err(Error::new("--state selection is empty")),
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
    let id = cynic::Id::new(crate::commands::user::resolve(transport, input, "User").await?);
    Ok(Some(NullableUserFilter {
        id: Some(IDComparator {
            eq: Some(id),
            ..Default::default()
        }),
        ..Default::default()
    }))
}
/// The ID of the project `reference` names: a UUID as given, else an exact
/// name match (refusing ambiguous names), else a slug ID match.
pub async fn project_id(
    transport: &GraphQlTransport,
    reference: &ProjectReference,
) -> Result<Option<String>, Error> {
    use crate::graphql::operations::project_view::{
        GetProjectIdByName, GetProjectIdBySlugId, ProjectReferenceVariables, ProjectSlugVariables,
    };
    let slug = match reference {
        ProjectReference::Id(id) => return Ok(Some(id.clone())),
        ProjectReference::Slug(slug) => slug,
        ProjectReference::NameOrSlug(name) => {
            let data: GetProjectIdByName = transport
                .execute(&GraphQlRequest::with_variables(GetProjectIdByName::build(
                    ProjectReferenceVariables { name: name.clone() },
                )))
                .await?;
            if data.projects.nodes.len() > 1 {
                return Err(Error::new(format!(
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
    let data: GetProjectIdBySlugId = transport
        .execute(&GraphQlRequest::with_variables(
            GetProjectIdBySlugId::build(ProjectSlugVariables {
                slug_id: slug.clone(),
            }),
        ))
        .await?;
    Ok(data
        .projects
        .nodes
        .into_iter()
        .next()
        .map(|p| p.id.into_inner()))
}
/// A milestone UUID, or the ID of the named milestone in `project`.
pub async fn milestone_id(
    transport: &GraphQlTransport,
    value: &str,
    project: Option<&str>,
) -> Result<String, Error> {
    if is_linear_uuid(value) {
        return Ok(value.to_owned());
    }
    reject_linear_url(value, "a milestone name or UUID")?;
    let project = project.ok_or_else(|| Error::new(format!("Cannot resolve milestone \"{value}\" without --project")).with_hint("Pass a milestone UUID, or specify --project so the milestone name can be looked up within that project."))?;
    crate::commands::milestone::id_by_name(transport, project, value).await
}
pub async fn mine(
    transport: &GraphQlTransport,
    filter: IssueFilter,
    priority: bool,
    limit: Option<NonZeroU32>,
) -> Result<Vec<GetIssuesForStateIssuesNodes>, Error> {
    let mut rows = pagination::collect(limit, |after, first| {
        let request =
            GraphQlRequest::with_variables(GetIssuesForState::build(GetIssuesForStateVariables {
                sort: Some(sort_payload(priority)),
                filter: filter.clone(),
                first: Some(first),
                after,
            }));
        async move {
            let data: GetIssuesForState = transport.execute(&request).await?;
            Ok(Page {
                nodes: data.issues.nodes,
                page_info: data.issues.page_info,
            })
        }
    })
    .await?;
    sort_mine(&mut rows);
    Ok(rows)
}
pub async fn query(
    transport: &GraphQlTransport,
    filter: Option<IssueFilter>,
    priority: bool,
    limit: Option<NonZeroU32>,
    archived: bool,
) -> Result<Vec<GetIssuesForQueryIssuesNodes>, Error> {
    let mut rows = pagination::collect(limit, |after, first| {
        let request =
            GraphQlRequest::with_variables(GetIssuesForQuery::build(GetIssuesForQueryVariables {
                sort: Some(sort_payload(priority)),
                filter: filter.clone(),
                first: Some(first),
                after,
                include_archived: archived.then_some(true),
            }));
        async move {
            let data: GetIssuesForQuery = transport.execute(&request).await?;
            Ok(Page {
                nodes: data.issues.nodes,
                page_info: data.issues.page_info,
            })
        }
    })
    .await?;
    sort_query(&mut rows);
    Ok(rows)
}
pub async fn search(
    transport: &GraphQlTransport,
    filter: Option<IssueFilter>,
    term: String,
    limit: Option<NonZeroU32>,
    archived: bool,
    comments: bool,
) -> Result<Vec<SearchIssuesSearchIssuesNodes>, Error> {
    pagination::collect(limit, |after, first| {
        let request = GraphQlRequest::with_variables(SearchIssues::build(SearchIssuesVariables {
            term: term.clone(),
            filter: filter.clone(),
            first: Some(first),
            after,
            include_archived: archived.then_some(true),
            include_comments: comments.then_some(true),
            order_by: None,
        }));
        async move {
            let data: SearchIssues = transport.execute(&request).await?;
            Ok(Page {
                nodes: data.search_issues.nodes,
                page_info: data.search_issues.page_info,
            })
        }
    })
    .await
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CycleKind {
    None,
    Past,
    Active,
    Future,
}

/// The issue's cycle relative to the team's active one: `now`, `+1`, `-2`,
/// or `#7` when the team has no active cycle.
pub fn cycle_short(
    cycle: Option<&GetIssuesForStateIssuesNodesCycle>,
    anchor: Option<WholeNumber>,
) -> (String, CycleKind) {
    let Some(c) = cycle else {
        return ("-".to_owned(), CycleKind::None);
    };
    if c.is_active {
        return ("now".to_owned(), CycleKind::Active);
    }
    if c.is_next {
        return ("+1".to_owned(), CycleKind::Future);
    }
    if c.is_previous {
        return ("-1".to_owned(), CycleKind::Past);
    }
    if let Some(anchor) = anchor {
        let offset = i64::from(c.number.0) - i64::from(anchor.0);
        return match offset.cmp(&0) {
            std::cmp::Ordering::Equal => ("now".to_owned(), CycleKind::Active),
            std::cmp::Ordering::Greater => (format!("+{offset}"), CycleKind::Future),
            std::cmp::Ordering::Less => (offset.to_string(), CycleKind::Past),
        };
    }
    let kind = if c.is_past {
        CycleKind::Past
    } else {
        CycleKind::Future
    };
    (format!("#{}", c.number), kind)
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
/// Issues as a table. `team` adds the team column and `assignee` the
/// assignee initials.
pub fn table(rows: &[TableRow], team: bool, assignee: bool, now: SystemTime) -> Table {
    let show_cycle = rows.iter().any(|r| r.cycle.is_some() || r.cycles_enabled);
    let mut columns = vec![Column::fixed("◌"), Column::fixed("ID")];
    if team {
        columns.push(Column::fixed("TEAM"));
    }
    columns.extend([
        Column::flexible("TITLE"),
        Column::flexible("LABELS"),
        Column::fixed("B"),
        Column::fixed("E"),
    ]);
    if show_cycle {
        columns.push(Column::fixed("CYC"));
    }
    if assignee {
        columns.push(Column::fixed("A"));
    }
    columns.extend([Column::fixed("STATE"), Column::fixed("UPDATED")]);
    let mut table = Table::new(columns);
    for r in rows {
        let mut cells = vec![
            Cell::from(priority(r.priority)),
            Cell::from(r.identifier.as_str()),
        ];
        if team {
            cells.push(Cell::from(r.team.as_str()));
        }
        cells.push(Cell::from(r.title.as_str()));
        cells.push(labels_cell(&r.labels));
        cells.push(if r.blocked {
            Cell::styled("⊘", style::yellow)
        } else {
            Cell::from("")
        });
        cells.push(Cell::from(
            r.estimate
                .as_ref()
                .map_or_else(|| "-".to_owned(), ToString::to_string),
        ));
        if show_cycle {
            let (text, kind) = cycle_short(r.cycle.as_ref(), r.anchor);
            cells.push(match kind {
                CycleKind::Active => Cell::styled(text, style::green),
                CycleKind::Past | CycleKind::None => Cell::styled(text, style::gray),
                CycleKind::Future => Cell::from(text),
            });
        }
        if assignee {
            let initials = r
                .initials
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or("-");
            cells.push(Cell::from(initials.chars().take(2).collect::<String>()));
        }
        let state_color = r.state_color.clone();
        cells.push(Cell::styled(r.state_name.as_str(), move |text, on| {
            style::rgb(text, &state_color, on)
        }));
        cells.push(Cell::styled(
            relative_time::format_relative_time(&r.updated, now.into(), &chrono::Local),
            style::gray,
        ));
        table.row(cells);
    }
    table
}

/// The labels, comma separated, each in its own color.
fn labels_cell(labels: &[GetIssuesForStateIssuesNodesLabelsNodes]) -> Cell {
    let mut text = String::new();
    let mut spans = Vec::new();
    for label in labels {
        if !text.is_empty() {
            text.push_str(", ");
        }
        spans.push((
            text.len()..text.len() + label.name.len(),
            label.color.clone(),
        ));
        text.push_str(&label.name);
    }
    let full = text.clone();
    Cell::styled(text, move |shown, on| {
        // `shown` is a prefix of the full text, possibly cut and padded.
        let kept = shown
            .char_indices()
            .zip(full.chars())
            .take_while(|((_, left), right)| left == right)
            .last()
            .map_or(0, |((index, ch), _)| index + ch.len_utf8());
        let mut painted = String::new();
        let mut done = 0;
        for (span, hex) in &spans {
            let end = span.end.min(kept);
            if span.start >= end {
                break;
            }
            let slice = |range: std::ops::Range<usize>| {
                shown
                    .get(range)
                    .expect("label boundaries fall on characters shared with the full text")
            };
            painted.push_str(slice(done..span.start));
            painted.push_str(&style::rgb(slice(span.start..end), hex, on));
            done = end;
        }
        painted.push_str(shown.get(done..).expect("done is a character boundary"));
        painted
    })
}

/// The team `reference` (a key, name, ID or URL) names.
pub(super) fn resolve_team(
    ctx: &crate::ctx::Ctx,
    client: &GraphQlTransport,
    reference: &str,
) -> Result<crate::refs::ResolvedTeam, Error> {
    let lookup = crate::refs::prepare_team_lookup(reference, &ctx.scope()?)?;
    ctx.block_on(crate::refs::resolve_team_with_transport(&lookup, client))
}

/// The project `--project` names. When no project matches exactly, a terminal
/// user may pick one of the similarly named projects.
pub(super) fn resolve_project(
    ctx: &crate::ctx::Ctx,
    client: &GraphQlTransport,
    value: Option<&str>,
) -> Result<Option<String>, Error> {
    use crate::platform::prompt::Choice;
    let Some(value) = value else { return Ok(None) };
    let reference = crate::refs::prepare_project_lookup(value, &ctx.scope()?)?;
    if let Some(id) = ctx.block_on(project_id(client, &reference))? {
        return Ok(Some(id));
    }
    let data: GetProjectIdOptionsByName =
        ctx.block_on(client.execute(&GraphQlRequest::with_variables(
            GetProjectIdOptionsByName::build(GetProjectIdOptionsByNameVariables {
                name: value.to_owned(),
            }),
        )))?;
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
    let (message, decline) = match rows.as_slice() {
        [(_, name)] => (
            format!(
                "Project named {value} does not exist, but {name} exists. Is this what you meant?"
            ),
            "no",
        ),
        _ => (
            format!(
                "Project with {value} does not exist, but the following exist. Is any of these what you meant?"
            ),
            "none of the above",
        ),
    };
    let single = rows.len() == 1;
    let mut choices: Vec<_> = rows
        .into_iter()
        .map(|(id, name)| Choice::new(if single { "yes".to_owned() } else { name }, Some(id)))
        .collect();
    choices.push(Choice::new(decline, None));
    ctx.prompter()?.select(&message, choices)
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
    ctx.block_on(crate::commands::cycle::view::resolve_id(
        client,
        &team_id,
        value,
        url.as_ref(),
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

/// Prints issues as a table, through the pager on a terminal.
pub(super) fn print_table(ctx: &crate::ctx::Ctx, table: &Table, paging: bool) -> Result<(), Error> {
    if table.is_empty() {
        return ctx.print("No issues found.\n");
    }
    let rendered = table.render_for(ctx);
    if ctx.stdout_tty() {
        ctx.page(&rendered, paging)
    } else {
        ctx.print(rendered)
    }
}
