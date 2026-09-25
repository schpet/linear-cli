//! Clap command tree built from the frozen generated inventory.

use clap::{Arg, ArgAction, ArgMatches, Command, builder::PossibleValuesParser};

use super::{
    ArgumentMeta, HELP_FLAG_DESCRIPTION, OptionMeta, ROUTES, Route, RouteMeta, TypeHandler,
    VERSION_FLAG_DESCRIPTION,
};
use crate::error::{AppError, AppErrorKind};

/// Which consumer a tree is built for. Both surfaces come from one builder and
/// the same generated inventory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Surface {
    /// The production parser: hidden routes, hidden options and internal
    /// positionals are registered so every accepted spelling parses.
    Parse,
    /// Static completion scripts: `clap_complete` offers every registered
    /// argument and subcommand regardless of `hide`, and zsh embeds positional
    /// ids in `_arguments` specs, so hidden and internal entries are omitted
    /// and positional ids are the plain argument names.
    Completion,
}

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
        .help(option.description)
        .value_name(value_name)
        .value_parser(parse_nonempty_string)
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

fn positional_arg(argument: &'static ArgumentMeta, index: usize, surface: Surface) -> Arg {
    let id = match surface {
        Surface::Parse => format!("pos:{}", argument.name),
        Surface::Completion => argument.name.to_owned(),
    };
    let mut arg = Arg::new(id)
        .index(index)
        .value_name(argument.name)
        .required(false)
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
        .help(option.description)
        .hide(option.hidden)
        .required(false);
    let mut primary_long = false;
    let mut primary_short = false;
    // Secondary spellings such as `--ref` are listed Cliffy flags, so they are
    // visible aliases that completion scripts offer too.
    for flag in super::spelling::effective_flags(route, option) {
        if let Some(long) = flag.strip_prefix("--") {
            if long.is_empty() {
                return Err(invariant(format!("empty long switch on {}", route.path)));
            }
            arg = if primary_long {
                arg.visible_alias(long)
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
                arg.visible_short_alias(letter)
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

fn label_workspace_filter_arg(
    route: &'static RouteMeta,
    option: &'static OptionMeta,
) -> Result<Arg, AppError> {
    if option.name != "workspace"
        || option.flags != ["--workspace"]
        || !option.args.is_empty()
        || option.hidden
        || option.required
    {
        return Err(invariant("label list workspace filter descriptor changed"));
    }
    let flags = super::spelling::effective_flags(route, option);
    if flags != ["--workspace-only"] {
        return Err(invariant("label list effective workspace spelling changed"));
    }
    Ok(Arg::new("opt:workspace")
        .long("workspace-only")
        .help(option.description)
        .action(ArgAction::SetTrue))
}

fn parse_finite_number(value: &str) -> Result<String, String> {
    match value.parse::<f64>() {
        Ok(number) if number.is_finite() => Ok(value.to_owned()),
        Ok(_) | Err(_) => Err(format!("expected a finite number, got {value:?}")),
    }
}

fn parse_nonempty_string(value: &str) -> Result<String, String> {
    if value.is_empty() {
        Err("expected a nonempty value".to_owned())
    } else {
        Ok(value.to_owned())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariableAssignment {
    pub key: String,
    pub value: String,
}

fn parse_variable_assignment(value: &str) -> Result<VariableAssignment, String> {
    let (key, rest) = value
        .split_once('=')
        .ok_or_else(|| format!("expected key=value, got {value:?}"))?;
    Ok(VariableAssignment {
        key: key.to_owned(),
        value: rest.to_owned(),
    })
}

fn free_text_value(route: &str, name: &str) -> bool {
    matches!(
        (route, name),
        ("linear issue query", "search")
            | ("linear issue pull-request", "title")
            | ("linear issue create", "description" | "title")
            | ("linear issue update", "description" | "title")
            | ("linear issue comment add", "body")
            | ("linear issue comment update", "body")
            | ("linear issue attach", "title" | "comment")
            | ("linear issue link", "title")
            | ("linear team create", "name" | "description")
            | ("linear project create", "name" | "description" | "content")
            | ("linear project update", "name" | "description" | "content")
            | ("linear project comment add", "body")
            | ("linear project-update create", "body")
            | ("linear milestone create", "name" | "description")
            | ("linear milestone update", "name" | "description")
            | ("linear initiative create", "name" | "description")
            | ("linear initiative update", "name" | "description")
            | ("linear initiative comment add", "body")
            | ("linear initiative-update create", "body")
            | ("linear label create", "name" | "description")
            | ("linear document create", "title" | "content")
            | ("linear document update", "title" | "content")
            | ("linear document comment add", "body")
    )
}

fn valued_option_base(
    route: &'static RouteMeta,
    option: &'static OptionMeta,
    argument: &'static ArgumentMeta,
    action: ArgAction,
) -> Result<Arg, AppError> {
    let mut arg = Arg::new(format!("opt:{}", option.name))
        .value_name(argument.name)
        .action(action)
        .help(option.description)
        .hide(option.hidden)
        .required(false);
    let mut primary_long = false;
    let mut primary_short = false;
    for flag in super::spelling::effective_flags(route, option) {
        if let Some(long) = flag.strip_prefix("--") {
            if long.is_empty() {
                return Err(invariant(format!(
                    "empty long valued option on {}",
                    route.path
                )));
            }
            arg = if primary_long {
                arg.visible_alias(long)
            } else {
                primary_long = true;
                arg.long(long)
            };
        } else if let Some(short) = flag.strip_prefix('-') {
            let mut chars = short.chars();
            let letter = match (chars.next(), chars.next()) {
                (Some(letter), None) => letter,
                _ => {
                    return Err(invariant(format!(
                        "invalid short valued option on {}",
                        route.path
                    )));
                }
            };
            arg = if primary_short {
                arg.visible_short_alias(letter)
            } else {
                primary_short = true;
                arg.short(letter)
            };
        } else {
            return Err(invariant(format!(
                "invalid valued option flag on {}",
                route.path
            )));
        }
    }
    if !primary_long && !primary_short {
        return Err(invariant(format!(
            "valued option has no flag on {}",
            route.path
        )));
    }
    Ok(arg)
}

fn valued_option_arg(
    route: &'static RouteMeta,
    option: &'static OptionMeta,
) -> Result<Arg, AppError> {
    let argument = match option.args {
        [argument] if !argument.optional && !argument.variadic && !argument.list => argument,
        _ => {
            return Err(invariant(format!(
                "ordinary option has unsupported value shape: {} {}",
                route.path, option.name
            )));
        }
    };
    let mut arg = valued_option_base(route, option, argument, ArgAction::Set)?;
    match argument.type_name {
        "string" => {
            arg = arg.value_parser(parse_nonempty_string);
            if free_text_value(route.path, option.name) {
                arg = arg.allow_hyphen_values(true);
            }
        }
        "number" => {
            arg = arg
                .allow_hyphen_values(true)
                .value_parser(parse_finite_number);
        }
        "integer" => {
            return Err(invariant(format!(
                "integer valued option is not in the reviewed inventory: {} {}",
                route.path, option.name
            )));
        }
        type_name => {
            let mut definitions = route
                .local_types
                .iter()
                .filter(|definition| definition.name == type_name);
            let definition = match (definitions.next(), definitions.next()) {
                (Some(definition), None) => definition,
                _ => {
                    return Err(invariant(format!(
                        "missing or duplicate value type {type_name} on {}",
                        route.path
                    )));
                }
            };
            match definition.handler {
                TypeHandler::Enum(values) => {
                    arg = arg.value_parser(PossibleValuesParser::new(values.iter().copied()));
                }
                TypeHandler::Variable => {
                    return Err(invariant(format!(
                        "variable value type unexpectedly ordinary on {}",
                        route.path
                    )));
                }
            }
        }
    }
    Ok(arg)
}

fn collected_option_arg(
    route: &'static RouteMeta,
    option: &'static OptionMeta,
) -> Result<Arg, AppError> {
    if !option.collect || option.required || option.hidden {
        return Err(invariant(format!(
            "collected option has unexpected settings on {} {}",
            route.path, option.name
        )));
    }
    let argument = match option.args {
        [argument] if !argument.optional && !argument.variadic && !argument.list => argument,
        _ => {
            return Err(invariant(format!(
                "collected option has unsupported value shape: {} {}",
                route.path, option.name
            )));
        }
    };
    let mut arg = valued_option_base(route, option, argument, ArgAction::Append)?.num_args(1);
    match argument.type_name {
        "string" => arg = arg.value_parser(parse_nonempty_string),
        type_name => {
            let mut definitions = route
                .local_types
                .iter()
                .filter(|definition| definition.name == type_name);
            let definition = match (definitions.next(), definitions.next()) {
                (Some(definition), None) => definition,
                _ => {
                    return Err(invariant(format!(
                        "missing or duplicate collected value type {type_name} on {}",
                        route.path
                    )));
                }
            };
            match definition.handler {
                TypeHandler::Variable => arg = arg.value_parser(parse_variable_assignment),
                TypeHandler::Enum(_) => {
                    return Err(invariant(format!(
                        "unreviewed collected enum type {type_name} on {}",
                        route.path
                    )));
                }
            }
        }
    }
    Ok(arg)
}

fn bulk_option_arg(
    route: &'static RouteMeta,
    option: &'static OptionMeta,
) -> Result<Arg, AppError> {
    if !matches!(
        route.path,
        "linear issue archive"
            | "linear issue delete"
            | "linear initiative archive"
            | "linear initiative delete"
            | "linear document delete"
    ) || option.name != "bulk"
        || option.flags != ["--bulk"]
        || option.collect
        || option.required
        || option.hidden
    {
        return Err(invariant(format!(
            "bulk option is outside the reviewed inventory on {}",
            route.path
        )));
    }
    let argument = match option.args {
        [argument]
            if argument.name == "ids"
                && argument.type_name == "string"
                && !argument.optional
                && argument.variadic
                && !argument.list =>
        {
            argument
        }
        _ => {
            return Err(invariant(format!(
                "bulk option has an unexpected value shape on {}",
                route.path
            )));
        }
    };
    Ok(valued_option_base(route, option, argument, ArgAction::Set)?
        .num_args(1..)
        .value_parser(parse_nonempty_string))
}

/// The first description line, which Cliffy lists beside a command name.
fn summary(description: &'static str) -> &'static str {
    description.split('\n').next().unwrap_or(description)
}

fn build_route(route: &'static RouteMeta, surface: Surface) -> Result<Command, AppError> {
    let about = match surface {
        Surface::Parse => route.description,
        Surface::Completion => summary(route.description),
    };
    let mut command = Command::new(route.name)
        .about(about)
        .hide(route.hidden)
        .arg(
            Arg::new("help:short")
                .short('h')
                .help(HELP_FLAG_DESCRIPTION)
                .action(ArgAction::Count),
        )
        .arg(
            Arg::new("help:long")
                .long("help")
                .help(HELP_FLAG_DESCRIPTION)
                .action(ArgAction::Count),
        );
    if route.path == "linear" {
        command = command
            .arg(
                Arg::new("version:short")
                    .short('V')
                    .help(VERSION_FLAG_DESCRIPTION)
                    .action(ArgAction::Count),
            )
            .arg(
                Arg::new("version:long")
                    .long("version")
                    .help(VERSION_FLAG_DESCRIPTION)
                    .action(ArgAction::Count),
            );
    }
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
        label_workspace_filter_arg(route, filter)?;
    }
    for (offset, argument) in route.arguments.iter().enumerate() {
        command = command.arg(positional_arg(argument, offset + 1, surface));
    }
    let next_index = route.arguments.len() + 1;
    if surface == Surface::Parse && !route.arguments.iter().any(|argument| argument.variadic) {
        command = command.arg(
            Arg::new("internal:surplus")
                .index(next_index)
                .num_args(0..)
                .action(ArgAction::Append)
                .hide(true),
        );
    }
    if surface == Surface::Parse {
        command = command.arg(
            Arg::new("internal:literal")
                .index(
                    next_index
                        + usize::from(!route.arguments.iter().any(|argument| argument.variadic)),
                )
                .num_args(0..)
                .action(ArgAction::Append)
                .last(true)
                .hide(true),
        );
    }
    for option in route.local_options {
        if surface == Surface::Completion && option.hidden {
            continue;
        }
        if option.args.is_empty() {
            let arg = if route.path == "linear label list" && option.name == "workspace" {
                label_workspace_filter_arg(route, option)?
            } else {
                switch_arg(route, option)?
            };
            command = command.arg(arg);
        } else if route.path == "linear" && option.name == "workspace" {
            // The root credential option was registered above for every route.
        } else if option.name == "bulk" {
            command = command.arg(bulk_option_arg(route, option)?);
        } else if option.collect {
            command = command.arg(collected_option_arg(route, option)?);
        } else {
            command = command.arg(valued_option_arg(route, option)?);
        }
    }
    // Visible aliases let static completion scripts navigate and offer them;
    // the custom help renderer and diagnostics read the inventory instead.
    for alias in route.aliases {
        command = command.visible_alias(alias);
    }
    for child_name in route.children {
        let child_path = format!("{} {child_name}", route.path);
        let child = unique_route(&child_path)?;
        if surface == Surface::Completion && child.hidden {
            continue;
        }
        command = command.subcommand(build_route(child, surface)?);
    }
    Ok(command)
}

fn build_surface(surface: Surface) -> Result<Command, AppError> {
    let root = unique_route("linear")?;
    Ok(build_route(root, surface)?
        .disable_help_subcommand(true)
        .disable_help_flag(true)
        .disable_version_flag(true))
}

/// Build the production parser tree from the frozen generated inventory.
pub fn build() -> Result<Command, AppError> {
    build_surface(Surface::Parse)
}

/// Build the static completion view of the same inventory: hidden routes,
/// hidden options and internal positionals are omitted.
pub fn build_completion() -> Result<Command, AppError> {
    build_surface(Surface::Completion)
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
