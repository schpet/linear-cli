//! Shadow clap command tree. The live parser remains in `parser` until R01C.

use clap::{Arg, ArgAction, ArgMatches, Command};

use super::{ArgumentMeta, OptionMeta, ROUTES, Route, RouteMeta};
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

fn workspace_arg(option: &'static OptionMeta) -> Result<Arg, AppError> {
    if option.name != "workspace" || option.flags != ["--workspace"] || option.args.len() != 1 {
        return Err(invariant(
            "global workspace descriptor changed unexpectedly",
        ));
    }
    let value_name = option
        .args
        .first()
        .ok_or_else(|| invariant("global workspace option has no value descriptor"))?
        .name;
    Ok(Arg::new("global:workspace")
        .long("workspace")
        .value_name(value_name)
        .action(ArgAction::Set))
}

fn register_workspace(command: Command, route: &'static RouteMeta) -> Result<Command, AppError> {
    let options = if route.path == "linear" {
        route.local_options
    } else {
        route.inherited_global_options
    };
    let mut workspace_options = options.iter().filter(|option| option.global);
    let workspace = match (workspace_options.next(), workspace_options.next()) {
        (Some(option), None) => Some(option),
        (None, None) => None,
        _ => {
            return Err(invariant(format!(
                "invalid global options on {}",
                route.path
            )));
        }
    };
    let workspace = if route.path == "linear label list" {
        if workspace.is_some() {
            return Err(invariant(
                "label list workspace override is no longer isolated",
            ));
        }
        let root = unique_route("linear")?;
        let mut root_workspaces = root
            .local_options
            .iter()
            .filter(|option| option.name == "workspace");
        let option = match (root_workspaces.next(), root_workspaces.next()) {
            (Some(option), None) => option,
            _ => return Err(invariant("root workspace descriptor is not unique")),
        };
        Some(option)
    } else {
        workspace
    };
    match workspace {
        Some(option) => Ok(command.arg(workspace_arg(option)?)),
        None => Ok(command),
    }
}

fn positional_arg(argument: &'static ArgumentMeta, index: usize) -> Arg {
    let mut arg = Arg::new(format!("pos:{}", argument.name))
        .index(index)
        .value_name(argument.name)
        .required(!argument.optional)
        .action(ArgAction::Set);
    if argument.variadic {
        arg = arg.num_args(if argument.optional { 0.. } else { 1.. });
    }
    arg
}

fn switch_arg(route: &'static RouteMeta, option: &'static OptionMeta) -> Result<Arg, AppError> {
    if !option.args.is_empty() {
        return Err(invariant(format!(
            "valued option passed to switch builder: {} {}",
            route.path, option.name
        )));
    }
    let mut arg = Arg::new(format!("opt:{}", option.name))
        .action(ArgAction::SetTrue)
        .hide(option.hidden)
        .required(option.required);
    let mut primary_long = false;
    let mut primary_short = false;
    for flag in option.flags {
        if let Some(long) = flag.strip_prefix("--") {
            if long.is_empty() {
                return Err(invariant(format!("empty long switch on {}", route.path)));
            }
            arg = if primary_long {
                arg.alias(long)
            } else {
                primary_long = true;
                arg.long(long)
            };
        } else if let Some(short) = flag.strip_prefix('-') {
            let mut chars = short.chars();
            let letter = match (chars.next(), chars.next()) {
                (Some(letter), None) => letter,
                _ => return Err(invariant(format!("invalid short switch on {}", route.path))),
            };
            arg = if primary_short {
                arg.short_alias(letter)
            } else {
                primary_short = true;
                arg.short(letter)
            };
        } else {
            return Err(invariant(format!("invalid switch flag on {}", route.path)));
        }
    }
    if !primary_long && !primary_short {
        return Err(invariant(format!("switch has no flag on {}", route.path)));
    }
    Ok(arg)
}

fn label_workspace_filter_arg(option: &'static OptionMeta) -> Result<Arg, AppError> {
    if option.name != "workspace"
        || option.flags != ["--workspace"]
        || !option.args.is_empty()
        || option.hidden
        || option.required
    {
        return Err(invariant("label list workspace filter descriptor changed"));
    }
    Ok(Arg::new("opt:workspace")
        .long("workspace-only")
        .action(ArgAction::SetTrue))
}

fn build_route(route: &'static RouteMeta) -> Result<Command, AppError> {
    let mut command = Command::new(route.name)
        .about(route.description)
        .hide(route.hidden);
    command = register_workspace(command, route)?;
    if route.path == "linear label list" {
        let mut filters = route
            .local_options
            .iter()
            .filter(|option| option.name == "workspace");
        let filter = match (filters.next(), filters.next()) {
            (Some(option), None) => option,
            _ => return Err(invariant("label list workspace filter is not unique")),
        };
        label_workspace_filter_arg(filter)?;
    }
    for (offset, argument) in route.arguments.iter().enumerate() {
        command = command.arg(positional_arg(argument, offset + 1));
    }
    for option in route
        .local_options
        .iter()
        .filter(|option| option.args.is_empty())
    {
        let arg = if route.path == "linear label list" && option.name == "workspace" {
            label_workspace_filter_arg(option)?
        } else {
            switch_arg(route, option)?
        };
        command = command.arg(arg);
    }
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

/// Build the shadow clap tree from the frozen generated inventory.
/// Valued local options remain for R01B2/B3; production still uses `parser`.
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

/// Read the credential workspace from all route levels, rejecting a duplicate
/// even though clap stores each manually registered level separately.
pub fn selected_workspace(matches: &ArgMatches) -> Result<Option<String>, AppError> {
    let final_route = selected_route(matches)?;
    let mut path = String::from("linear");
    let mut selected = matches;
    let mut workspace = None;
    loop {
        let route = unique_route(&path)?;
        let registered = route.path == "linear"
            || !route.inherited_global_options.is_empty()
            || route.path == "linear label list";
        if registered {
            let value = selected
                .try_get_one::<String>("global:workspace")
                .map_err(|error| invariant(format!("workspace match shape: {error}")))?;
            if let Some(value) = value {
                if workspace.is_some() {
                    return Err(AppError::usage(
                        final_route,
                        "Option \"--workspace\" cannot be specified more than once.",
                    ));
                }
                workspace = Some(value.clone());
            }
        }
        match selected.subcommand() {
            Some((name, child)) => {
                path.push(' ');
                path.push_str(name);
                selected = child;
            }
            None => break,
        }
    }
    Ok(workspace)
}
