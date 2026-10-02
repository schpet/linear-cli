//! The GitHub autolink action delegates to a waited, shell-free `gh` process.
use std::path::Path;
use std::process::{Command, Stdio};

use crate::commands::team_key::configured_team_key;
use crate::config::StartupConfig;
use crate::error::{AppError, AppErrorKind};

const CONTEXT: &str = "Failed to configure autolinks";

pub fn execute(
    config: &StartupConfig,
    cli_workspace: Option<&str>,
    cwd: &Path,
) -> Result<(), AppError> {
    execute_inner(config, cli_workspace, cwd).map_err(|error| error.with_context(CONTEXT))
}

fn execute_inner(
    config: &StartupConfig,
    cli_workspace: Option<&str>,
    cwd: &Path,
) -> Result<(), AppError> {
    let team = configured_team_key(&config.options).ok_or_else(|| {
        AppError::new(
            AppErrorKind::Validation,
            "Could not determine team id from directory name",
        )
        .with_suggestion("Run `linear config` to set a team.")
    })?;
    let workspace = cli_workspace
        .or_else(|| {
            config
                .options
                .workspace()
                .map(|resolved| resolved.value().as_str())
        })
        .filter(|workspace| !workspace.is_empty())
        .ok_or_else(|| {
            AppError::new(
                AppErrorKind::Validation,
                "workspace is not set via command line, configuration file, or environment",
            )
        })?;
    let mut child = Command::new("gh")
        .args([
            "api".to_owned(),
            "repos/{owner}/{repo}/autolinks".to_owned(),
            "-f".to_owned(),
            format!("key_prefix={team}-"),
            "-f".to_owned(),
            format!("url_template=https://linear.app/{workspace}/issue/{team}-<num>"),
        ])
        .current_dir(cwd)
        .envs(config.child_env.iter())
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| {
            let message = if error.kind() == std::io::ErrorKind::NotFound {
                "Failed to spawn 'gh': entity not found".to_owned()
            } else {
                format!("Failed to spawn 'gh': {error}")
            };
            AppError::new(AppErrorKind::IoProcess, message).with_source(error)
        })?;
    let status = child.wait().map_err(|error| {
        AppError::new(AppErrorKind::IoProcess, "Failed to wait for 'gh'").with_source(error)
    })?;
    if status.success() {
        Ok(())
    } else {
        Err(AppError::new(AppErrorKind::IoProcess, CONTEXT))
    }
}
