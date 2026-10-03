//! Issue lists as tables: ordering, priority glyphs, cycles and labels.
use std::time::SystemTime;

use crate::commands::relative_time;
use crate::commands::table::{Cell, Column, Table};
use crate::error::Error;
use crate::graphql::operations::issue_read::*;
use crate::graphql::scalars::WholeNumber;
use crate::platform::style;

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
/// Orders issues by workflow state type, then (within one team) by the
/// state's position, highest first.
pub fn sort(rows: &mut [ListedIssue]) {
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
/// Issues as a table. `team` adds the team column and `assignee` the
/// assignee initials.
pub fn table(rows: &[ListedIssue], team: bool, assignee: bool, now: SystemTime) -> Table {
    let show_cycle = rows
        .iter()
        .any(|r| r.cycle.is_some() || r.team.cycles_enabled);
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
            cells.push(Cell::from(r.team.key.as_str()));
        }
        cells.push(Cell::from(r.title.as_str()));
        cells.push(labels_cell(&r.labels.nodes));
        cells.push(if blocked(r) {
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
            let anchor = r.team.active_cycle.as_ref().map(|cycle| cycle.number);
            let (text, kind) = cycle_short(r.cycle.as_ref(), anchor);
            cells.push(match kind {
                CycleKind::Active => Cell::styled(text, style::green),
                CycleKind::Past | CycleKind::None => Cell::styled(text, style::gray),
                CycleKind::Future => Cell::from(text),
            });
        }
        if assignee {
            let initials = r
                .assignee
                .as_ref()
                .map(|assignee| assignee.initials.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or("-");
            cells.push(Cell::from(initials.chars().take(2).collect::<String>()));
        }
        let state_color = r.state.color.clone();
        cells.push(Cell::styled(r.state.name.as_str(), move |text, on| {
            style::rgb(text, &state_color, on)
        }));
        cells.push(Cell::styled(
            relative_time::ago(r.updated_at.0, now.into(), &chrono::Local),
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
