//! Project detail lookup, pagination, picker and Markdown document.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, TimeZone, Utc};
use cynic::QueryBuilder;

use crate::commands::project_list;
use crate::commands::relative_time::format_relative_time;
use crate::error::Error;
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::project_view::{
    DateResolutionType, GetProjectDetails, GetProjectIssuesPage, GetProjectsForPicker,
    PickerProject, PickerVariables, ProjectDetails, ProjectDetailsVariables,
    ProjectIssuesVariables, ProjectMilestoneStatus, ViewInverseRelation, ViewRelation,
};
use crate::graphql::operations::teams::PageInfo;
use crate::graphql::transport::GraphQlTransport;
use crate::platform::collation;
use crate::platform::selector::SelectOption;

pub const CONTEXT: &str = "Failed to view project";
const PAGE_SIZE: i32 = 250;

fn protocol_error(message: String, suggestion: &str) -> Error {
    Error::new(message).with_hint(suggestion)
}

pub async fn fetch_details(
    transport: &GraphQlTransport,
    project_id: &str,
    original_input: &str,
) -> Result<ProjectDetails, Error> {
    let first = GraphQlRequest::with_variables(GetProjectDetails::build(ProjectDetailsVariables {
        id: project_id.to_owned(),
        first: PAGE_SIZE,
    }));
    let data: GetProjectDetails = transport.execute(&first).await.map_err(Error::from)?;
    let mut project = data
        .project
        .ok_or_else(|| Error::not_found("Project", original_input))?;
    let mut page_info = project.issues.page_info.clone();
    let mut seen = HashSet::new();
    while page_info.has_next_page {
        let cursor = page_info.end_cursor.ok_or_else(|| protocol_error(
            format!("Linear reported more issues for project {} but returned no cursor to fetch them.", project.name),
            "Retry, or report this if it keeps happening.",
        ))?;
        if !seen.insert(cursor.clone()) {
            return Err(protocol_error(
                format!(
                    "Linear returned a repeated issue cursor for project {}.",
                    project.name
                ),
                "Retry, or report this if it keeps happening.",
            ));
        }
        let query =
            GraphQlRequest::with_variables(GetProjectIssuesPage::build(ProjectIssuesVariables {
                id: project_id.to_owned(),
                first: PAGE_SIZE,
                after: cursor.clone(),
            }));
        let data: GetProjectIssuesPage = transport.execute(&query).await.map_err(Error::from)?;
        let next = data
            .project
            .ok_or_else(|| Error::not_found("Project", original_input))?;
        project.issues.nodes.extend(next.issues.nodes);
        page_info = next.issues.page_info;
        if page_info.has_next_page && page_info.end_cursor.as_deref() == Some(&cursor) {
            return Err(protocol_error(
                format!(
                    "Linear returned the same issue cursor twice for project {}.",
                    project.name
                ),
                "Retry, or report this if it keeps happening.",
            ));
        }
    }
    project.issues.page_info = page_info;
    Ok(project)
}

pub async fn fetch_picker(
    transport: &GraphQlTransport,
    team_key: Option<&str>,
) -> Result<Vec<PickerProject>, Error> {
    let filter = project_list::filter(team_key, None);
    let mut projects = Vec::new();
    let mut after: Option<String> = None;
    let mut seen = HashSet::new();
    loop {
        let query = GraphQlRequest::with_variables(GetProjectsForPicker::build(PickerVariables {
            filter: filter.clone(),
            first: 100,
            after: after.clone(),
        }));
        let data: GetProjectsForPicker = transport.execute(&query).await.map_err(Error::from)?;
        projects.extend(data.projects.nodes);
        let page_info = data.projects.page_info;
        if !page_info.has_next_page {
            break;
        }
        let cursor = page_info.end_cursor.ok_or_else(|| {
            protocol_error(
                "Linear reported more projects but returned no new cursor to fetch them."
                    .to_owned(),
                "Retry, or pass a project explicitly.",
            )
        })?;
        if after.as_deref() == Some(&cursor) {
            return Err(protocol_error(
                "Linear reported more projects but returned no new cursor to fetch them."
                    .to_owned(),
                "Retry, or pass a project explicitly.",
            ));
        }
        if !seen.insert(cursor.clone()) {
            return Err(protocol_error(
                "Linear returned a repeated project picker cursor.".to_owned(),
                "Retry, or pass a project explicitly.",
            ));
        }
        after = Some(cursor);
    }
    if projects.is_empty() {
        let identifier = team_key.map_or("this workspace".to_owned(), |key| format!("team {key}"));
        let suggestion = team_key.map_or("Create one with `linear project create`.".to_owned(), |key| format!("No projects are accessible to team {key}. Check `linear project list --all-teams`, or create one with `linear project create`."));
        return Err(Error::not_found("Project", &identifier).with_hint(suggestion));
    }
    Ok(projects)
}

pub fn picker_options(projects: &[PickerProject]) -> Vec<SelectOption> {
    let mut ordered: Vec<_> = projects.iter().collect();
    ordered.sort_by(|a, b| {
        collation::compare(&a.name.to_lowercase(), &b.name.to_lowercase())
            .then_with(|| collation::compare(&a.slug_id, &b.slug_id))
            .then_with(|| collation::compare(a.id.inner(), b.id.inner()))
    });
    ordered
        .into_iter()
        .map(|project| {
            let mut parts = vec![project.name.clone(), project.status.name.clone()];
            let teams = project
                .teams
                .nodes
                .iter()
                .map(|team| team.key.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            if !teams.is_empty() {
                parts.push(teams);
            }
            parts.push(project.slug_id.clone());
            SelectOption {
                label: parts.join("  ·  "),
                value: project.id.inner().to_owned(),
            }
        })
        .collect()
}

pub fn json(project: &ProjectDetails) -> Result<Vec<u8>, Error> {
    let mut bytes = serde_json::to_vec_pretty(project)
        .map_err(|error| Error::new("could not serialize project").with_source(error))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn ratio(value: f64) -> String {
    format!("{}%", (value * 100.0 + 0.5).floor())
}
fn percent(value: f64) -> String {
    format!("{}%", (value + 0.5).floor())
}
fn date_resolution(value: &DateResolutionType) -> &str {
    match value {
        DateResolutionType::HalfYear => "halfYear",
        DateResolutionType::Month => "month",
        DateResolutionType::Quarter => "quarter",
        DateResolutionType::Year => "year",
        DateResolutionType::Unknown(other) => other,
    }
}
fn milestone_status(value: &ProjectMilestoneStatus) -> &str {
    match value {
        ProjectMilestoneStatus::Done => "done",
        ProjectMilestoneStatus::Next => "next",
        ProjectMilestoneStatus::Overdue => "overdue",
        ProjectMilestoneStatus::Unstarted => "unstarted",
        ProjectMilestoneStatus::Unknown(other) => other,
    }
}
fn display_name(user: Option<&crate::graphql::operations::project_view::ViewUser>) -> Option<&str> {
    user.map(|user| {
        if user.display_name.is_empty() {
            user.name.as_str()
        } else {
            user.display_name.as_str()
        }
    })
}
fn note(info: &PageInfo) -> &'static str {
    if info.has_next_page {
        "\n_…and more (showing the first 250)._\n"
    } else {
        ""
    }
}
fn joined(values: Vec<String>, info: &PageInfo) -> String {
    let joined = values.join(", ");
    if info.has_next_page {
        format!("{joined}, …")
    } else {
        joined
    }
}
fn sorted_by<T>(items: &[T], key: impl Fn(&T) -> f64) -> Vec<&T> {
    let mut ordered: Vec<_> = items.iter().collect();
    ordered.sort_by(|a, b| key(a).total_cmp(&key(b)));
    ordered
}
fn relation_label(own: &str, other: &str) -> &'static str {
    match (own, other) {
        ("end", "start") => "Blocks",
        ("start", "end") => "Blocked by",
        _ => "Related to",
    }
}
fn milestone_note(
    own: Option<&crate::graphql::operations::project_view::ViewMilestoneRef>,
    other: Option<&crate::graphql::operations::project_view::ViewMilestoneRef>,
) -> String {
    let mut parts = Vec::new();
    if let Some(own) = own {
        parts.push(format!("from milestone {}", own.name));
    }
    if let Some(other) = other {
        parts.push(format!("to milestone {}", other.name));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" _({})_", parts.join(", "))
    }
}
fn outgoing(relation: &ViewRelation) -> String {
    format!(
        "- **{}** {}: {}{}\n",
        relation_label(&relation.anchor_type, &relation.related_anchor_type),
        relation.related_project.name,
        relation.related_project.url,
        milestone_note(
            relation.project_milestone.as_ref(),
            relation.related_project_milestone.as_ref()
        )
    )
}
fn incoming(relation: &ViewInverseRelation) -> String {
    format!(
        "- **{}** {}: {}{}\n",
        relation_label(&relation.related_anchor_type, &relation.anchor_type),
        relation.project.name,
        relation.project.url,
        milestone_note(
            relation.related_project_milestone.as_ref(),
            relation.project_milestone.as_ref()
        )
    )
}

pub fn markdown<Tz: TimeZone>(
    project: &ProjectDetails,
    now: DateTime<Utc>,
    zone: &Tz,
) -> Result<String, Error> {
    let mut out = if let Some(identifier) = &project.identifier
        && !identifier.is_empty()
    {
        format!("# {} [{}]", project.name, identifier)
    } else {
        format!("# {}", project.name)
    };
    let priority = match project.priority {
        0 => "None".to_owned(),
        1 => "Urgent".to_owned(),
        2 => "High".to_owned(),
        3 => "Medium".to_owned(),
        4 => "Low".to_owned(),
        other => other.to_string(),
    };
    let mut meta = vec![
        format!("**Status:** {}", project.status.name),
        format!("**Priority:** {priority}"),
    ];
    if let Some(health) = &project.health {
        meta.push(format!("**Health:** {}", health.as_str()));
    }
    meta.push(format!(
        "**Lead:** {}",
        display_name(project.lead.as_ref())
            .map_or("Unassigned".to_owned(), |name| format!("@{name}"))
    ));
    if !project.teams.nodes.is_empty() {
        meta.push(format!(
            "**Teams:** {}",
            joined(
                project
                    .teams
                    .nodes
                    .iter()
                    .map(|t| format!("{} ({})", t.name, t.key))
                    .collect(),
                &project.teams.page_info
            )
        ));
    }
    if !project.labels.nodes.is_empty() {
        meta.push(format!(
            "**Labels:** {}",
            joined(
                project
                    .labels
                    .nodes
                    .iter()
                    .map(|l| l.name.clone())
                    .collect(),
                &project.labels.page_info
            )
        ));
    }
    if !project.initiatives.nodes.is_empty() {
        meta.push(format!(
            "**Initiatives:** {}",
            joined(
                project
                    .initiatives
                    .nodes
                    .iter()
                    .map(|i| i.name.clone())
                    .collect(),
                &project.initiatives.page_info
            )
        ));
    }
    meta.push(format!("**Progress:** {}", ratio(project.progress.get())));
    out.push_str(&format!("\n\n{}", meta.join(" | ")));
    if !project.description.is_empty() {
        out.push_str(&format!("\n\n{}", project.description));
    }
    if let Some(content) = &project.content
        && !content.is_empty()
    {
        out.push_str(&format!("\n\n## Overview\n\n{content}"));
    }
    if !project.project_milestones.nodes.is_empty() {
        out.push_str("\n\n## Milestones\n\n");
        for milestone in sorted_by(&project.project_milestones.nodes, |item| {
            item.sort_order.get()
        }) {
            let mut parts = vec![
                milestone_status(&milestone.status).to_owned(),
                percent(milestone.progress.get()),
            ];
            if let Some(date) = &milestone.target_date {
                parts.push(format!("target {}", date.0));
            }
            out.push_str(&format!(
                "- **{}** _[{}]_\n",
                milestone.name,
                parts.join(", ")
            ));
            if let Some(description) = &milestone.description
                && !description.is_empty()
            {
                out.push_str(&format!("  {}\n", description.replace('\n', "\n  ")));
            }
        }
        out.push_str(note(&project.project_milestones.page_info));
        out = out.trim_end().to_owned();
    }
    if !project.external_links.nodes.is_empty() {
        out.push_str("\n\n## Resources\n\n");
        for link in sorted_by(&project.external_links.nodes, |item| item.sort_order.get()) {
            out.push_str(&format!("- **{}**: {}\n", link.label, link.url));
        }
        out.push_str(note(&project.external_links.page_info));
        out = out.trim_end().to_owned();
    }
    if !project.documents.nodes.is_empty() {
        out.push_str("\n\n## Documents\n\n");
        for doc in sorted_by(&project.documents.nodes, |item| item.sort_order.get()) {
            out.push_str(&format!("- **{}**: {}\n", doc.title, doc.url));
        }
        out.push_str(note(&project.documents.page_info));
        out = out.trim_end().to_owned();
    }
    if !project.attachments.nodes.is_empty() {
        out.push_str("\n\n## Attachments\n\n");
        for item in &project.attachments.nodes {
            out.push_str(&format!(
                "- **{}**: {}{}\n",
                item.title,
                item.url,
                item.source_type
                    .as_deref()
                    .filter(|source| !source.is_empty())
                    .map_or(String::new(), |source| format!(" _[{source}]_"))
            ));
            if let Some(subtitle) = &item.subtitle
                && !subtitle.is_empty()
            {
                out.push_str(&format!("  _{subtitle}_\n"));
            }
        }
        out.push_str(note(&project.attachments.page_info));
        out = out.trim_end().to_owned();
    }
    if !project.relations.nodes.is_empty() || !project.inverse_relations.nodes.is_empty() {
        out.push_str("\n\n## Related projects\n\n");
        for relation in &project.relations.nodes {
            out.push_str(&outgoing(relation));
        }
        for relation in &project.inverse_relations.nodes {
            out.push_str(&incoming(relation));
        }
        out.push_str(note(&project.relations.page_info));
        out.push_str(note(&project.inverse_relations.page_info));
        out = out.trim_end().to_owned();
    }
    if let Some(update) = &project.last_update {
        out.push_str("\n\n## Latest Update\n\n");
        out.push_str(&format!(
            "**By:** {}\n**When:** {}\n",
            display_name(update.user.as_ref()).unwrap_or("Unknown"),
            format_relative_time(&update.created_at.0, now, zone)
        ));
        if let Some(health) = &update.health {
            out.push_str(&format!("**Health:** {}\n", health.as_str()));
        }
        out.push_str(&format!("\n{}", update.body));
    }
    if !project.issues.nodes.is_empty() {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        let mut unknown_order = Vec::new();
        for issue in &project.issues.nodes {
            let key = issue.state.state_type.as_str();
            if !counts.contains_key(key) {
                unknown_order.push(key);
            }
            *counts.entry(key).or_default() += 1;
        }
        let mut parts = vec![format!("{} total", project.issues.nodes.len())];
        for (kind, label) in [
            ("triage", "triage"),
            ("backlog", "backlog"),
            ("unstarted", "to do"),
            ("started", "in progress"),
            ("completed", "completed"),
            ("canceled", "canceled"),
        ] {
            if let Some(count) = counts.remove(kind) {
                parts.push(format!("{count} {label}"));
            }
        }
        for kind in unknown_order {
            if let Some(count) = counts.remove(kind) {
                parts.push(format!("{count} {kind}"));
            }
        }
        out.push_str(&format!("\n\n## Issues\n\n{}", parts.join(" · ")));
    }
    let mut rows = Vec::new();
    let mut push = |label: &str, value: Option<String>| {
        if let Some(value) = value
            && !value.is_empty()
        {
            rows.push(format!("- **{label}:** {value}"));
        }
    };
    push("Slug", Some(project.slug_id.clone()));
    push("URL", Some(project.url.clone()));
    push("Icon", project.icon.clone());
    push(
        "Creator",
        display_name(project.creator.as_ref()).map(str::to_owned),
    );
    if !project.members.nodes.is_empty() {
        push(
            "Members",
            Some(joined(
                project
                    .members
                    .nodes
                    .iter()
                    .map(|u| display_name(Some(u)).unwrap_or("").to_owned())
                    .collect(),
                &project.members.page_info,
            )),
        );
    }
    if project.scope.get() > 0.0 {
        push("Scope", Some(project.scope.to_string()));
    }
    let project_date = |date: &Option<crate::graphql::scalars::TimelessDate>,
                        resolution: &Option<DateResolutionType>| {
        date.as_ref().map(|date| {
            resolution.as_ref().map_or(date.0.clone(), |r| {
                format!("{} ({})", date.0, date_resolution(r))
            })
        })
    };
    push(
        "Start date",
        project_date(&project.start_date, &project.start_date_resolution),
    );
    push(
        "Target date",
        project_date(&project.target_date, &project.target_date_resolution),
    );
    let relative =
        |date: &crate::graphql::scalars::DateTime| format_relative_time(&date.0, now, zone);
    push("Started", project.started_at.as_ref().map(relative));
    push("Completed", project.completed_at.as_ref().map(relative));
    push("Canceled", project.canceled_at.as_ref().map(relative));
    push(
        "Archived",
        project.archived_at.as_ref().map(|date| {
            let formatted = relative(date);
            if project.auto_archived_at.is_some() {
                format!("{formatted} (automatically)")
            } else {
                formatted
            }
        }),
    );
    push(
        "Health updated",
        project.health_updated_at.as_ref().map(relative),
    );
    push("Created", Some(relative(&project.created_at)));
    push("Updated", Some(relative(&project.updated_at)));
    out.push_str(&format!("\n\n## Details\n\n{}", rows.join("\n")));
    Ok(out)
}
