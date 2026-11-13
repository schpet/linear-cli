//! Where agents read skills from: the directories `setup` installs into and
//! the others a copy of the skill could be read from instead.
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::cli::skill::Agent;
use crate::config::{FileKind, FileSource};
use crate::error::{Error, Result};

/// The skill's name, which is also the name of its directory.
pub const SKILL_NAME: &str = "linear-cli";
pub const SKILL_FILE: &str = "SKILL.md";

/// The agents that read the shared `.agents/skills` directory.
const SHARED: [Agent; 3] = [Agent::Codex, Agent::Opencode, Agent::Pi];

/// The parts of the environment that decide where skills live.
#[derive(Clone, Debug, Default)]
pub struct SkillEnv {
    pub home: Option<PathBuf>,
    pub cwd: Option<PathBuf>,
    pub claude_config_dir: Option<PathBuf>,
    pub codex_home: Option<PathBuf>,
    pub pi_agent_dir: Option<PathBuf>,
    pub xdg_config_home: Option<PathBuf>,
    pub path: Option<OsString>,
}

impl SkillEnv {
    pub fn capture() -> Result<Self> {
        let home = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
        Ok(Self {
            home: directory(home)?,
            // Read only when a project is the target, which reports its absence.
            cwd: std::env::current_dir().ok(),
            claude_config_dir: directory("CLAUDE_CONFIG_DIR")?,
            codex_home: directory("CODEX_HOME")?,
            pi_agent_dir: directory("PI_CODING_AGENT_DIR")?,
            xdg_config_home: directory("XDG_CONFIG_HOME")?,
            path: std::env::var_os("PATH"),
        })
    }

    /// Whether a program named `linear` is on `PATH`, as an agent following
    /// the skill needs it to be.
    pub fn linear_on_path(&self) -> bool {
        let Some(path) = &self.path else {
            return false;
        };
        let names: &[&str] = if cfg!(windows) {
            &["linear.exe", "linear.cmd", "linear.bat"]
        } else {
            &["linear"]
        };
        std::env::split_paths(path)
            .any(|directory| names.iter().any(|name| directory.join(name).is_file()))
    }
}

/// The directory the environment variable `name` holds. Unset and empty both
/// mean it was not given; a relative path would send files somewhere that
/// depends on the working directory, so it is refused.
fn directory(name: &str) -> Result<Option<PathBuf>> {
    match std::env::var_os(name) {
        None => Ok(None),
        Some(value) if value.is_empty() => Ok(None),
        Some(value) => absolute(name, PathBuf::from(value)).map(Some),
    }
}

pub(super) fn absolute(name: &str, path: PathBuf) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(Error::invalid(format!(
            "{name} is not an absolute path: {}",
            path.display()
        ))
        .with_hint(format!("Set {name} to an absolute path, or unset it")))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    User,
    Project,
}

/// How a skill directory relates to what `setup` installs.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Location {
    /// A directory `setup` installs into.
    Canonical,
    /// An agent's own directory, read as well as or instead of a canonical
    /// one. Nothing is installed here; a copy here shadows the skill.
    Alternate,
    /// The same, in a directory between the working directory and the
    /// project root. Reported, and left for the user to deal with.
    Nested,
}

/// One directory a `linear-cli` skill can live in.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Slot {
    /// The skill's directory, `<skills>/linear-cli`.
    pub path: PathBuf,
    /// The agents that read it.
    pub agents: Vec<Agent>,
    pub location: Location,
}

impl Slot {
    fn new(skills: PathBuf, agents: &[Agent], location: Location) -> Self {
        Self {
            path: skills.join(SKILL_NAME),
            agents: agents.to_vec(),
            location,
        }
    }

    pub fn skill_file(&self) -> PathBuf {
        self.path.join(SKILL_FILE)
    }
}

/// Every slot of one scope.
#[derive(Clone, Debug)]
pub struct Layout {
    pub scope: Scope,
    /// The home directory, or the project's root.
    pub root: PathBuf,
    pub slots: Vec<Slot>,
    /// Claude Code's configuration directory, when the scope is the user's
    /// account and that directory exists: the sign Claude Code is set up.
    pub claude_present: bool,
}

impl Layout {
    pub fn user(env: &SkillEnv) -> Result<Self> {
        let home = env.home.clone().ok_or_else(|| {
            Error::new("Could not find your home directory")
                .with_hint("Set HOME, or pass --project to use the current repository")
        })?;
        let claude = env
            .claude_config_dir
            .clone()
            .unwrap_or_else(|| home.join(".claude"));
        let codex = env
            .codex_home
            .clone()
            .unwrap_or_else(|| home.join(".codex"));
        let pi = env
            .pi_agent_dir
            .clone()
            .unwrap_or_else(|| home.join(".pi").join("agent"));
        let config = env
            .xdg_config_home
            .clone()
            .unwrap_or_else(|| home.join(".config"));
        let slots = vec![
            Slot::new(
                home.join(".agents").join("skills"),
                &SHARED,
                Location::Canonical,
            ),
            Slot::new(claude.join("skills"), &[Agent::Claude], Location::Canonical),
            Slot::new(codex.join("skills"), &[Agent::Codex], Location::Alternate),
            Slot::new(pi.join("skills"), &[Agent::Pi], Location::Alternate),
            Slot::new(
                config.join("opencode").join("skills"),
                &[Agent::Opencode],
                Location::Alternate,
            ),
        ];
        Ok(Self {
            scope: Scope::User,
            claude_present: claude.is_dir(),
            root: home,
            slots: distinct(slots),
        })
    }

    pub fn project(env: &SkillEnv, files: &impl FileSource) -> Result<Self> {
        let cwd = env
            .cwd
            .clone()
            .ok_or_else(|| Error::new("Failed to read the working directory"))?;
        let root = project_root(&cwd, files)?;
        let mut slots = Vec::new();
        let mut below_root: Vec<&Path> = cwd
            .ancestors()
            .take_while(|directory| *directory != root)
            .collect();
        below_root.reverse();
        for (directory, canonical, alternate) in
            std::iter::once((root.as_path(), Location::Canonical, Location::Alternate)).chain(
                below_root
                    .into_iter()
                    .map(|directory| (directory, Location::Nested, Location::Nested)),
            )
        {
            let skills = |agent_directory: &str| directory.join(agent_directory).join("skills");
            slots.extend([
                Slot::new(skills(".agents"), &SHARED, canonical),
                Slot::new(skills(".claude"), &[Agent::Claude], canonical),
                Slot::new(skills(".codex"), &[Agent::Codex], alternate),
                Slot::new(skills(".pi"), &[Agent::Pi], alternate),
                Slot::new(skills(".opencode"), &[Agent::Opencode], alternate),
            ]);
        }
        Ok(Self {
            scope: Scope::Project,
            root,
            slots: distinct(slots),
            claude_present: false,
        })
    }

    /// The agents to act for when `named` holds the ones `--agent` gave.
    /// With none named, a project is set up for everyone who may work in it,
    /// and an account for the agents that share a directory plus Claude Code
    /// when it is set up here: writing `~/.claude` on a machine without it
    /// would make it look installed.
    pub fn setup_agents(&self, named: &[Agent]) -> BTreeSet<Agent> {
        if !named.is_empty() {
            return named.iter().copied().collect();
        }
        match self.scope {
            Scope::Project => Agent::ALL.into_iter().collect(),
            Scope::User => SHARED
                .into_iter()
                .chain(self.claude_present.then_some(Agent::Claude))
                .collect(),
        }
    }

    /// The slots read by any of `agents`.
    pub fn slots_for<'a>(&'a self, agents: &'a BTreeSet<Agent>) -> impl Iterator<Item = &'a Slot> {
        self.slots
            .iter()
            .filter(|slot| slot.agents.iter().any(|agent| agents.contains(agent)))
    }
}

/// The nearest directory at or above `cwd` that holds a `.git` entry or a
/// `.jj` directory, or `cwd` itself when none does. A marker that cannot be
/// inspected is an error: guessing would install into a different directory.
pub(super) fn project_root(cwd: &Path, files: &impl FileSource) -> Result<PathBuf> {
    for directory in cwd.ancestors() {
        let kind = |marker: &str| {
            let path = directory.join(marker);
            files.kind(&path).map_err(|error| {
                Error::new(format!(
                    "Could not tell whether {} is a repository: {error}",
                    directory.display()
                ))
                .with_source(error)
            })
        };
        if kind(".git")?.is_some() || kind(".jj")? == Some(FileKind::Directory) {
            return Ok(directory.to_path_buf());
        }
    }
    Ok(cwd.to_path_buf())
}

/// `slots` with those that are the same directory on disk merged into the
/// first, as when one agent's skills directory is a symlink to another's.
/// An agent's own entry that is a symlink to a directory `setup` installs
/// into is that directory too: the agent reads the installed skill through
/// it, so it is not a second copy.
fn distinct(slots: Vec<Slot>) -> Vec<Slot> {
    let mut kept: Vec<(PathBuf, Slot)> = Vec::new();
    for slot in slots {
        let real = resolved(&slot.path);
        let target = match slot.location {
            Location::Canonical => None,
            Location::Alternate | Location::Nested => fs::canonicalize(&slot.path).ok(),
        };
        match kept
            .iter_mut()
            .find(|(path, _)| *path == real || target.as_deref() == Some(path.as_path()))
        {
            Some((_, first)) => {
                for agent in slot.agents {
                    if !first.agents.contains(&agent) {
                        first.agents.push(agent);
                    }
                }
                first.agents.sort();
            }
            None => kept.push((real, slot)),
        }
    }
    kept.into_iter().map(|(_, slot)| slot).collect()
}

/// `path` with the symlinks in its parent directories followed, as far as
/// those directories exist. The last component is kept as written: a skill
/// directory that is itself a symlink is a different thing from its target.
fn resolved(path: &Path) -> PathBuf {
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return path.to_path_buf();
    };
    let mut missing = Vec::new();
    let mut existing = parent;
    loop {
        if let Ok(real) = fs::canonicalize(existing) {
            let mut real = real;
            real.extend(missing.iter().rev());
            real.push(name);
            return real;
        }
        match (existing.parent(), existing.file_name()) {
            (Some(up), Some(component)) => {
                missing.push(component);
                existing = up;
            }
            _ => return path.to_path_buf(),
        }
    }
}
