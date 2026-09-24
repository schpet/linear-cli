//! Pure v3 effective spelling data shared by clap and the later help cutover.
use super::{OptionMeta, ROUTES, RouteMeta};

const LABEL_WORKSPACE_ONLY: &[&str] = &["--workspace-only"];

pub fn effective_flags(route: &RouteMeta, option: &OptionMeta) -> &'static [&'static str] {
    if route.path == "linear label list" && option.name == "workspace" && !option.global {
        LABEL_WORKSPACE_ONLY
    } else {
        option.flags
    }
}

/// Proposed help rows; the production renderer adopts these in R01C2.
pub fn effective_help_options(
    route: &RouteMeta,
) -> Vec<(&'static OptionMeta, &'static [&'static str])> {
    let mut rows = route
        .local_options
        .iter()
        .map(|option| (option, effective_flags(route, option)))
        .collect::<Vec<_>>();
    if route.path == "linear label list" {
        if let Some(root) = ROUTES.iter().find(|candidate| candidate.path == "linear")
            && let Some(workspace) = root
                .local_options
                .iter()
                .find(|option| option.name == "workspace" && option.global)
        {
            rows.push((workspace, workspace.flags));
        }
    } else {
        rows.extend(
            route
                .inherited_global_options
                .iter()
                .map(|option| (option, option.flags)),
        );
    }
    rows
}
