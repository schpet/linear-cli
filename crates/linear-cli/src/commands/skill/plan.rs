//! What is in each skill directory, and what `setup` and `remove` would do
//! about it. Nothing here changes a file.
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::slots::{Layout, Location, SKILL_FILE, SKILL_NAME, Slot};
use crate::cli::skill::Agent;
use crate::config::absent;

/// The directory, beside a skills directory, that `setup --force` moves
/// conflicting skills into. Agents do not read skills from it.
pub const BACKUPS: &str = "linear-cli-skill-backups";

/// The skill file this version installs, and the ones earlier versions did.
#[derive(Clone, Copy, Debug)]
pub struct Stubs<'a> {
    pub current: &'a str,
    /// Every stub a released version installed that differs from `current`.
    /// A file equal to one of them is ours to update.
    pub previous: &'a [&'a str],
}

impl Stubs<'static> {
    pub const SHIPPED: Self = Self {
        current: include_str!("stub.md"),
        previous: &[],
    };
}

/// The frontmatter lines of the skill this repository distributed before
/// `linear skill setup` existed, unchanged from its first release.
const OLD_SKILL: [&str; 2] = [
    "name: linear-cli\n",
    "description: Manage Linear issues from the command line using the linear cli.",
];

/// The line every stub carries in its frontmatter.
const STUB_MARKER: &str = "\n  linear-cli-stub: ";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    /// The stub this version installs.
    Current,
    /// A stub an earlier version installed.
    Outdated,
    /// Nothing is there.
    Missing,
    /// Something `setup` did not install, or installed and someone edited.
    Foreign,
    Unreadable,
}

/// Why a slot is foreign or unreadable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Reason {
    OldSkill,
    EditedStub,
    Symlink,
    NoSkillFile,
    NotADirectory,
    Other,
    Unreadable(String),
}

impl Reason {
    fn describe(&self) -> String {
        match self {
            Self::OldSkill => {
                "the linear-cli skill from before `linear skill setup` existed".to_owned()
            }
            Self::EditedStub => "linear's skill, edited since it was installed".to_owned(),
            Self::Symlink => "a symlink to a linear-cli skill kept elsewhere".to_owned(),
            Self::NoSkillFile => format!("a directory with no {SKILL_FILE}"),
            Self::NotADirectory => "not a directory".to_owned(),
            Self::Other => "a linear-cli skill this command did not install".to_owned(),
            Self::Unreadable(error) => format!("could not be read: {error}"),
        }
    }
}

/// What one slot holds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Seen {
    pub state: State,
    pub reason: Option<Reason>,
    /// The slot is a symlink to a directory holding the current stub. It
    /// works, and it is someone else's to change or remove.
    pub symlink: bool,
    /// The slot's directory exists, though it may be empty.
    pub directory: bool,
}

impl Seen {
    fn new(state: State) -> Self {
        Self {
            state,
            reason: None,
            symlink: false,
            directory: true,
        }
    }

    fn foreign(reason: Reason) -> Self {
        Self {
            reason: Some(reason),
            ..Self::new(State::Foreign)
        }
    }

    fn unreadable(error: &io::Error) -> Self {
        Self {
            reason: Some(Reason::Unreadable(error.to_string())),
            ..Self::new(State::Unreadable)
        }
    }
}

/// Looks at the skill directory `path`. A symlink there is never followed
/// for anything but reading.
pub fn inspect(path: &Path, stubs: Stubs) -> Seen {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if absent(&error) => {
            return Seen {
                directory: false,
                ..Seen::new(State::Missing)
            };
        }
        Err(error) => return Seen::unreadable(&error),
    };
    let file = path.join(SKILL_FILE);
    if metadata.file_type().is_symlink() {
        return match fs::read(&file) {
            Ok(bytes) if bytes == stubs.current.as_bytes() => Seen {
                symlink: true,
                ..Seen::new(State::Current)
            },
            Ok(_) | Err(_) => Seen::foreign(Reason::Symlink),
        };
    }
    if !metadata.is_dir() {
        return Seen::foreign(Reason::NotADirectory);
    }
    match fs::symlink_metadata(&file) {
        Ok(metadata) if metadata.is_file() => match fs::read(&file) {
            Ok(bytes) => classify(&bytes, stubs),
            Err(error) => Seen::unreadable(&error),
        },
        Ok(_) => Seen::foreign(Reason::Other),
        Err(error) if absent(&error) => match is_empty(path) {
            // An empty directory is what an interrupted install leaves.
            Ok(true) => Seen::new(State::Missing),
            Ok(false) => Seen::foreign(Reason::NoSkillFile),
            Err(error) => Seen::unreadable(&error),
        },
        Err(error) => Seen::unreadable(&error),
    }
}

fn is_empty(directory: &Path) -> io::Result<bool> {
    Ok(fs::read_dir(directory)?.next().is_none())
}

fn classify(bytes: &[u8], stubs: Stubs) -> Seen {
    if bytes == stubs.current.as_bytes() {
        return Seen::new(State::Current);
    }
    if stubs
        .previous
        .iter()
        .any(|previous| bytes == previous.as_bytes())
    {
        return Seen::new(State::Outdated);
    }
    let text = String::from_utf8_lossy(bytes);
    Seen::foreign(if text.contains(STUB_MARKER) {
        Reason::EditedStub
    } else if OLD_SKILL.iter().all(|line| text.contains(line)) {
        Reason::OldSkill
    } else {
        Reason::Other
    })
}

/// What a command did, or would do, to one slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Change {
    Created,
    Updated,
    Unchanged,
    /// A foreign skill was moved to a backup and the stub installed.
    Replaced,
    /// A foreign skill in a directory nothing is installed into was moved to
    /// a backup.
    MovedAside,
    /// Left alone: something is there that is not ours to change.
    Conflict,
    Removed,
    /// Nothing was there to remove.
    Absent,
    /// Not ours to remove.
    Kept,
    /// The change was attempted and the filesystem refused it.
    Failed,
    /// An earlier failure stopped the command before this change.
    NotRun,
}

impl Change {
    /// Whether applying it writes, moves or deletes something.
    pub fn mutates(self) -> bool {
        match self {
            Self::Created | Self::Updated | Self::Replaced | Self::MovedAside | Self::Removed => {
                true
            }
            Self::Unchanged
            | Self::Conflict
            | Self::Absent
            | Self::Kept
            | Self::Failed
            | Self::NotRun => false,
        }
    }

    /// Whether the command should exit unsuccessfully because of it.
    pub fn failed(self) -> bool {
        matches!(self, Self::Conflict | Self::Failed | Self::NotRun)
    }
}

/// One slot's part in a `setup` or `remove`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Step {
    pub slot: Slot,
    pub seen: Seen,
    pub change: Change,
    /// What to tell the reader about the slot, when its state needs it.
    pub note: Option<String>,
    /// Where `--force` moved, or would move, what was there.
    pub backup: Option<PathBuf>,
}

impl Step {
    fn new(slot: &Slot, seen: Seen, change: Change) -> Self {
        let note = note(slot, &seen);
        Self {
            slot: slot.clone(),
            seen,
            change,
            note,
            backup: None,
        }
    }
}

/// A slot `status` reports.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Item {
    pub slot: Slot,
    pub seen: Seen,
    pub note: Option<String>,
}

/// What a reader should know about a slot beyond its state.
fn note(slot: &Slot, seen: &Seen) -> Option<String> {
    let what = seen.reason.as_ref().map(Reason::describe);
    let shadow = match (slot.location, seen.state) {
        (Location::Canonical, _) | (_, State::Missing) => None,
        (Location::Alternate, State::Current) => {
            Some("a second copy, which the agent may list twice".to_owned())
        }
        (Location::Alternate, _) => Some(format!(
            "{} reads this copy as well as or instead of the installed skill",
            titles(&slot.agents)
        )),
        (Location::Nested, _) => Some(
            "agents started in this directory read it as well as or instead of the skill at the \
             project root; `--force` does not move it"
                .to_owned(),
        ),
    };
    let link = (seen.symlink && slot.location == Location::Canonical)
        .then(|| "a symlink to a copy kept elsewhere".to_owned());
    let parts: Vec<String> = [what, link, shadow].into_iter().flatten().collect();
    (!parts.is_empty()).then(|| parts.join("; "))
}

pub fn titles(agents: &[Agent]) -> String {
    agents
        .iter()
        .map(|agent| agent.title())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Every slot worth showing: the ones `setup` installs into, and any other
/// that holds something.
pub fn status(layout: &Layout, stubs: Stubs) -> Vec<Item> {
    layout
        .slots
        .iter()
        .filter_map(|slot| {
            let seen = inspect(&slot.path, stubs);
            (slot.location == Location::Canonical || seen.state != State::Missing).then(|| Item {
                note: note(slot, &seen),
                slot: slot.clone(),
                seen,
            })
        })
        .collect()
}

/// Where `--force` would move `slot`: beside its skills directory, under
/// `stamp`. `apply` picks the next free name if this one is taken.
pub fn backup_directory(slot: &Slot, stamp: &str) -> Option<PathBuf> {
    let skills = slot.path.parent()?;
    Some(skills.parent()?.join(BACKUPS).join(stamp))
}

/// What `setup` would do for `agents`. With `force`, a foreign skill is
/// moved aside instead of being a conflict; one that could not be read never
/// is, and neither is one in a nested directory.
pub fn setup(
    layout: &Layout,
    agents: &BTreeSet<Agent>,
    force: bool,
    stubs: Stubs,
    stamp: &str,
) -> Vec<Step> {
    layout
        .slots_for(agents)
        .filter_map(|slot| {
            let seen = inspect(&slot.path, stubs);
            let forced = |change| {
                let mut step = Step::new(slot, seen.clone(), Change::Conflict);
                if force && let Some(backup) = backup_directory(slot, stamp) {
                    step.change = change;
                    step.backup = Some(backup.join(SKILL_NAME));
                }
                step
            };
            let plain = |change| Step::new(slot, seen.clone(), change);
            match (slot.location, seen.state) {
                (Location::Canonical, State::Current) => Some(plain(Change::Unchanged)),
                (Location::Canonical, State::Outdated) => Some(plain(Change::Updated)),
                (Location::Canonical, State::Missing) => Some(plain(Change::Created)),
                (Location::Canonical, State::Foreign) => Some(forced(Change::Replaced)),
                // A second, identical copy is harmless; `status` shows it.
                (Location::Alternate | Location::Nested, State::Missing | State::Current) => None,
                (Location::Alternate, State::Outdated | State::Foreign) => {
                    Some(forced(Change::MovedAside))
                }
                (Location::Nested, State::Outdated | State::Foreign) | (_, State::Unreadable) => {
                    Some(plain(Change::Conflict))
                }
            }
        })
        .collect()
}

/// What `remove` would do for `agents`: delete the stubs `setup` installed
/// and nothing else.
pub fn remove(layout: &Layout, agents: &BTreeSet<Agent>, stubs: Stubs) -> Vec<Step> {
    layout
        .slots_for(agents)
        .filter_map(|slot| {
            let seen = inspect(&slot.path, stubs);
            let change = match (slot.location, seen.state) {
                (Location::Nested, _) => return None,
                (Location::Canonical, State::Missing) => Change::Absent,
                (Location::Alternate, State::Missing) => return None,
                (_, State::Current | State::Outdated) if !seen.symlink => Change::Removed,
                (_, State::Current | State::Outdated | State::Foreign) => Change::Kept,
                (_, State::Unreadable) => Change::Conflict,
            };
            Some(Step::new(slot, seen, change))
        })
        .collect()
}
