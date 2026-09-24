use std::path::PathBuf;

use super::dotenv::ConfigFailure;
use super::source::{ConfigInputs, GitProbeResult, GitRootProbe, OsFamily, lexical};

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigPaths {
    pub global: Vec<ConfigCandidate>,
    pub project: Vec<ConfigCandidate>,
}

fn truthy(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.is_empty())
}

pub fn discover_config_paths(
    inputs: &ConfigInputs,
    git: &impl GitRootProbe,
) -> Result<ConfigPaths, ConfigFailure> {
    let mut global = Vec::new();
    let base = match inputs.os {
        OsFamily::Unix => truthy(inputs.env("XDG_CONFIG_HOME"))
            .map(PathBuf::from)
            .or_else(|| truthy(inputs.env("HOME")).map(|home| PathBuf::from(home).join(".config"))),
        OsFamily::Windows => truthy(inputs.env("APPDATA")).map(PathBuf::from),
    };
    if let Some(base) = base {
        global.push(ConfigCandidate {
            tier: CandidateTier::Global,
            path: lexical(&base.join("linear").join("linear.toml")),
        });
    }
    let mut project = ["linear.toml", ".linear.toml"]
        .into_iter()
        .map(|name| ConfigCandidate {
            tier: CandidateTier::Project,
            path: lexical(&inputs.cwd.join(name)),
        })
        .collect::<Vec<_>>();
    // Unlike dotenv's probe, the source config loader ignores process success.
    match git.probe() {
        GitProbeResult::Completed { stdout, .. } => {
            let root = stdout.trim();
            for suffix in ["linear.toml", ".linear.toml", ".config/linear.toml"] {
                let path = if root.is_empty() {
                    inputs.cwd.join(suffix)
                } else {
                    PathBuf::from(root).join(suffix)
                };
                project.push(ConfigCandidate {
                    tier: CandidateTier::Project,
                    path: lexical(&path),
                });
            }
        }
        GitProbeResult::Failed(error) => return Err(ConfigFailure::GitProbe(error)),
        GitProbeResult::SpawnFailure => {}
    }
    Ok(ConfigPaths { global, project })
}
