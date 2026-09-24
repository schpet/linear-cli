//! Bounded Cliffy-compatible command selection and option parsing.
use std::collections::VecDeque;

use super::{OptionMeta, ROUTES, RouteMeta, TypeHandler, resolve_child, root};
use crate::error::{AppError, AppErrorKind};

#[derive(Clone, Debug)]
pub enum ParseOutcome {
    Help {
        route: &'static RouteMeta,
        long: bool,
    },
    Version {
        long: bool,
    },
    Action {
        route: &'static RouteMeta,
        positionals: Vec<String>,
        literal: Vec<String>,
        options: Vec<ParsedOption>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedOption {
    pub name: &'static str,
    pub values: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OptionRef {
    Help,
    Version,
    Meta(&'static str),
}

#[derive(Clone, Copy, Debug)]
struct Candidate {
    kind: OptionRef,
    flags: &'static [&'static str],
    args: &'static [super::ArgumentMeta],
    global: bool,
}

const HELP_FLAGS: &[&str] = &["-h", "--help"];
const VERSION_FLAGS: &[&str] = &["-V", "--version"];

#[derive(Default)]
struct ParseContext {
    remaining: VecDeque<String>,
    actions: Vec<(OptionRef, String)>,
    standalone: Option<OptionRef>,
    literal: Vec<String>,
    options: Vec<ParsedOption>,
}

pub fn parse(argv: &[String]) -> Result<ParseOutcome, AppError> {
    let route = root().ok_or_else(|| {
        AppError::new(
            AppErrorKind::Invariant,
            "generated route inventory is empty",
        )
    })?;
    let mut context = ParseContext {
        remaining: VecDeque::from(argv.to_vec()),
        ..ParseContext::default()
    };
    parse_route(route, &mut context)
}

fn parse_route(
    route: &'static RouteMeta,
    ctx: &mut ParseContext,
) -> Result<ParseOutcome, AppError> {
    // Cliffy snapshots each route's argv before its global preparse consumes flags.
    let raw = ctx.remaining.iter().cloned().collect::<Vec<_>>();
    let mut child = ctx
        .remaining
        .front()
        .and_then(|name| resolve_child(route, name));
    if child.is_none()
        && let Some(token) = ctx.remaining.front()
    {
        let flag = token
            .split_once('=')
            .map_or(token.as_str(), |(name, _)| name);
        if candidates(route)
            .iter()
            .any(|candidate| candidate.global && candidate.flags.contains(&flag))
        {
            parse_options(route, ctx, true)?;
            child = ctx
                .remaining
                .front()
                .and_then(|name| resolve_child(route, name));
        }
    }
    if let Some(next) = child {
        ctx.remaining.pop_front();
        return parse_route(next, ctx);
    }
    parse_options(route, ctx, false)?;
    validate_positionals(route, ctx)?;
    if !raw.iter().any(|arg| arg == "--version")
        && ctx
            .actions
            .iter()
            .any(|(kind, flag)| *kind == OptionRef::Version && flag == "-V")
    {
        return Ok(ParseOutcome::Version { long: false });
    }
    if let Some((action, _)) = ctx.actions.first() {
        return match action {
            OptionRef::Help => Ok(ParseOutcome::Help {
                route,
                long: raw.iter().any(|arg| arg == "--help"),
            }),
            OptionRef::Version => Ok(ParseOutcome::Version {
                long: raw.iter().any(|arg| arg == "--version"),
            }),
            OptionRef::Meta(_) => Err(AppError::new(
                AppErrorKind::Invariant,
                "non-standalone option queued as action",
            )),
        };
    }
    Ok(ParseOutcome::Action {
        route,
        positionals: ctx.remaining.drain(..).collect(),
        literal: std::mem::take(&mut ctx.literal),
        options: std::mem::take(&mut ctx.options),
    })
}

fn candidates(route: &'static RouteMeta) -> Vec<Candidate> {
    let mut result = Vec::new();
    for option in route.inherited_global_options {
        if !route
            .local_options
            .iter()
            .any(|local| local.name == option.name)
        {
            result.push(meta_candidate(option));
        }
    }
    result.push(Candidate {
        kind: OptionRef::Help,
        flags: HELP_FLAGS,
        args: &[],
        global: true,
    });
    if route.path == "linear" {
        result.push(Candidate {
            kind: OptionRef::Version,
            flags: VERSION_FLAGS,
            args: &[],
            global: false,
        });
    }
    result.extend(route.local_options.iter().map(meta_candidate));
    result
}

fn meta_candidate(option: &'static OptionMeta) -> Candidate {
    Candidate {
        kind: OptionRef::Meta(option.name),
        flags: option.flags,
        args: option.args,
        global: option.global,
    }
}

fn canonical_flag(option: OptionRef) -> String {
    match option {
        OptionRef::Help => "--help".to_owned(),
        OptionRef::Version => "--version".to_owned(),
        OptionRef::Meta(name) if name.chars().count() == 1 => format!("-{name}"),
        OptionRef::Meta(name) => format!("--{name}"),
    }
}

fn validate_option_value(
    route: &'static RouteMeta,
    argument: &super::ArgumentMeta,
    value: &str,
) -> Result<(), AppError> {
    if let Some(type_meta) = route
        .local_types
        .iter()
        .find(|type_meta| type_meta.name == argument.type_name)
    {
        match type_meta.handler {
            TypeHandler::Variable if !value.contains('=') => {
                return Err(AppError::usage(
                    route.route,
                    format!(
                        "Invalid variable format: {value}. Variables must be in key=value format, e.g. --variable teamId=abc"
                    ),
                ));
            }
            TypeHandler::Variable | TypeHandler::Enum(_) => {}
        }
    }
    Ok(())
}

fn parse_options(
    route: &'static RouteMeta,
    ctx: &mut ParseContext,
    preparse: bool,
) -> Result<(), AppError> {
    let choices = candidates(route);
    let mut seen = Vec::<OptionRef>::new();
    let mut source = std::mem::take(&mut ctx.remaining);
    while let Some(token) = source.pop_front() {
        if token == "--" {
            ctx.literal.extend(source);
            break;
        }
        if !token.starts_with('-') || token == "-" {
            ctx.remaining.push_back(token);
            if preparse {
                ctx.remaining.extend(source);
                break;
            }
            continue;
        }
        if token.starts_with('-') && !token.starts_with("--") && token.chars().count() > 2 {
            let short = token.trim_start_matches('-');
            let (letters, suffix) = short
                .split_once('=')
                .map_or((short, None), |(letters, value)| (letters, Some(value)));
            let mut parts = letters
                .chars()
                .map(|letter| format!("-{letter}"))
                .collect::<Vec<_>>();
            if parts.len() > 1 {
                if let Some(suffix) = suffix
                    && let Some(last) = parts.last_mut()
                {
                    last.push('=');
                    last.push_str(suffix);
                }
                for part in parts.into_iter().rev() {
                    source.push_front(part);
                }
                continue;
            }
        }
        let (flag, inline) = token
            .split_once('=')
            .map_or((token.as_str(), None), |(name, value)| (name, Some(value)));
        let matched = choices.iter().find(|choice| choice.flags.contains(&flag));
        let Some(choice) = matched else {
            if preparse {
                ctx.remaining.push_back(token);
                ctx.remaining.extend(source);
                break;
            }
            let names = choices
                .iter()
                .flat_map(|choice| choice.flags.iter().copied())
                .collect::<Vec<_>>();
            let suggestion = closest(flag, &names).map_or(String::new(), |name| {
                format!(" Did you mean option \"{name}\"?")
            });
            return Err(AppError::usage(
                route.route,
                format!("Unknown option \"{flag}\".{suggestion}"),
            ));
        };
        if preparse && !choice.global {
            ctx.remaining.push_back(token);
            ctx.remaining.extend(source);
            break;
        }
        if let Some(value) = inline.filter(|value| !value.is_empty())
            && choice.args.is_empty()
        {
            let option_name = canonical_flag(choice.kind);
            return Err(AppError::usage(
                route.route,
                format!("Option \"{option_name}\" doesn't take a value, but got \"{value}\"."),
            ));
        }
        let mut inline = inline.filter(|value| !value.is_empty()).map(str::to_owned);
        let mut values = Vec::new();
        for argument in choice.args {
            let value = if argument.optional {
                if inline.is_some() {
                    inline.take()
                } else if source
                    .front()
                    .is_some_and(|next| !next.is_empty() && !next.starts_with('-'))
                {
                    source.pop_front()
                } else {
                    None
                }
            } else if inline.is_some() {
                inline.take()
            } else if source.front().is_some_and(|next| !next.is_empty()) {
                source.pop_front()
            } else {
                return Err(AppError::usage(
                    route.route,
                    format!(
                        "Missing value for option \"{}\".",
                        canonical_flag(choice.kind)
                    ),
                ));
            };
            if let Some(value) = value {
                validate_option_value(route, argument, &value)?;
                values.push(value);
            }
        }
        if let OptionRef::Meta(name) = choice.kind {
            ctx.options.push(ParsedOption { name, values });
        }
        seen.push(choice.kind);
        if matches!(choice.kind, OptionRef::Help | OptionRef::Version) {
            ctx.standalone = Some(choice.kind);
            ctx.actions.push((choice.kind, flag.to_owned()));
        }
    }
    if let Some(standalone) = ctx.standalone
        && seen.iter().any(|kind| *kind != standalone)
    {
        let name = match standalone {
            OptionRef::Help => "--help",
            OptionRef::Version => "--version",
            OptionRef::Meta(_) => {
                return Err(AppError::new(
                    AppErrorKind::Invariant,
                    "non-standalone option marked standalone",
                ));
            }
        };
        return Err(AppError::usage(
            route.route,
            format!("Option \"{name}\" cannot be combined with other options."),
        ));
    }
    Ok(())
}

fn validate_positionals(route: &'static RouteMeta, ctx: &ParseContext) -> Result<(), AppError> {
    let args = ctx.remaining.iter().map(String::as_str).collect::<Vec<_>>();
    if route.arguments.is_empty() {
        if let Some(first) = args.first() {
            if !route.children.is_empty() {
                if resolve_child(route, first).is_some() {
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
    if args.is_empty() && ctx.actions.is_empty() {
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
    Ok(())
}

fn closest<'a>(word: &str, choices: &'a [&str]) -> Option<&'a str> {
    let word = word.to_lowercase();
    let mut best = None;
    for candidate in choices {
        let candidate_distance = distance(&word, &candidate.to_lowercase());
        if best.is_none_or(|(_, best_distance)| candidate_distance < best_distance) {
            best = Some((*candidate, candidate_distance));
        }
    }
    best.map(|(candidate, _)| candidate)
}

fn distance(left: &str, right: &str) -> usize {
    let width = right.chars().count();
    let mut previous = (0..=width).collect::<Vec<_>>();
    let mut result = width;
    for (row, letter) in left.chars().enumerate() {
        let mut left_cost = row + 1;
        let mut current = vec![left_cost];
        for ((diagonal, above), other) in previous
            .iter()
            .copied()
            .zip(previous.iter().skip(1).copied())
            .zip(right.chars())
        {
            let cost = (above + 1)
                .min(left_cost + 1)
                .min(diagonal + usize::from(letter != other));
            current.push(cost);
            left_cost = cost;
        }
        result = left_cost;
        previous = current;
    }
    result
}
