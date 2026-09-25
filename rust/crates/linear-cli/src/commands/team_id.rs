use crate::app::AppContext;
use crate::commands::team_key::configured_team_key;
use crate::error::{AppError, AppErrorKind};

pub fn render(context: &AppContext<'_>) -> Result<String, AppError> {
    match configured_team_key(&context.config()?.options) {
        Some(key) => Ok(format!("{key}\n")),
        None => Err(
            AppError::new(AppErrorKind::Validation, "No team id configured")
                .with_context("Failed to get team id")
                .with_suggestion("Run `linear config` to set a team."),
        ),
    }
}
