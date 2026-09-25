//! Static shell completion scripts and the hidden v2 `complete` shim.
use clap::Command;
use clap_complete::{Shell, generate};

use crate::cli::clap_input::{OptionValue, ParsedAction};
use crate::cli::suggest::closest;
use crate::cli::{ROUTES, RouteMeta, TypeHandler, clap_tree, fish_completion};
use crate::error::{AppError, AppErrorKind};

/// The literal default script name. Cliffy used its main command name, never
/// the invoked path, and so does v3.
pub const DEFAULT_COMMAND_NAME: &str = "linear";

/// Cliffy registers `boolean` on its main command as a global type whose
/// completion is these values; the inventory records only custom types.
const BUILTIN_BOOLEAN_VALUES: &[&str] = &["true", "false"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionShell {
    Bash,
    Fish,
    Zsh,
}

fn invariant(message: impl Into<String>) -> AppError {
    AppError::new(AppErrorKind::Invariant, message)
}

/// Generated scripts embed the name unquoted in function names, `complete`
/// registrations and `#compdef`, so only a plain command word is accepted.
fn valid_command_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphanumeric() || first == '_')
        && chars.all(|letter| letter.is_ascii_alphanumeric() || matches!(letter, '_' | '-' | '.'))
}

fn command_name(action: &ParsedAction) -> Result<&str, AppError> {
    let name = match action.option("name").map(|option| &option.value.value) {
        None => return Ok(DEFAULT_COMMAND_NAME),
        Some(OptionValue::String(name)) => name,
        Some(other) => {
            return Err(invariant(format!(
                "completion name option has an unexpected shape: {other:?}"
            )));
        }
    };
    if valid_command_name(name) {
        Ok(name)
    } else {
        Err(AppError::new(
            AppErrorKind::Validation,
            format!("Invalid command name \"{name}\""),
        )
        .with_suggestion(
            "Use ASCII letters, digits, '_', '-' or '.', starting with a letter, digit or '_'.",
        ))
    }
}

/// Generate one complete script from the static completion view of the clap
/// tree into memory, so a failed stdout write never reaches the generator.
/// Fish uses the project generator, which resolves full command paths at any
/// depth; bash and zsh use `clap_complete`.
pub fn script(shell: CompletionShell, action: &ParsedAction) -> Result<Vec<u8>, AppError> {
    let name = command_name(action)?.to_owned();
    let mut command = clap_tree::build_completion()?;
    let generator = match shell {
        CompletionShell::Bash => Shell::Bash,
        CompletionShell::Zsh => Shell::Zsh,
        CompletionShell::Fish => return fish_completion::script(command, &name),
    };
    let mut script = Vec::new();
    generate(generator, &mut command, name, &mut script);
    Ok(script)
}

fn route_for(path: &str) -> Result<&'static RouteMeta, AppError> {
    let mut found = ROUTES.iter().filter(|route| route.path == path);
    match (found.next(), found.next()) {
        (Some(route), None) => Ok(route),
        _ => Err(invariant(format!("completion route not unique: {path}"))),
    }
}

fn unknown_command(word: &str, parent: &Command) -> AppError {
    let names = parent
        .get_subcommands()
        .filter(|command| !command.is_hide_set())
        .map(Command::get_name)
        .collect::<Vec<_>>();
    let suggestion = closest(word, &names).map_or(String::new(), |name| {
        format!(" Did you mean command \"{name}\"?")
    });
    AppError::new(
        AppErrorKind::Validation,
        format!("Auto-completion failed. Unknown command \"{word}\".{suggestion}"),
    )
}

/// Local type first, then the nearest ancestor's global type, then Cliffy's
/// built-in global `boolean`. Other built-ins and unknown actions are empty.
fn completion_values(
    type_name: &str,
    chain: &[&'static RouteMeta],
) -> Result<&'static [&'static str], AppError> {
    let (selected, ancestors) = chain
        .split_last()
        .ok_or_else(|| invariant("completion chain lacks the root route"))?;
    let found = selected
        .local_types
        .iter()
        .find(|definition| definition.name == type_name)
        .or_else(|| {
            ancestors.iter().rev().find_map(|route| {
                route
                    .local_types
                    .iter()
                    .find(|definition| definition.global && definition.name == type_name)
            })
        });
    Ok(match found.map(|definition| definition.handler) {
        Some(TypeHandler::Enum(values)) => values,
        Some(TypeHandler::Variable) => &[],
        None if type_name == "boolean" => BUILTIN_BOOLEAN_VALUES,
        None => &[],
    })
}

/// Resolve `complete <action> [command...]` the way saved v2 scripts call it:
/// words select visible commands by name or alias, words after `--` are not
/// command words, and values are joined by LF without a trailing newline.
pub fn complete(action: &ParsedAction) -> Result<Vec<u8>, AppError> {
    let (type_name, words) = action
        .positionals
        .split_first()
        .ok_or_else(|| invariant("complete lacks its action argument"))?;
    let tree = clap_tree::build()?;
    let mut command = &tree;
    let mut path = String::from("linear");
    let mut chain = vec![route_for(&path)?];
    for word in words {
        let child = command.get_subcommands().find(|child| {
            child.get_name() == word || child.get_all_aliases().any(|alias| alias == word)
        });
        let child = match child {
            Some(child) if !child.is_hide_set() => child,
            Some(_) | None => return Err(unknown_command(word, command)),
        };
        path.push(' ');
        path.push_str(child.get_name());
        chain.push(route_for(&path)?);
        command = child;
    }
    Ok(completion_values(type_name, &chain)?
        .join("\n")
        .into_bytes())
}
