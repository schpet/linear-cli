//! What `setup`, `status` and `remove` print: lines for a person, or one
//! JSON object.
use std::path::Path;

use serde::Serialize;

use super::plan::{Change, Item, State, Step, titles};
use super::slots::{Layout, Location, Scope};
use crate::cli::skill::Agent;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Setup,
    Remove,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusJson<'a> {
    scope: Scope,
    root: String,
    items: Vec<ItemJson<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ItemJson<'a> {
    path: String,
    agents: &'a [Agent],
    location: Location,
    state: State,
    reason: Option<&'a str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ChangesJson<'a> {
    scope: Scope,
    root: String,
    action: Action,
    dry_run: bool,
    steps: Vec<StepJson<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StepJson<'a> {
    path: String,
    agents: &'a [Agent],
    location: Location,
    state: State,
    change: Change,
    reason: Option<&'a str>,
    backup_path: Option<String>,
}

fn text(path: &Path) -> String {
    path.display().to_string()
}

pub fn status_json(layout: &Layout, items: &[Item]) -> impl Serialize {
    StatusJson {
        scope: layout.scope,
        root: text(&layout.root),
        items: items
            .iter()
            .map(|item| ItemJson {
                path: text(&item.slot.path),
                agents: &item.slot.agents,
                location: item.slot.location,
                state: item.seen.state,
                reason: item.note.as_deref(),
            })
            .collect(),
    }
}

pub fn changes_json<'a>(
    layout: &Layout,
    action: Action,
    dry_run: bool,
    steps: &'a [Step],
) -> impl Serialize + use<'a> {
    ChangesJson {
        scope: layout.scope,
        root: text(&layout.root),
        action,
        dry_run,
        steps: steps
            .iter()
            .map(|step| StepJson {
                path: text(&step.slot.path),
                agents: &step.slot.agents,
                location: step.slot.location,
                state: step.seen.state,
                change: step.change,
                reason: step.note.as_deref(),
                backup_path: step.backup.as_deref().map(text),
            })
            .collect(),
    }
}

fn state_word(state: State) -> &'static str {
    match state {
        State::Current => "current",
        State::Outdated => "outdated",
        State::Missing => "missing",
        State::Foreign => "foreign",
        State::Unreadable => "unreadable",
    }
}

fn change_word(change: Change, dry_run: bool) -> &'static str {
    match (change, dry_run) {
        (Change::Created, false) => "created",
        (Change::Created, true) => "would create",
        (Change::Updated, false) => "updated",
        (Change::Updated, true) => "would update",
        (Change::Unchanged, _) => "unchanged",
        (Change::Replaced, false) => "replaced",
        (Change::Replaced, true) => "would replace",
        (Change::MovedAside, false) => "moved aside",
        (Change::MovedAside, true) => "would move aside",
        (Change::Conflict, _) => "conflict",
        (Change::Removed, false) => "removed",
        (Change::Removed, true) => "would remove",
        (Change::Absent, _) => "not installed",
        (Change::Kept, _) => "kept",
        (Change::Failed, _) => "failed",
        (Change::NotRun, _) => "not run",
    }
}

/// `path` as a person would write it: under `~` when it is in `home`.
fn short(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) => Path::new("~").join(rest).display().to_string(),
        None => text(path),
    }
}

/// One line per slot, columns aligned, with what needs saying indented
/// beneath it.
fn lines(rows: &[(String, &str, String, Vec<String>)]) -> String {
    let path_width = rows.iter().map(|row| row.0.len()).max().unwrap_or_default();
    let word_width = rows.iter().map(|row| row.1.len()).max().unwrap_or_default();
    let mut out = String::new();
    for (path, word, agents, details) in rows {
        out.push_str(&format!(
            "{path:path_width$}  {word:word_width$}  {agents}\n"
        ));
        for detail in details {
            out.push_str(&format!("  {detail}\n"));
        }
    }
    out
}

pub fn status_text(items: &[Item], home: Option<&Path>) -> String {
    let rows: Vec<_> = items
        .iter()
        .map(|item| {
            (
                short(&item.slot.path, home),
                state_word(item.seen.state),
                titles(&item.slot.agents),
                item.note.iter().cloned().collect(),
            )
        })
        .collect();
    lines(&rows)
}

pub fn changes_text(steps: &[Step], dry_run: bool, home: Option<&Path>) -> String {
    let rows: Vec<_> = steps
        .iter()
        .map(|step| {
            let backup = step.backup.as_deref().map(|backup| {
                let verb = if dry_run {
                    "would move it to"
                } else {
                    "moved to"
                };
                format!("{verb} {}", short(backup, home))
            });
            (
                short(&step.slot.path, home),
                change_word(step.change, dry_run),
                titles(&step.slot.agents),
                step.note.iter().cloned().chain(backup).collect(),
            )
        })
        .collect();
    lines(&rows)
}
