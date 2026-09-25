use crate::config::ConfigOptions;

/// Resolve the configured team key once, preserving presence-based tier selection.
pub(crate) fn configured_team_key(options: &ConfigOptions) -> Option<String> {
    match options.team_id() {
        Some(resolved) if !resolved.value().is_empty() => Some(resolved.value().to_uppercase()),
        Some(_) | None => None,
    }
}
