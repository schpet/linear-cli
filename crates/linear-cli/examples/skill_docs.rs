//! Regenerates the agent skill in `skills/linear-cli/` from the CLI's own help:
//! `SKILL.md` (from `SKILL.template.md`) and one reference file per command.
//!
//! Run with `cargo run --example skill_docs`.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

/// Hand-written references kept alongside the generated ones.
const PRESERVED: &[&str] = &["organization-features.md"];

/// Help is wrapped at a fixed width so the output does not depend on the
/// terminal that runs the generator.
const HELP_WIDTH: usize = 100;

struct CommandDoc {
    name: String,
    path: Vec<String>,
    description: String,
    help: String,
    subcommands: Vec<CommandDoc>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let skill_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../skills/linear-cli");
    let mut root = linear_cli::cli::command().term_width(HELP_WIDTH);
    root.build();

    let mut commands = documented_subcommands(&root)
        .filter(|command| command.get_name() != "completions")
        .map(|command| document(&root, command, Vec::new()))
        .collect::<Result<Vec<_>, _>>()?;
    commands.sort_by(|a, b| a.name.cmp(&b.name));

    // Render everything before touching the skill directory, so a failure
    // leaves the committed docs intact.
    let template = fs::read_to_string(skill_dir.join("SKILL.template.md"))?;
    for placeholder in ["{{COMMANDS}}", "{{REFERENCE_TOC}}"] {
        if !template.contains(placeholder) {
            return Err(format!("SKILL.template.md is missing {placeholder}").into());
        }
    }
    let skill = template
        .replace("{{COMMANDS}}", &command_list(&commands))
        .replace("{{REFERENCE_TOC}}", &reference_toc(&commands));
    let mut references: Vec<(String, String)> = commands
        .iter()
        .map(|command| (format!("{}.md", command.name), reference(command)))
        .collect();
    references.push(("commands.md".to_owned(), index(&commands)));

    let references_dir = skill_dir.join("references");
    fs::create_dir_all(&references_dir)?;
    for (file, content) in &references {
        fs::write(references_dir.join(file), content)?;
    }
    for stale in stale_references(&references_dir, &references)? {
        fs::remove_file(stale)?;
    }
    fs::write(skill_dir.join("SKILL.md"), skill)?;
    eprintln!(
        "Wrote skills/linear-cli/SKILL.md and {} reference files",
        references.len()
    );
    Ok(())
}

fn documented_subcommands(command: &clap::Command) -> impl Iterator<Item = &clap::Command> {
    command
        .get_subcommands()
        .filter(|child| !child.is_hide_set() && child.get_name() != "help")
}

fn document(
    root: &clap::Command,
    command: &clap::Command,
    parent: Vec<String>,
) -> Result<CommandDoc, Box<dyn Error>> {
    let mut path = parent;
    path.push(command.get_name().to_owned());
    let subcommands = documented_subcommands(command)
        .map(|child| document(root, child, path.clone()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CommandDoc {
        name: command.get_name().to_owned(),
        description: command
            .get_about()
            .map(|about| about.to_string().replace('\n', " "))
            .unwrap_or_default(),
        help: help(root, &path)?,
        path,
        subcommands,
    })
}

/// The `--help` output for `linear <path>`, exactly as a user sees it.
fn help(root: &clap::Command, path: &[String]) -> Result<String, Box<dyn Error>> {
    let arguments = std::iter::once("linear")
        .chain(path.iter().map(String::as_str))
        .chain(std::iter::once("--help"));
    match root.clone().try_get_matches_from(arguments) {
        Err(error) if error.kind() == clap::error::ErrorKind::DisplayHelp => {
            Ok(error.render().to_string().trim_end().to_owned())
        }
        Err(error) => Err(error.into()),
        Ok(_) => Err(format!("`linear {} --help` did not print help", path.join(" ")).into()),
    }
}

fn full_name(command: &CommandDoc) -> String {
    format!("linear {}", command.path.join(" "))
}

fn command_paths(command: &CommandDoc) -> Vec<String> {
    std::iter::once(full_name(command))
        .chain(command.subcommands.iter().flat_map(command_paths))
        .collect()
}

fn command_list(commands: &[CommandDoc]) -> String {
    commands
        .iter()
        .map(|command| command_paths(command).join("\n"))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn reference_toc(commands: &[CommandDoc]) -> String {
    commands
        .iter()
        .map(|command| {
            format!(
                "- [{0}](references/{0}.md) - {1}",
                command.name, command.description
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn fenced(content: &str) -> String {
    format!("```\n{content}\n```\n")
}

fn heading(level: usize, text: &str) -> String {
    format!("{} {text}\n", "#".repeat(level.min(6)))
}

fn reference(command: &CommandDoc) -> String {
    let mut out = heading(1, &command.name);
    out.push_str(&format!("\n> {}\n\n", command.description));
    out.push_str(&heading(2, "Usage"));
    out.push('\n');
    out.push_str(&fenced(&command.help));
    if !command.subcommands.is_empty() {
        out.push('\n');
        out.push_str(&heading(2, "Subcommands"));
        for subcommand in &command.subcommands {
            subcommand_section(&mut out, subcommand, 3);
        }
    }
    out
}

fn subcommand_section(out: &mut String, command: &CommandDoc, level: usize) {
    out.push('\n');
    out.push_str(&heading(level, &command.name));
    out.push('\n');
    if !command.description.is_empty() {
        out.push_str(&format!("> {}\n\n", command.description));
    }
    out.push_str(&fenced(&command.help));
    if !command.subcommands.is_empty() {
        out.push('\n');
        out.push_str(&heading(
            level + 1,
            &format!("{} subcommands", command.name),
        ));
        for subcommand in &command.subcommands {
            subcommand_section(out, subcommand, level + 2);
        }
    }
}

fn index(commands: &[CommandDoc]) -> String {
    let entries = commands
        .iter()
        .map(|command| format!("- [{0}](./{0}.md) - {1}", command.name, command.description))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "# Linear CLI Command Reference\n\n## Commands\n\n{entries}\n\n## Quick Reference\n\n\
         ```bash\n# Get help for any command\nlinear <command> --help\n\
         linear <command> <subcommand> --help\n```\n"
    )
}

/// Markdown files in `dir` that this run neither generated nor preserves.
fn stale_references(
    dir: &Path,
    generated: &[(String, String)],
) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut stale = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        let Some(file) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let keep = generated.iter().any(|(name, _)| name == file) || PRESERVED.contains(&file);
        if file.ends_with(".md") && !keep {
            stale.push(path);
        }
    }
    Ok(stale)
}
