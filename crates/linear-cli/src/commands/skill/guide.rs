//! The guide `linear skill show` prints: hand-written Markdown with the parts
//! that must match the binary filled in when it is printed.
use clap::Command;

use crate::cli::EXIT_STATUS;

const GUIDE: &str = include_str!("guide.md");
const MARKDOWN: &str = include_str!("../../cli/markdown.txt");

/// Root commands the index leaves out: `help` is clap's own, and
/// `completions` is for a person's shell startup file.
const UNLISTED: &[&str] = &["help", "completions"];

/// The guide for the command tree `root`.
pub fn render(root: &Command) -> String {
    GUIDE
        .replace("{{VERSION}}", env!("CARGO_PKG_VERSION"))
        .replace("{{MARKDOWN}}", MARKDOWN.trim_end())
        .replace("{{EXIT_STATUS}}", EXIT_STATUS)
        .replace("{{COMMANDS}}", &command_index(root))
}

/// One line per command a user can run, `linear <path>` then its one-line
/// description, with a blank line between root commands.
pub(super) fn command_index(root: &Command) -> String {
    let groups: Vec<Vec<(String, String)>> = visible(root)
        .filter(|command| !UNLISTED.contains(&command.get_name()))
        .map(|command| {
            let mut lines = Vec::new();
            collect(command, "linear", &mut lines);
            lines
        })
        .collect();
    let width = groups
        .iter()
        .flatten()
        .map(|(path, _)| path.len())
        .max()
        .unwrap_or_default();
    groups
        .iter()
        .map(|group| {
            group
                .iter()
                .map(|(path, about)| format!("{path:width$}  {about}").trim_end().to_owned())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn visible(command: &Command) -> impl Iterator<Item = &Command> {
    command
        .get_subcommands()
        .filter(|child| !child.is_hide_set() && child.get_name() != "help")
}

fn collect(command: &Command, parent: &str, lines: &mut Vec<(String, String)>) {
    let path = format!("{parent} {}", command.get_name());
    let about = command
        .get_about()
        .map(|about| about.to_string().replace('\n', " "))
        .unwrap_or_default();
    lines.push((path.clone(), about));
    for child in visible(command) {
        collect(child, &path, lines);
    }
}
