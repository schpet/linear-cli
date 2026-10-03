//! Shell scripts from the native command grammar and saved-script value data.
use crate::cli::{self, AgentSessionStatus, Sort, TemplateType, fish_completion};
use crate::error::Error;
use clap::{Command, ValueEnum};
use clap_complete::{Shell, generate};
pub const DEFAULT_COMMAND_NAME: &str = "linear";
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionShell {
    Bash,
    Fish,
    Zsh,
}
fn valid_command_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphanumeric() || first == '_')
        && chars.all(|letter| letter.is_ascii_alphanumeric() || matches!(letter, '_' | '-' | '.'))
}
/// `clap_complete` includes hidden entries; derive a visible view from the
/// actual built grammar so hidden script interfaces are never offered.
fn completion_view(native: &Command) -> Command {
    let mut view = Command::new(native.get_name().to_owned())
        .disable_help_flag(true)
        .disable_version_flag(true)
        .disable_help_subcommand(true)
        .visible_aliases(native.get_visible_aliases().map(str::to_owned))
        .args(
            native
                .get_arguments()
                .filter(|arg| !arg.is_hide_set())
                .cloned(),
        )
        .subcommands(
            native
                .get_subcommands()
                .filter(|child| !child.is_hide_set())
                .map(completion_view),
        );
    if let Some(version) = native.get_version() {
        view = view.version(version.to_owned());
    }
    if let Some(version) = native.get_long_version() {
        view = view.long_version(version.to_owned());
    }
    if let Some(about) = native.get_about() {
        view = view.about(about.clone());
    }
    view
}

pub fn script(shell: CompletionShell, supplied_name: Option<&str>) -> Result<Vec<u8>, Error> {
    let name = supplied_name.unwrap_or(DEFAULT_COMMAND_NAME);
    if !valid_command_name(name) {
        return Err(
            Error::new(format!("Invalid command name \"{name}\"")).with_hint(
                "Use ASCII letters, digits, '_', '-' or '.', starting with a letter, digit or '_'.",
            ),
        );
    }
    let mut command = cli::command();
    command.build();
    let mut command = completion_view(&command);
    command.build();
    let generator = match shell {
        CompletionShell::Bash => Shell::Bash,
        CompletionShell::Zsh => Shell::Zsh,
        CompletionShell::Fish => return fish_completion::script(command, name),
    };
    let mut output = Vec::new();
    generate(generator, &mut command, name, &mut output);
    Ok(output)
}
fn enum_values<T: ValueEnum>() -> Vec<String> {
    T::value_variants()
        .iter()
        .filter_map(|value| value.to_possible_value())
        .filter(|value| !value.is_hide_set())
        .map(|value| value.get_name().to_owned())
        .collect()
}
/// Saved scripts select a command path before `--`; literal words stay separate.
/// The output is LF-separated data without a trailing newline.
pub fn complete(action: &cli::completions::CompletionsComplete) -> Result<Vec<u8>, Error> {
    let mut tree = cli::command();
    tree.build();
    let mut command = &tree;
    let mut path = vec![];
    for word in &action.command {
        command = command
            .get_subcommands()
            .find(|child| {
                !child.is_hide_set()
                    && (child.get_name() == word
                        || child.get_all_aliases().any(|alias| alias == word))
            })
            .ok_or_else(|| {
                Error::new(format!(
                    "Auto-completion failed. Unknown command \"{word}\"."
                ))
            })?;
        path.push(command.get_name());
    }
    let values = match (action.action.as_str(), path.as_slice()) {
        ("boolean", _) => vec!["true".to_owned(), "false".to_owned()],
        ("sort", ["issue", "mine" | "query"] | ["config"]) => enum_values::<Sort>(),
        ("agentSessionStatus", ["issue", "agent-session", "list"]) => {
            enum_values::<AgentSessionStatus>()
        }
        ("template-type", ["template", "list"]) => enum_values::<TemplateType>(),
        _ => vec![],
    };
    Ok(values.join("\n").into_bytes())
}
