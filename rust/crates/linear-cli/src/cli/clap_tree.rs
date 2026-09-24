//! Shadow clap command tree. The live parser remains in `parser` until R01C.

use clap::{ArgMatches, Command};

use super::{ROUTES, Route, RouteMeta};
use crate::error::{AppError, AppErrorKind};

fn invariant(message: impl Into<String>) -> AppError {
    AppError::new(AppErrorKind::Invariant, message)
}

fn unique_route(path: &str) -> Result<&'static RouteMeta, AppError> {
    let mut found = ROUTES.iter().filter(|route| route.path == path);
    match (found.next(), found.next()) {
        (Some(route), None) => Ok(route),
        (None, _) => Err(invariant(format!(
            "clap route missing from inventory: {path}"
        ))),
        (Some(_), Some(_)) => Err(invariant(format!(
            "clap route duplicated in inventory: {path}"
        ))),
    }
}

fn build_route(route: &'static RouteMeta) -> Result<Command, AppError> {
    let mut command = Command::new(route.name)
        .about(route.description)
        .hide(route.hidden);
    for alias in route.aliases {
        command = command.alias(alias);
    }
    for child_name in route.children {
        let child_path = format!("{} {child_name}", route.path);
        let child = unique_route(&child_path)?;
        command = command.subcommand(build_route(child)?);
    }
    Ok(command)
}

/// Build the route-only clap tree from the frozen generated inventory.
/// Options and positionals are deliberately left to R01B.
pub fn build() -> Result<Command, AppError> {
    let root = unique_route("linear")?;
    Ok(build_route(root)?.disable_help_subcommand(true))
}

/// Resolve a clap match chain to the canonical generated route identity.
pub fn selected_route(matches: &ArgMatches) -> Result<Route, AppError> {
    let mut path = String::from("linear");
    let mut selected = matches;
    while let Some((name, child)) = selected.subcommand() {
        path.push(' ');
        path.push_str(name);
        selected = child;
    }
    Ok(unique_route(&path)?.route)
}
