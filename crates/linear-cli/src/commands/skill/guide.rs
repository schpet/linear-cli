//! MiniJinja guides assembled from effective command hooks and canonical CLI facts.
use clap::Command;

use crate::cli::EXIT_STATUS;

use crate::error::{Error, Result};
use crate::templates::{SkillContext, Slot, TemplateConfig, skill_hook};

const MARKDOWN: &str = include_str!("../../cli/markdown.txt");
const UNLISTED: &[&str] = &["help", "completions"];

/// Render every visible command hook, or the ancestors and subtree of a target.
pub fn render(root: &Command, templates: &TemplateConfig, target: &[String]) -> Result<String> {
    let mut root = root.clone();
    root.build();
    let mut selected = &root;
    let mut path = Vec::new();
    let mut hooks = Vec::new();
    for part in target {
        let child = visible(selected)
            .find(|child| {
                child.get_name() == part || child.get_all_aliases().any(|alias| alias == part)
            })
            .ok_or_else(|| {
                let requested = path
                    .iter()
                    .chain(std::iter::once(part))
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" ");
                Error::invalid(format!("Unknown skill command path: linear {requested}")).with_hint(
                    "Run `linear --help` or `linear COMMAND --help` for visible command names.",
                )
            })?;
        path.push(child.get_name().to_owned());
        hooks.push(hook(&path));
        selected = child;
    }
    collect_hooks(selected, &mut path.clone(), &mut hooks);
    let command_help = if target.is_empty() {
        String::new()
    } else {
        let mut command = selected
            .clone()
            .bin_name(format!("linear {}", path.join(" ")));
        format!(
            "## Command help\n\n```text\n{}```\n\n",
            command.render_long_help()
        )
    };
    let mut context = SkillContext {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        commands: command_index(&root),
        command_help,
        markdown_rules: MARKDOWN.trim_end().to_owned(),
        exit_status: EXIT_STATUS.to_owned(),
        ..SkillContext::default()
    };
    let mut guidance = String::new();
    for name in hooks {
        let text = templates.render_named(&name, &context)?;
        if !text.trim().is_empty() {
            guidance.push_str(&text);
            if !text.ends_with("\n\n") {
                guidance.push_str(if text.ends_with('\n') { "\n" } else { "\n\n" });
            }
        }
    }
    context.command_guidance = guidance;
    templates.render(
        if target.is_empty() {
            Slot::Guide
        } else {
            Slot::GuideTargeted
        },
        &context,
    )
}

fn hook(path: &[String]) -> String {
    skill_hook(&path.iter().map(String::as_str).collect::<Vec<_>>())
}

fn collect_hooks(command: &Command, path: &mut Vec<String>, hooks: &mut Vec<String>) {
    for child in visible(command) {
        path.push(child.get_name().to_owned());
        hooks.push(hook(path));
        collect_hooks(child, path, hooks);
        path.pop();
    }
}

/// One line per command a user can run, `linear <path>` then its one-line
/// description.
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
