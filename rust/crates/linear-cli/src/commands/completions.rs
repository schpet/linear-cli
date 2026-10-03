//! `linear completions <shell>`: a completion script generated from the
//! command grammar.
use crate::cli::{self, completions::Completions};
use clap::Command;

pub const DEFAULT_COMMAND_NAME: &str = "linear";

pub fn script(args: &Completions) -> Vec<u8> {
    let name = args.name.as_deref().unwrap_or(DEFAULT_COMMAND_NAME);
    let mut command = cli::command();
    command.build();
    let mut output = Vec::new();
    clap_complete::generate(args.shell, &mut visible(&command), name, &mut output);
    output
}

/// A copy of a built command without its hidden arguments and subcommands,
/// which clap_complete would otherwise offer.
fn visible(command: &Command) -> Command {
    let mut copy = Command::new(command.get_name().to_owned())
        .disable_help_flag(true)
        .disable_version_flag(true)
        .disable_help_subcommand(true)
        .visible_aliases(command.get_visible_aliases().map(str::to_owned))
        .args(
            command
                .get_arguments()
                .filter(|arg| !arg.is_hide_set())
                .cloned(),
        )
        .subcommands(
            command
                .get_subcommands()
                .filter(|child| !child.is_hide_set())
                .map(visible),
        );
    if let Some(about) = command.get_about() {
        copy = copy.about(about.clone());
    }
    if let Some(version) = command.get_version() {
        copy = copy.version(version.to_owned());
    }
    copy
}
