use std::path::{Path, PathBuf};

use super::source::{ConfigInputs, OsFamily, lexical};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateTier {
    Global,
    Project,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigCandidate {
    pub tier: CandidateTier,
    pub path: PathBuf,
}

/// Candidate config files per tier, in lookup order. The first one that
/// exists is used.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigPaths {
    pub global: Vec<ConfigCandidate>,
    pub project: Vec<ConfigCandidate>,
}

fn nonempty(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.is_empty())
}

/// The global config lives under `$XDG_CONFIG_HOME` (or `~/.config`), or
/// `%APPDATA%` on Windows. Project config is looked up in the working
/// directory, then at the repository root.
pub fn discover_config_paths(inputs: &ConfigInputs, repo_root: Option<&Path>) -> ConfigPaths {
    let base = match inputs.os {
        OsFamily::Unix => nonempty(inputs.env("XDG_CONFIG_HOME"))
            .map(PathBuf::from)
            .or_else(|| {
                nonempty(inputs.env("HOME")).map(|home| PathBuf::from(home).join(".config"))
            }),
        OsFamily::Windows => nonempty(inputs.env("APPDATA")).map(PathBuf::from),
    };
    let global = base
        .map(|base| ConfigCandidate {
            tier: CandidateTier::Global,
            path: lexical(&base.join("linear").join("linear.toml")),
        })
        .into_iter()
        .collect();
    let project = |path: PathBuf| ConfigCandidate {
        tier: CandidateTier::Project,
        path: lexical(&path),
    };
    let mut candidates = ["linear.toml", ".linear.toml"]
        .into_iter()
        .map(|name| project(inputs.cwd.join(name)))
        .collect::<Vec<_>>();
    if let Some(root) = repo_root {
        for name in ["linear.toml", ".linear.toml", ".config/linear.toml"] {
            let candidate = project(root.join(name));
            if !candidates.contains(&candidate) {
                candidates.push(candidate);
            }
        }
    }
    ConfigPaths {
        global,
        project: candidates,
    }
}

#[cfg(test)]
mod tests;
