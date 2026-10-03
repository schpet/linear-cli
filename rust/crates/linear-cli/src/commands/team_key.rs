use crate::config::ConfigOptions;
use crate::ctx::Ctx;
use crate::error::{Error, Result};

/// The default team's key from config, if one is set.
pub(crate) fn configured_team_key(options: &ConfigOptions) -> Option<String> {
    options.team_key().map(str::to_owned)
}

/// The team named by `explicit` (a `--team` flag), else the configured team.
pub(crate) fn team_or_configured(ctx: &Ctx, explicit: Option<&str>) -> Result<String> {
    explicit
        .map(str::to_owned)
        .or_else(|| configured_team_key(ctx.options()))
        .ok_or_else(no_team)
}

/// No `--team` was given and no default team is configured.
pub(crate) fn no_team() -> Error {
    Error::new("No team given and no default team configured")
        .with_hint("Pass --team <key, name, or ID>, or run `linear config` to set a default team.")
}
