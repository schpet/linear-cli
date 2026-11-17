//! Startup reminders for Claude Code and Codex, alongside their other hooks.
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::apply::Files;
use super::plan::{Change, State};
use super::report::Action;
use super::slots::{Layout, Scope};
use crate::cli::skill::HookAgent;
use crate::error::{Error, Result};
use crate::platform::output::Stdout;

impl HookAgent {
    fn command(self) -> &'static str {
        match self {
            Self::Claude => "linear skill session-start",
            Self::Codex => "linear skill session-start --agent codex",
        }
    }

    fn skill_name(self) -> &'static str {
        match self {
            Self::Claude => "/linear-cli",
            Self::Codex => "$linear-cli",
        }
    }
}

pub fn session_start(agent: HookAgent) -> Result<()> {
    let reminder = format!(
        "For tasks involving Linear or an issue ID such as ENG-123, load the \
        {} skill before your first `linear` command. If the skill is not available, run \
        `linear skill show` on its own and read the whole guide. The guide matches the installed \
        CLI version and includes your template preferences.\n",
        agent.skill_name()
    );
    let stdout = Stdout::new();
    stdout.write(reminder.as_bytes())?;
    stdout.flush()
}

// Parse only the structure we edit, retaining all other settings and hook fields.
// Nulls and wrong types in these fields are errors, even during --force.
#[derive(Default, Deserialize, Serialize)]
struct Settings {
    #[serde(default, skip_serializing_if = "Hooks::is_empty")]
    hooks: Hooks,
    #[serde(flatten)]
    other: Map<String, Value>,
}

#[derive(Default, Deserialize, Serialize)]
struct Hooks {
    #[serde(
        rename = "SessionStart",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    session_start: Vec<Group>,
    #[serde(flatten)]
    other: Map<String, Value>,
}

impl Hooks {
    fn is_empty(&self) -> bool {
        self.session_start.is_empty() && self.other.is_empty()
    }
}

#[derive(Deserialize, Serialize)]
struct Group {
    #[serde(
        default,
        deserialize_with = "matcher",
        skip_serializing_if = "Option::is_none"
    )]
    matcher: Option<String>,
    hooks: Vec<Map<String, Value>>,
    #[serde(flatten)]
    other: Map<String, Value>,
}

fn matcher<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error> {
    String::deserialize(deserializer).map(Some)
}

fn entry(agent: HookAgent) -> Map<String, Value> {
    Map::from_iter([
        ("type".to_owned(), Value::from("command")),
        ("command".to_owned(), Value::from(agent.command())),
        ("timeout".to_owned(), Value::from(5)),
    ])
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    #[serde(serialize_with = "display_path")]
    pub path: PathBuf,
    pub agent: HookAgent,
    pub state: State,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change: Option<Change>,
    pub reason: Option<String>,
}

fn display_path<S: serde::Serializer>(
    path: &Path,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error> {
    serializer.serialize_str(&path.display().to_string())
}

#[derive(Eq, PartialEq)]
struct Snapshot {
    target: PathBuf,
    bytes: Option<Vec<u8>>,
}

fn read(path: &Path, agent: HookAgent) -> Result<Snapshot> {
    match fs::symlink_metadata(path) {
        Ok(_) => {
            // Follow a settings-file symlink, then replace its target atomically.
            // The symlink itself belongs to the user's configuration manager.
            let target = fs::canonicalize(path)
                .map_err(|error| file_error(agent, "resolve", path, error))?;
            let bytes =
                fs::read(&target).map_err(|error| file_error(agent, "read", path, error))?;
            Ok(Snapshot {
                target,
                bytes: Some(bytes),
            })
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            // Resolve existing parent directories too: a missing settings file
            // can still be reached through a symlinked configuration directory.
            let mut ancestor = path;
            let mut missing = Vec::new();
            let target = loop {
                match fs::canonicalize(ancestor) {
                    Ok(mut real) => {
                        real.extend(missing.into_iter().rev());
                        break real;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        match fs::symlink_metadata(ancestor) {
                            Ok(_) => return Err(file_error(agent, "resolve", path, error)),
                            Err(absent) if absent.kind() == std::io::ErrorKind::NotFound => {}
                            Err(error) => return Err(file_error(agent, "inspect", path, error)),
                        }
                        missing.push(
                            ancestor
                                .file_name()
                                .ok_or_else(|| file_error(agent, "resolve", path, error))?,
                        );
                        ancestor = ancestor
                            .parent()
                            .expect("an absolute file path has a parent");
                    }
                    Err(error) => return Err(file_error(agent, "resolve", path, error)),
                }
            };
            Ok(Snapshot {
                target,
                bytes: None,
            })
        }
        Err(error) => Err(file_error(agent, "inspect", path, error)),
    }
}

pub struct Plan {
    pub report: Report,
    before: Snapshot,
    after: Option<Vec<u8>>,
    scope: Scope,
}

fn settings_path(layout: &Layout, agent: HookAgent) -> PathBuf {
    match agent {
        HookAgent::Claude => layout.claude_settings.clone(),
        HookAgent::Codex => layout.codex_hooks.clone(),
    }
}

pub fn status(layout: &Layout, agent: HookAgent) -> Report {
    match plan(layout, agent, None) {
        Ok(plan) => plan.report,
        Err(error) => Report {
            path: settings_path(layout, agent),
            agent,
            state: State::Unreadable,
            change: None,
            reason: Some(error.to_string()),
        },
    }
}

pub fn plan(layout: &Layout, agent: HookAgent, action: Option<Action>) -> Result<Plan> {
    let path = settings_path(layout, agent);
    let before = read(&path, agent)?;
    if layout.scope == Scope::Project {
        let root = fs::canonicalize(&layout.root)
            .map_err(|error| file_error(agent, "resolve project root for", &layout.root, error))?;
        if !before.target.starts_with(root) {
            return Err(Error::new(format!("Project hook settings at {} resolve outside the project", path.display()))
                .with_hint("Keep project hook settings inside the repository, or use account setup without --project"));
        }
    }
    let mut settings: Settings = match &before.bytes {
        Some(bytes) => serde_json::from_slice(bytes).map_err(|error| {
            Error::new(format!(
                "Invalid {} hook settings at {}: {error}",
                agent.agent().title(),
                path.display()
            ))
            .with_hint("Fix the settings file and retry; --force never replaces agent settings")
            .with_source(error)
        })?,
        None => Settings::default(),
    };
    let owned = entry(agent);
    let mut found = false;
    let mut edited = false;
    for group in &settings.hooks.session_start {
        for hook in &group.hooks {
            if hook.get("type").and_then(Value::as_str) == Some("command")
                && hook.get("command").and_then(Value::as_str) == Some(agent.command())
            {
                found = true;
                edited |= *hook != owned || group.matcher.as_deref().is_some_and(|s| !s.is_empty());
            }
        }
    }
    let state = if edited {
        State::Foreign
    } else if found {
        State::Current
    } else {
        State::Missing
    };
    let change = action.map(|action| match (action, edited, found) {
        (Action::Setup, true, _) => Change::Conflict,
        (Action::Setup, false, true) => Change::Unchanged,
        (Action::Setup, false, false) => Change::Created,
        (Action::Remove, true, _) => Change::Kept,
        (Action::Remove, false, true) => Change::Removed,
        (Action::Remove, false, false) => Change::Absent,
    });
    let reason =
        edited.then(|| "linear's SessionStart hook was edited; it was left alone".to_owned());
    let mut after = None;
    match change {
        Some(Change::Created) => {
            settings.hooks.session_start.push(Group {
                matcher: None,
                hooks: vec![owned],
                other: Map::new(),
            });
        }
        Some(Change::Removed) => {
            settings.hooks.session_start.retain_mut(|group| {
                let before = group.hooks.len();
                group.hooks.retain(|hook| *hook != owned);
                group.hooks.len() == before || !group.hooks.is_empty()
            });
        }
        _ => {}
    }
    if change.is_some_and(Change::mutates) {
        let mut bytes =
            serde_json::to_vec_pretty(&settings).expect("Agent settings contain only JSON values");
        bytes.push(b'\n');
        after = Some(bytes);
    }
    Ok(Plan {
        report: Report {
            path,
            agent,
            state,
            change,
            reason,
        },
        before,
        after,
        scope: layout.scope,
    })
}

impl Plan {
    pub fn apply(&mut self, files: &impl Files, stopped: bool) {
        let Some(after) = &self.after else {
            return;
        };
        if stopped {
            self.report.change = Some(Change::NotRun);
            self.report.reason = Some("an earlier filesystem operation failed".to_owned());
            return;
        }
        let result = (|| {
            if read(&self.report.path, self.report.agent)? != self.before {
                self.report.change = Some(Change::Conflict);
                return Err(Error::new(
                    "Agent hook settings changed while this command was running",
                ));
            }
            let parent = self
                .before
                .target
                .parent()
                .expect("agent hook settings have a parent directory");
            files.create_dir_all(parent).map_err(|error| {
                file_error(self.report.agent, "create directory for", parent, error)
            })?;
            match (self.before.bytes.is_some(), self.scope) {
                (true, _) => files.write_preserving(&self.before.target, after),
                (false, Scope::User) => files.write_private(&self.before.target, after),
                (false, Scope::Project) => files.write(&self.before.target, after),
            }
            .map_err(|error| file_error(self.report.agent, "write", &self.report.path, error))
        })();
        if let Err(error) = result {
            if self.report.change != Some(Change::Conflict) {
                self.report.change = Some(Change::Failed);
            }
            self.report.reason = Some(error.to_string());
        }
    }
}

fn file_error(agent: HookAgent, action: &str, path: &Path, error: std::io::Error) -> Error {
    Error::new(format!(
        "Failed to {action} {} hook settings at {}: {error}",
        agent.agent().title(),
        path.display()
    ))
    .with_source(error)
}

#[cfg(test)]
mod tests;
