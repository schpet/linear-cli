//! Pure v3 effective spelling data shared by clap and help rendering.
use super::{OptionMeta, ROUTES, RouteMeta};

const LABEL_WORKSPACE_ONLY: &[&str] = &["--workspace-only"];

pub fn effective_flags(route: &RouteMeta, option: &OptionMeta) -> &'static [&'static str] {
    if route.path == "linear label list" && option.name == "workspace" && !option.global {
        LABEL_WORKSPACE_ONLY
    } else {
        option.flags
    }
}

/// Effective help rows in the renderer's inherited-before-local order.
pub fn effective_help_options(
    route: &RouteMeta,
) -> Vec<(&'static OptionMeta, &'static [&'static str])> {
    let mut rows = Vec::new();
    if route.path == "linear label list" {
        if let Some(root) = ROUTES.iter().find(|candidate| candidate.path == "linear")
            && let Some(workspace) = root
                .local_options
                .iter()
                .find(|option| option.name == "workspace" && option.global)
        {
            rows.push((workspace, effective_flags(route, workspace)));
        }
    } else {
        rows.extend(
            route
                .inherited_global_options
                .iter()
                .map(|option| (option, effective_flags(route, option))),
        );
    }
    rows.extend(
        route
            .local_options
            .iter()
            .map(|option| (option, effective_flags(route, option))),
    );
    rows
}
