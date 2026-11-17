//! Prepare release metadata without committing, tagging, or publishing.
use std::error::Error;
use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::process::Command;

use serde::Deserialize;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

trait Runner {
    fn run(&mut self, root: &Path, program: &str, args: &[OsString]) -> Result<String>;
}

struct Process;

impl Runner for Process {
    fn run(&mut self, root: &Path, program: &str, args: &[OsString]) -> Result<String> {
        let output = Command::new(program)
            .args(args)
            .current_dir(root)
            // Git's configured worktree can belong to another jj checkout.
            .env("GIT_WORK_TREE", root)
            .output()
            .map_err(|error| io::Error::other(format!("Failed to run {program}: {error}")))?;
        if !output.status.success() {
            return Err(io::Error::other(format!(
                "{program} failed ({}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ))
            .into());
        }
        Ok(String::from_utf8(output.stdout)?)
    }
}

#[derive(Deserialize)]
struct Plan {
    announcement_tag: String,
    announcement_title: String,
    announcement_is_prerelease: bool,
    releases: Vec<Release>,
}

#[derive(Deserialize)]
struct Release {
    app_version: String,
}

pub fn run(root: &Path, requested: &str) -> Result<()> {
    let version = prepare(root, requested, &mut Process)?;
    println!("Prepared v{version}; run just check before tagging.");
    Ok(())
}

fn prepare(root: &Path, requested: &str, runner: &mut impl Runner) -> Result<String> {
    let version = if requested == "latest" {
        runner
            .run(root, "changelog", &["version".into(), "latest".into()])?
            .trim()
            .to_owned()
    } else {
        requested.to_owned()
    };
    if version.is_empty() {
        return Err(io::Error::other("No release version was selected").into());
    }
    runner.run(root, "changelog", &["entry".into(), version.clone().into()])?;

    // Detect stale svbump builds before touching the actual workspace manifest.
    let probe = tempfile::Builder::new()
        .prefix("linear-release-")
        .suffix(".toml")
        .tempfile()?
        .into_temp_path();
    std::fs::copy(root.join("Cargo.toml"), &probe)?;
    let selector = OsString::from("workspace.package.version");
    runner.run(
        root,
        "svbump",
        &[
            "write".into(),
            version.clone().into(),
            selector.clone(),
            probe.as_os_str().to_owned(),
        ],
    )?;
    let actual = runner.run(
        root,
        "svbump",
        &[
            "read".into(),
            selector.clone(),
            probe.as_os_str().to_owned(),
        ],
    )?;
    if actual.trim() != version {
        return Err(io::Error::other(format!(
            "svbump wrote '{}' instead of '{version}'; update svbump before releasing",
            actual.trim()
        ))
        .into());
    }
    runner.run(
        root,
        "svbump",
        &[
            "write".into(),
            version.clone().into(),
            selector,
            "Cargo.toml".into(),
        ],
    )?;
    runner.run(root, "cargo", &["update".into(), "--workspace".into()])?;
    let plan = runner.run(
        root,
        "dist",
        &[
            "plan".into(),
            "--tag".into(),
            format!("v{version}").into(),
            "--output-format=json".into(),
        ],
    )?;
    validate_plan(&version, &plan)?;
    Ok(version)
}

fn validate_plan(version: &str, json: &str) -> Result<()> {
    let plan: Plan = serde_json::from_str(json)?;
    let without_build = version.split_once('+').map_or(version, |(base, _)| base);
    if plan.announcement_tag != format!("v{version}")
        || !plan
            .announcement_title
            .starts_with(&format!("{version} - "))
        || plan.announcement_is_prerelease != without_build.contains('-')
        || plan.releases.is_empty()
        || plan
            .releases
            .iter()
            .any(|release| release.app_version != version)
    {
        return Err(io::Error::other(format!(
            "cargo-dist release metadata does not match v{version} and its dated changelog heading"
        ))
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
