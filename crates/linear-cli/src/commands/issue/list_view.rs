//! Issue lists as tables: ordering, priority glyphs, cycles and labels.
use std::time::SystemTime;

use crate::cli::issue::IssueColumn;
use crate::commands::relative_time;
use crate::commands::table::{Cell, Column, Table};
use crate::error::Result;
use crate::graphql::operations::issue_read::*;
use crate::graphql::scalars::WholeNumber;
use crate::platform::style;
use crate::refs::{IssueReference, workflow_states};
use crate::templates::TemplateConfig;

mod output;
pub(super) use output::Output;

/// Orders issues by workflow state type, then (within one team) by the
/// state's position, highest first.
pub fn sort(rows: &mut [ListedIssue]) {
    let multi = rows
        .first()
        .is_some_and(|first| rows.iter().any(|r| r.team.key != first.team.key));
    rows.sort_by(|a, b| {
        workflow_states::compare_types(&a.state.r#type, &b.state.r#type).then_with(|| {
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
/// Whether an unfinished issue blocks `issue`.
fn blocked(issue: &ListedIssue) -> bool {
    issue.inverse_relations.nodes.iter().any(|relation| {
        relation.r#type == "blocks"
            && !matches!(
                relation.issue.state.r#type.as_str(),
                "completed" | "canceled"
            )
    })
}
struct TableLayout<'a> {
    team: bool,
    assignee: bool,
    columns: &'a [IssueColumn],
}

fn default_columns(rows: &[ListedIssue], team: bool, assignee: bool) -> Vec<IssueColumn> {
    use IssueColumn::*;

    let cycle = rows
        .iter()
        .any(|r| r.cycle.is_some() || r.team.cycles_enabled);
    let mut columns = vec![Priority, Id];
    columns.extend(team.then_some(Team));
    columns.extend([Title, Labels, Blocked, Estimate]);
    columns.extend(cycle.then_some(Cycle));
    columns.extend(assignee.then_some(Assignee));
    columns.extend([State, Updated]);
    columns
}

fn table(
    templates: &TemplateConfig,
    rows: &[ListedIssue],
    layout: TableLayout<'_>,
    now: SystemTime,
    styled: bool,
) -> Result<Table> {
    let explicit = !layout.columns.is_empty();
    let defaults;
    let columns = if explicit {
        layout.columns
    } else {
        defaults = default_columns(rows, layout.team, layout.assignee);
        &defaults
    };
    let mut table = Table::new(columns.iter().map(|&column| table_column(column, explicit)));
    for row in rows {
        table.row(
            columns
                .iter()
                .map(|&column| issue_cell(templates, row, column, now, styled))
                .collect::<Result<Vec<_>>>()?,
        );
    }
    Ok(table)
}

fn table_column(column: IssueColumn, explicit: bool) -> Column {
    use IssueColumn::*;

    let (column, drop_rank) = match column {
        Priority => (Column::fixed("◌"), None),
        Id => (Column::flexible("ID"), None),
        Team => (Column::fixed("TEAM"), Some(6)),
        Title => (Column::flexible("TITLE"), None),
        Labels => (Column::flexible("LABELS"), Some(2)),
        Blocked => (Column::fixed("B"), Some(5)),
        Estimate => (Column::fixed("E"), Some(4)),
        Cycle => (Column::fixed("CYC"), Some(3)),
        Assignee => (Column::fixed("A"), None),
        State => (Column::fixed("STATE"), None),
        Updated => (Column::fixed("UPDATED"), Some(1)),
    };
    match drop_rank {
        Some(rank) if !explicit => column.droppable(rank),
        _ => column,
    }
}

fn issue_cell(
    templates: &TemplateConfig,
    row: &ListedIssue,
    column: IssueColumn,
    now: SystemTime,
    styled: bool,
) -> Result<Cell> {
    use IssueColumn::*;

    Ok(match column {
        Priority => Cell::from(priority(row.priority)),
        Id => {
            let reference = IssueReference::new(&row.identifier, Some(&row.url));
            let display = templates.format_issue_id(&reference, styled)?;
            let marker = super::lifecycle::Lifecycle::of(row.trashed, row.archived_at).marker();
            Cell::display(display, marker)
        }
        Team => Cell::from(row.team.key.as_str()),
        Title => Cell::from(row.title.as_str()),
        Labels => labels_cell(&row.labels.nodes),
        Blocked if blocked(row) => Cell::styled("⊘", style::yellow),
        Blocked => Cell::from(""),
        Estimate => Cell::from(
            row.estimate
                .as_ref()
                .map_or_else(|| "-".to_owned(), ToString::to_string),
        ),
        Cycle => {
            let anchor = row.team.active_cycle.as_ref().map(|cycle| cycle.number);
            let (text, kind) = cycle_short(row.cycle.as_ref(), anchor);
            match kind {
                CycleKind::Active => Cell::styled(text, style::green),
                CycleKind::Past | CycleKind::None => Cell::styled(text, style::gray),
                CycleKind::Future => Cell::from(text),
            }
        }
        Assignee => {
            let initials = row
                .assignee
                .as_ref()
                .map(|assignee| assignee.initials.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or("-");
            Cell::from(initials.chars().take(2).collect::<String>())
        }
        State => {
            let color = row.state.color.clone();
            Cell::styled(row.state.name.as_str(), move |text, on| {
                style::rgb(text, &color, on)
            })
        }
        Updated => Cell::styled(
            relative_time::ago(row.updated_at.0, now.into(), &chrono::Local),
            style::gray,
        ),
    })
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

/// Prints issues as a table, through the pager on a terminal.
fn print_table(ctx: &crate::ctx::Ctx, table: &Table, paging: bool) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn lifecycle_markers_survive_narrow_tables_with_long_titles() {
        for (trashed, archived, marker) in [
            (Some(true), None, "[deleted]"),
            (Some(false), Some("2026-11-09T20:50:36.574Z"), "[archived]"),
        ] {
            let row: ListedIssue = serde_json::from_value(json!({
                "id": "issue-1", "identifier": "DEMO-42",
                "title": "Investigate the widget regression with a very long title that must be truncated",
                "url": "https://linear.app/workspace-one/issue/DEMO-42", "priority": 2, "priorityLabel": "High", "estimate": null,
                "trashed": trashed, "archivedAt": archived,
                "createdAt": "2024-01-01T00:00:00.000Z", "updatedAt": "2024-01-01T00:00:00.000Z",
                "state": { "id": "state-1", "name": "Todo", "color": "#123456", "type": "unstarted", "position": 1 },
                "assignee": null, "project": null, "projectMilestone": null, "cycle": null,
                "team": { "id": "team-1", "key": "DEMO", "name": "Demo", "cyclesEnabled": false, "activeCycle": null },
                "labels": { "nodes": [] }, "inverseRelations": { "nodes": [] }
            })).expect("issue row");
            let linked_id =
                crate::platform::markdown_terminal::hyperlink(&row.identifier, &row.url);
            let output = table(
                &TemplateConfig::builtin(),
                &[row],
                TableLayout {
                    team: true,
                    assignee: true,
                    columns: &[],
                },
                SystemTime::now(),
                true,
            )
            .expect("renderable issue table")
            .render(Some(80), true);
            assert!(
                output.contains(&format!("{linked_id} {marker}")),
                "{output}"
            );
            assert!(
                output.contains('…'),
                "the title should be truncated: {output}"
            );
        }
    }
}
