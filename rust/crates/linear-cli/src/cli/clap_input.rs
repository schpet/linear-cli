//! Typed, shadow-only extraction from the registered clap grammar.
use std::ffi::OsString;

use clap::ArgMatches;
use clap::error::{ContextKind, ContextValue, ErrorKind};
use clap::parser::ValueSource;

use super::clap_tree::{self, VariableAssignment};
use super::suggest::closest;
use super::{OptionDefault, OptionMeta, ROUTES, RouteMeta, TypeHandler};
use crate::error::{AppError, AppErrorKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueOrigin {
    Explicit,
    Default,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Sourced<T> {
    pub value: T,
    pub origin: ValueOrigin,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CollectedValue {
    String(String),
    Variable { key: String, value: String },
}

#[derive(Clone, Debug, PartialEq)]
pub enum OptionValue {
    Switch(bool),
    String(String),
    Number(f64),
    Enum(String),
    Collected(Vec<CollectedValue>),
    Bulk(Vec<String>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct TypedOption {
    pub name: &'static str,
    pub value: Sourced<OptionValue>,
}

impl TypedOption {
    pub fn is_explicit(&self) -> bool {
        self.value.origin == ValueOrigin::Explicit
    }
}

#[derive(Clone, Debug)]
pub struct ParsedAction {
    pub route: &'static RouteMeta,
    pub positionals: Vec<String>,
    pub literal: Vec<String>,
    pub global_workspace: Option<Sourced<String>>,
    pub options: Vec<TypedOption>,
}

impl ParsedAction {
    pub fn option(&self, name: &str) -> Option<&TypedOption> {
        self.options.iter().find(|option| option.name == name)
    }
}

#[derive(Clone, Debug)]
pub enum Invocation {
    Help {
        route: &'static RouteMeta,
        long: bool,
    },
    Version {
        long: bool,
    },
    Action(ParsedAction),
}

fn invariant(message: impl Into<String>) -> AppError {
    AppError::new(AppErrorKind::Invariant, message)
}

fn route_for(path: &str) -> Result<&'static RouteMeta, AppError> {
    let mut found = ROUTES.iter().filter(|route| route.path == path);
    match (found.next(), found.next()) {
        (Some(route), None) => Ok(route),
        _ => Err(invariant(format!("clap route not unique: {path}"))),
    }
}

fn present(matches: &ArgMatches, id: &str) -> Result<bool, AppError> {
    Ok(matches.value_source(id) == Some(ValueSource::CommandLine))
}

fn count(matches: &ArgMatches, id: &str) -> Result<u8, AppError> {
    matches
        .try_get_one::<u8>(id)
        .map(|value| value.copied().unwrap_or(0))
        .map_err(|error| invariant(format!("clap count shape for {id}: {error}")))
}

fn values(matches: &ArgMatches, id: &str) -> Result<Vec<String>, AppError> {
    matches
        .try_get_many::<String>(id)
        .map(|values| {
            values
                .map(|values| values.cloned().collect())
                .unwrap_or_default()
        })
        .map_err(|error| invariant(format!("clap string shape for {id}: {error}")))
}

fn positionals(route: &RouteMeta, matches: &ArgMatches) -> Result<Vec<String>, AppError> {
    let mut result = Vec::new();
    for argument in route.arguments {
        result.extend(values(matches, &format!("pos:{}", argument.name))?);
    }
    Ok(result)
}

fn surplus(matches: &ArgMatches) -> Result<Vec<String>, AppError> {
    values(matches, "internal:surplus")
}
fn literal(matches: &ArgMatches) -> Result<Vec<String>, AppError> {
    values(matches, "internal:literal")
}

fn arity(route: &'static RouteMeta, args: &[String], has_action: bool) -> Result<(), AppError> {
    if route.arguments.is_empty() {
        if let Some(first) = args.first() {
            if !route.children.is_empty() {
                if super::resolve_child(route, first).is_some() {
                    return Err(AppError::usage(
                        route.route,
                        format!("Too many arguments: {}", args.join(" ")),
                    ));
                }
                let names = route
                    .children
                    .iter()
                    .filter_map(|name| {
                        ROUTES
                            .iter()
                            .find(|child| {
                                child.path == format!("{} {name}", route.path) && !child.hidden
                            })
                            .map(|child| child.name)
                    })
                    .collect::<Vec<_>>();
                let suggestion = closest(first, &names).map_or(String::new(), |name| {
                    format!(" Did you mean command \"{name}\"?")
                });
                return Err(AppError::usage(
                    route.route,
                    format!("Unknown command \"{first}\".{suggestion}"),
                ));
            }
            return Err(AppError::usage(
                route.route,
                format!("No arguments allowed for command \"{}\".", route.path),
            ));
        }
        return Ok(());
    }
    if args.is_empty() && !has_action {
        let names = route
            .arguments
            .iter()
            .filter(|arg| !arg.optional)
            .map(|arg| arg.name)
            .collect::<Vec<_>>();
        if !names.is_empty() {
            return Err(AppError::usage(
                route.route,
                format!("Missing argument(s): {}", names.join(", ")),
            ));
        }
    }
    let mut consumed = 0;
    for argument in route.arguments {
        if argument.variadic {
            if consumed == args.len() && !argument.optional && !args.is_empty() {
                return Err(AppError::usage(
                    route.route,
                    format!("Missing argument: {}", argument.name),
                ));
            }
            consumed = args.len();
            break;
        }
        if consumed < args.len() {
            consumed += 1;
        } else if !argument.optional && !args.is_empty() {
            return Err(AppError::usage(
                route.route,
                format!("Missing argument: {}", argument.name),
            ));
        }
    }
    if consumed < args.len() {
        return Err(AppError::usage(
            route.route,
            format!(
                "Too many arguments: {}",
                args.iter()
                    .skip(consumed)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
        ));
    }
    Ok(())
}

fn default_value(option: &OptionMeta) -> Result<Option<OptionValue>, AppError> {
    match option.default {
        OptionDefault::Integer(value) => Ok(Some(OptionValue::Number(
            value
                .to_string()
                .parse::<f64>()
                .map_err(|_| invariant("invalid integer metadata default"))?,
        ))),
        OptionDefault::Strings(values) => Ok(Some(OptionValue::Collected(
            values
                .iter()
                .map(|value| CollectedValue::String((*value).to_owned()))
                .collect(),
        ))),
        OptionDefault::Null | OptionDefault::Absent => Ok(None),
    }
}

fn typed_option(
    route: &'static RouteMeta,
    option: &'static OptionMeta,
    matches: &ArgMatches,
) -> Result<Option<TypedOption>, AppError> {
    let id = format!("opt:{}", option.name);
    let explicit = present(matches, &id)?;
    if !explicit {
        if option.name.starts_with("no-") {
            return Ok(Some(TypedOption {
                name: option.name,
                value: Sourced {
                    value: OptionValue::Switch(true),
                    origin: ValueOrigin::Default,
                },
            }));
        }
        return Ok(default_value(option)?.map(|value| TypedOption {
            name: option.name,
            value: Sourced {
                value,
                origin: ValueOrigin::Default,
            },
        }));
    }
    let value = if option.args.is_empty() {
        OptionValue::Switch(!option.name.starts_with("no-"))
    } else if option.name == "bulk" {
        OptionValue::Bulk(values(matches, &id)?)
    } else if option.collect {
        let descriptor = option
            .args
            .first()
            .ok_or_else(|| invariant("collected option lacks descriptor"))?;
        if descriptor.type_name == "string" {
            OptionValue::Collected(
                values(matches, &id)?
                    .into_iter()
                    .map(CollectedValue::String)
                    .collect(),
            )
        } else {
            let values = matches
                .try_get_many::<VariableAssignment>(&id)
                .map_err(|error| invariant(format!("clap variable shape: {error}")))?
                .ok_or_else(|| invariant("explicit variable option lacks values"))?;
            OptionValue::Collected(
                values
                    .map(|value| CollectedValue::Variable {
                        key: value.key.clone(),
                        value: value.value.clone(),
                    })
                    .collect(),
            )
        }
    } else {
        let descriptor = option
            .args
            .first()
            .ok_or_else(|| invariant("valued option lacks descriptor"))?;
        let value = matches
            .try_get_one::<String>(&id)
            .map_err(|error| invariant(format!("clap value shape for {id}: {error}")))?
            .ok_or_else(|| invariant(format!("explicit option lacks value: {id}")))?
            .clone();
        match descriptor.type_name {
            "number" => OptionValue::Number(
                value
                    .parse::<f64>()
                    .map_err(|_| invariant("clap validated number cannot parse"))?,
            ),
            "string" => OptionValue::String(value),
            _ => {
                if !route.local_types.iter().any(|definition| {
                    definition.name == descriptor.type_name
                        && matches!(definition.handler, TypeHandler::Enum(_))
                }) {
                    return Err(invariant(format!(
                        "unexpected option type {}",
                        descriptor.type_name
                    )));
                }
                OptionValue::Enum(value)
            }
        }
    };
    Ok(Some(TypedOption {
        name: option.name,
        value: Sourced {
            value,
            origin: ValueOrigin::Explicit,
        },
    }))
}

fn has_workspace(route: &RouteMeta) -> bool {
    route.path == "linear label list"
        || if route.path == "linear" {
            route
                .local_options
                .iter()
                .any(|option| option.name == "workspace" && option.global)
        } else {
            route
                .inherited_global_options
                .iter()
                .any(|option| option.name == "workspace" && option.global)
        }
}

fn workspace(
    chain: &[(&'static RouteMeta, &ArgMatches)],
    final_route: &'static RouteMeta,
) -> Result<Option<Sourced<String>>, AppError> {
    let mut selected = None;
    for (route, matches) in chain {
        if !has_workspace(route) {
            continue;
        }
        if present(matches, "global:workspace")? {
            let value = matches
                .try_get_one::<String>("global:workspace")
                .map_err(|error| invariant(format!("workspace shape: {error}")))?
                .ok_or_else(|| invariant("explicit workspace missing value"))?;
            if selected.is_some() {
                return Err(AppError::usage(
                    final_route.route,
                    "Option \"--workspace\" can only occur once, but was found several times.",
                ));
            }
            selected = Some(Sourced {
                value: value.clone(),
                origin: ValueOrigin::Explicit,
            });
        }
    }
    Ok(selected)
}

fn context_text(error: &clap::Error, kind: ContextKind) -> Option<String> {
    match error.get(kind) {
        Some(ContextValue::String(value)) => Some(value.clone()),
        Some(ContextValue::StyledStr(value)) => Some(value.to_string()),
        _ => None,
    }
}

fn tolerant_tree(command: clap::Command) -> clap::Command {
    command.ignore_errors(true).mut_subcommands(tolerant_tree)
}

fn failure_route(argv: &[String], tree: clap::Command) -> Result<&'static RouteMeta, AppError> {
    let mut partial = tolerant_tree(tree).try_get_matches_from(argv).ok();
    let mut path = String::from("linear");
    while let Some(matches) = partial.as_ref() {
        let Some((name, child)) = matches.subcommand() else {
            break;
        };
        path.push(' ');
        path.push_str(name);
        partial = Some(child.clone());
    }
    route_for(&path)
}

fn failure_parent_route(
    argv: &[String],
    tree: clap::Command,
) -> Result<Option<&'static RouteMeta>, AppError> {
    let matches = tolerant_tree(tree)
        .try_get_matches_from(argv)
        .map_err(|error| invariant(format!("tolerant clap parse failed: {error}")))?;
    let mut path = String::from("linear");
    let mut selected = &matches;
    loop {
        let route = route_for(&path)?;
        let Some((name, child)) = selected.subcommand() else {
            break;
        };
        let mut args = positionals(route, selected)?;
        if !route.arguments.iter().any(|argument| argument.variadic) {
            args.extend(surplus(selected)?);
        }
        if arity(route, &args, true).is_err() {
            return Ok(Some(route));
        }
        path.push(' ');
        path.push_str(name);
        selected = child;
    }
    Ok(None)
}

fn error_option(route: &'static RouteMeta, display: &str) -> Option<&'static OptionMeta> {
    let spelling = display.split_whitespace().next().unwrap_or(display);
    if route.path == "linear label list" && spelling == "--workspace" {
        return ROUTES
            .iter()
            .find(|candidate| candidate.path == "linear")
            .and_then(|root| {
                root.local_options
                    .iter()
                    .find(|option| option.name == "workspace" && option.global)
            });
    }
    route
        .local_options
        .iter()
        .chain(route.inherited_global_options.iter())
        .find(|option| {
            super::spelling::effective_flags(route, option).contains(&spelling)
                || display == format!("opt:{}", option.name)
        })
}

fn canonical_flag(option: &OptionMeta) -> String {
    if option.name.chars().count() == 1 {
        format!("-{}", option.name)
    } else {
        format!("--{}", option.name)
    }
}

fn suggestion_flags(route: &'static RouteMeta) -> Vec<&'static str> {
    let mut flags = route
        .inherited_global_options
        .iter()
        .filter(|option| {
            !route
                .local_options
                .iter()
                .any(|local| local.name == option.name)
        })
        .flat_map(|option| option.flags.iter().copied())
        .collect::<Vec<_>>();
    if route.path == "linear label list" {
        flags.push("--workspace");
    }
    flags.extend(["-h", "--help"]);
    if route.path == "linear" {
        flags.extend(["-V", "--version"]);
    }
    flags.extend(route.local_options.iter().flat_map(|option| {
        super::spelling::effective_flags(route, option)
            .iter()
            .copied()
    }));
    flags
}

fn map_error(error: clap::Error, route: &'static RouteMeta) -> AppError {
    let invalid = context_text(&error, ContextKind::InvalidArg).unwrap_or_default();
    let value = context_text(&error, ContextKind::InvalidValue).unwrap_or_default();
    let prior = context_text(&error, ContextKind::PriorArg).unwrap_or_default();
    let option = error_option(route, &invalid);
    let flag = match invalid.as_str() {
        "-h" | "--help" => "--help".to_owned(),
        "-V" | "--version" => "--version".to_owned(),
        _ => option
            .map(canonical_flag)
            .unwrap_or_else(|| invalid.clone()),
    };
    let message = match error.kind() {
        ErrorKind::UnknownArgument => {
            let candidates = suggestion_flags(route);
            let suggestion = closest(&invalid, &candidates).map_or(String::new(), |name| {
                format!(" Did you mean option \"{name}\"?")
            });
            format!("Unknown option \"{invalid}\".{suggestion}")
        }
        ErrorKind::InvalidSubcommand => format!("Unknown command \"{invalid}\"."),
        ErrorKind::InvalidValue | ErrorKind::ValueValidation if value.is_empty() => {
            format!("Missing value for option \"{flag}\".")
        }
        ErrorKind::InvalidValue | ErrorKind::ValueValidation => {
            let descriptor = option.and_then(|option| option.args.first());
            if descriptor.is_some_and(|descriptor| descriptor.type_name == "variable") {
                format!(
                    "Invalid variable format: {value}. Variables must be in key=value format, e.g. --variable teamId=abc"
                )
            } else if let Some((descriptor, choices)) = descriptor.and_then(|descriptor| {
                route.local_types.iter().find_map(|type_meta| {
                    if type_meta.name == descriptor.type_name
                        && let TypeHandler::Enum(choices) = type_meta.handler
                    {
                        return Some((descriptor, choices));
                    }
                    None
                })
            }) {
                let expected = choices
                    .iter()
                    .map(|choice| format!("\"{choice}\""))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "Option \"{flag}\" must be of type \"{}\", but got \"{value}\". Expected values: {expected}",
                    descriptor.type_name
                )
            } else {
                format!("Option \"{flag}\" has invalid value \"{value}\".")
            }
        }
        ErrorKind::TooManyValues
            if option.is_some_and(|option| option.args.is_empty())
                || ["-h", "--help", "-V", "--version"].contains(&invalid.as_str()) =>
        {
            format!("Option \"{flag}\" doesn't take a value, but got \"{value}\".")
        }
        ErrorKind::TooManyValues
        | ErrorKind::TooFewValues
        | ErrorKind::WrongNumberOfValues
        | ErrorKind::NoEquals => format!("Invalid number of values for option \"{flag}\"."),
        ErrorKind::ArgumentConflict if invalid == prior => {
            format!("Option \"{flag}\" can only occur once, but was found several times.")
        }
        ErrorKind::ArgumentConflict => format!("Option \"{flag}\" conflicts with another option."),
        _ => {
            return invariant(format!(
                "unexpected clap parse error kind: {:?}",
                error.kind()
            ));
        }
    };
    AppError::usage(route.route, message)
}

fn last_standalone(words: &[String]) -> &'static str {
    let mut last = "--version";
    for word in words.iter().skip(1) {
        match word.as_str() {
            "-h" | "--help" => last = "--help",
            "-V" | "--version" => last = "--version",
            _ if word.starts_with('-') && !word.starts_with("--") => {
                for letter in word.chars().skip(1) {
                    if letter == 'h' {
                        last = "--help";
                    }
                    if letter == 'V' {
                        last = "--version";
                    }
                }
            }
            _ => {}
        }
    }
    last
}

fn workspace_conflicts_with_standalone(
    words: &[String],
    chain: &[(&'static RouteMeta, &ArgMatches)],
) -> Result<bool, AppError> {
    for (route, matches) in chain {
        if !has_workspace(route) || !present(matches, "global:workspace")? {
            continue;
        }
        if count(matches, "help:short")? > 0 || count(matches, "help:long")? > 0 {
            return Ok(true);
        }
        if route.path == "linear"
            && (count(matches, "version:short")? > 0 || count(matches, "version:long")? > 0)
        {
            let workspace_index = words
                .iter()
                .position(|word| word == "--workspace" || word.starts_with("--workspace="));
            let version_index = words
                .iter()
                .position(|word| word == "-V" || word == "--version");
            if let (Some(workspace_index), Some(version_index)) = (workspace_index, version_index)
                && version_index < workspace_index
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn help_workspace_conflict(
    words: &[String],
    chain: &[(&'static RouteMeta, &ArgMatches)],
) -> Option<&'static RouteMeta> {
    let mut depth = 0;
    let mut help_depth = None;
    let mut workspace_depth = None;
    let mut skip_value = false;
    for word in words.iter().skip(1) {
        if skip_value {
            skip_value = false;
            continue;
        }
        if word == "--" {
            break;
        }
        let (route, _) = *chain.get(depth)?;
        if word == "--workspace" || word.starts_with("--workspace=") {
            if help_depth.is_some() {
                return Some(route);
            }
            workspace_depth = Some(depth);
            skip_value = word == "--workspace";
            continue;
        }
        if word == "-h" || word == "--help" {
            if workspace_depth == Some(depth) {
                return Some(route);
            }
            help_depth = Some(depth);
            continue;
        }
        if chain.get(depth + 1).is_some_and(|(next, _)| {
            super::resolve_child(route, word).is_some_and(|child| child.path == next.path)
        }) {
            depth += 1;
        }
    }
    None
}

fn has_short_flag(word: &str, flag: char) -> bool {
    word.starts_with('-')
        && !word.starts_with("--")
        && word
            .chars()
            .skip(1)
            .take_while(|letter| *letter != '=')
            .any(|letter| letter == flag)
}

fn root_version_before_child(words: &[String]) -> bool {
    let Some(root) = ROUTES.first() else {
        return false;
    };
    let mut seen_version = false;
    let mut skip_value = false;
    for word in words.iter().skip(1) {
        if skip_value {
            skip_value = false;
            continue;
        }
        if word == "--" {
            break;
        }
        if word == "--workspace" {
            skip_value = true;
            continue;
        }
        if has_short_flag(word, 'V') || word == "--version" {
            seen_version = true;
            continue;
        }
        if super::resolve_child(root, word).is_some() {
            return seen_version;
        }
    }
    false
}

fn root_lexical_error(words: &[String], root: &'static RouteMeta) -> Option<AppError> {
    if !root_version_before_child(words) {
        return None;
    }
    let mut skip_value = false;
    let mut workspaces = 0;
    let mut empty_workspace_equals = false;
    for word in words.iter().skip(1) {
        if skip_value {
            skip_value = false;
            continue;
        }
        if word == "--" {
            break;
        }
        if word == "--workspace" || word.starts_with("--workspace=") {
            workspaces += 1;
            if workspaces > 1 {
                return Some(AppError::usage(
                    root.route,
                    "Option \"--workspace\" can only occur once, but was found several times.",
                ));
            }
            empty_workspace_equals |= word == "--workspace=";
            skip_value = word == "--workspace" || word == "--workspace=";
            continue;
        }
        for flag in ["--help", "--version"] {
            if let Some(value) = word
                .strip_prefix(flag)
                .and_then(|suffix| suffix.strip_prefix('='))
            {
                return Some(AppError::usage(
                    root.route,
                    format!("Option \"{flag}\" doesn't take a value, but got \"{value}\"."),
                ));
            }
        }
        if word == "--help" || word == "--version" || word == "-" {
            continue;
        }
        if word.starts_with("--") {
            let spelling = word.split('=').next().unwrap_or(word);
            return Some(unknown_option(root, spelling));
        }
        if word.starts_with('-') {
            for letter in word.chars().skip(1) {
                if letter != 'h' && letter != 'V' {
                    return Some(unknown_option(root, &format!("-{letter}")));
                }
            }
        }
    }
    empty_workspace_equals.then(|| {
        AppError::usage(
            root.route,
            "Option \"--version\" cannot be combined with other options.",
        )
    })
}

fn unknown_option(route: &'static RouteMeta, invalid: &str) -> AppError {
    let candidates = suggestion_flags(route);
    let suggestion = closest(invalid, &candidates).map_or(String::new(), |name| {
        format!(" Did you mean option \"{name}\"?")
    });
    AppError::usage(
        route.route,
        format!("Unknown option \"{invalid}\".{suggestion}"),
    )
}

pub fn parse(argv: &[OsString]) -> Result<Invocation, AppError> {
    let mut words = vec!["linear".to_owned()];
    for token in argv {
        let text = token.to_str().ok_or_else(|| {
            AppError::new(AppErrorKind::IoProcess, "command argument is not UTF-8")
        })?;
        words.push(text.to_owned());
    }
    let root = ROUTES
        .first()
        .ok_or_else(|| invariant("root route missing"))?;
    if let Some(error) = root_lexical_error(&words, root) {
        return Err(error);
    }
    let tree = clap_tree::build()?;
    let matches = match tree.clone().try_get_matches_from(&words) {
        Ok(matches) => matches,
        Err(error) => {
            let route = match failure_parent_route(&words, tree.clone())? {
                Some(route) => route,
                None => failure_route(&words, tree)?,
            };
            if error.kind() == ErrorKind::TooManyValues {
                for token in words.iter().skip(1) {
                    for (spelling, canonical) in [
                        ("-h=", "--help"),
                        ("--help=", "--help"),
                        ("-V=", "--version"),
                        ("--version=", "--version"),
                    ] {
                        if let Some(value) = token
                            .strip_prefix(spelling)
                            .filter(|value| !value.is_empty())
                        {
                            return Err(AppError::usage(
                                route.route,
                                format!(
                                    "Option \"{canonical}\" doesn't take a value, but got \"{value}\"."
                                ),
                            ));
                        }
                    }
                }
            }
            return Err(map_error(error, route));
        }
    };
    let mut chain = Vec::new();
    let mut path = String::from("linear");
    let mut selected = &matches;
    loop {
        let route = route_for(&path)?;
        chain.push((route, selected));
        let Some((name, child)) = selected.subcommand() else {
            break;
        };
        path.push(' ');
        path.push_str(name);
        selected = child;
    }
    let route = chain
        .last()
        .map(|(route, _)| *route)
        .ok_or_else(|| invariant("empty clap match chain"))?;
    let root_version =
        count(&matches, "version:short")? > 0 || count(&matches, "version:long")? > 0;
    let root_help = count(&matches, "help:short")? > 0 || count(&matches, "help:long")? > 0;
    if !root_version && let Some(level) = help_workspace_conflict(&words, &chain) {
        return Err(AppError::usage(
            level.route,
            "Option \"--help\" cannot be combined with other options.",
        ));
    }
    if root_version && chain.len() > 1 {
        let root = ROUTES
            .first()
            .ok_or_else(|| invariant("root route missing"))?;
        let version_index = words
            .iter()
            .position(|word| has_short_flag(word, 'V') || word == "--version")
            .ok_or_else(|| invariant("matched root version lacks argv spelling"))?;
        let first_global = words.get(1).is_some_and(|word| {
            word == "-h"
                || word == "--help"
                || word == "--workspace"
                || word.starts_with("--workspace=")
        });
        let root_help_preparsed = first_global
            && words
                .iter()
                .position(|word| has_short_flag(word, 'h') || word == "--help")
                .is_some_and(|help_index| help_index <= version_index);
        if root_help && !root_help_preparsed {
            return Err(AppError::usage(
                root.route,
                format!(
                    "Option \"{}\" cannot be combined with other options.",
                    last_standalone(&words)
                ),
            ));
        }
        if present(&matches, "global:workspace")? {
            let workspace_index = words
                .iter()
                .position(|word| word == "--workspace" || word.starts_with("--workspace="))
                .ok_or_else(|| invariant("matched root workspace lacks argv spelling"))?;
            let conflict = if root_help_preparsed && workspace_index < version_index {
                Some("--help")
            } else if workspace_index > version_index {
                Some(last_standalone(&words))
            } else {
                None
            };
            if let Some(name) = conflict {
                return Err(AppError::usage(
                    root.route,
                    format!("Option \"{name}\" cannot be combined with other options."),
                ));
            }
        }
        let mut child_help = false;
        let mut child_workspace = false;
        for (level, child_matches) in chain.iter().skip(1) {
            child_help |=
                count(child_matches, "help:short")? > 0 || count(child_matches, "help:long")? > 0;
            child_workspace |= has_workspace(level) && present(child_matches, "global:workspace")?;
        }
        if child_help || child_workspace {
            let name = if child_help { "--help" } else { "--version" };
            return Err(AppError::usage(
                root.route,
                format!("Option \"{name}\" cannot be combined with other options."),
            ));
        }
        let trailing = words
            .get(version_index + 1..)
            .map(|tail| {
                tail.iter()
                    .take_while(|word| *word != "--")
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .ok_or_else(|| invariant("matched root version lacks argv spelling"))?;
        return Err(AppError::usage(
            root.route,
            format!("Too many arguments: {trailing}"),
        ));
    }
    let mut saw_help = false;
    let mut long_help = false;
    let mut saw_version = false;
    let mut long_version = false;
    for (level, matches) in &chain {
        let short = count(matches, "help:short")? > 0;
        let long = count(matches, "help:long")? > 0;
        let version_short = if level.path == "linear" {
            count(matches, "version:short")? > 0
        } else {
            false
        };
        let version_long = if level.path == "linear" {
            count(matches, "version:long")? > 0
        } else {
            false
        };
        saw_help |= short || long;
        long_help |= long;
        saw_version |= version_short || version_long;
        long_version |= version_long;
        let mut level_args = positionals(level, matches)?;
        if !level.arguments.iter().any(|argument| argument.variadic) {
            level_args.extend(surplus(matches)?);
        }
        arity(level, &level_args, saw_help || saw_version)?;
    }
    let mut options = Vec::new();
    for option in route.local_options {
        if route.path == "linear" && option.name == "workspace" {
            continue;
        }
        if let Some(option) = typed_option(route, option, selected)? {
            options.push(option);
        }
    }
    let global_workspace = workspace(&chain, route)?;
    if saw_help || saw_version {
        if root_help && root_version && present(&matches, "global:workspace")? {
            let version_index = words
                .iter()
                .position(|word| has_short_flag(word, 'V') || word == "--version");
            let help_index = words
                .iter()
                .position(|word| has_short_flag(word, 'h') || word == "--help");
            let workspace_index = words
                .iter()
                .position(|word| word == "--workspace" || word.starts_with("--workspace="));
            let starts_global = words.get(1).is_some_and(|word| {
                word == "-h"
                    || word == "--help"
                    || word == "--workspace"
                    || word.starts_with("--workspace=")
            });
            if starts_global
                && matches!((help_index, workspace_index, version_index), (Some(help), Some(workspace), Some(version)) if help <= version && workspace <= version)
            {
                return Err(AppError::usage(
                    ROUTES
                        .first()
                        .ok_or_else(|| invariant("root route missing"))?
                        .route,
                    "Option \"--help\" cannot be combined with other options.",
                ));
            }
        }
        let has_other = options.iter().any(TypedOption::is_explicit)
            || workspace_conflicts_with_standalone(&words, &chain)?;
        let root_help_first = words
            .get(1)
            .is_some_and(|word| word == "-h" || word == "--help");
        if has_other || (saw_help && saw_version && !root_help_first) {
            let name = last_standalone(&words);
            return Err(AppError::usage(
                route.route,
                format!("Option \"{name}\" cannot be combined with other options."),
            ));
        }
        if saw_help {
            return Ok(Invocation::Help {
                route,
                long: long_help,
            });
        }
        return Ok(Invocation::Version { long: long_version });
    }
    for option in route.local_options.iter().filter(|option| option.required) {
        if !options
            .iter()
            .any(|actual| actual.name == option.name && actual.is_explicit())
        {
            return Err(AppError::usage(
                route.route,
                format!("Missing required option \"{}\".", canonical_flag(option)),
            ));
        }
    }
    Ok(Invocation::Action(ParsedAction {
        route,
        positionals: positionals(route, selected)?,
        literal: literal(selected)?,
        global_workspace,
        options,
    }))
}
