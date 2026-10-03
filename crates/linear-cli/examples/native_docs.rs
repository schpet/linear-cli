//! Development-only typed clap manifest, bound to an immutable binary SHA.
//! No startup, HTTP, credentials or application command execution.
use clap::Parser;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    binary: PathBuf,
    #[arg(long)]
    binary_sha256: String,
}
#[derive(Serialize)]
struct CommandDoc {
    name: String,
    description: String,
    help: String,
    subcommands: Vec<CommandDoc>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    version: &'static str,
    binary_sha256: String,
    root_help: String,
    commands: Vec<CommandDoc>,
}
fn help(root: &clap::Command, path: &[String]) -> Result<String, Box<dyn std::error::Error>> {
    let arguments = std::iter::once("linear")
        .chain(path.iter().map(String::as_str))
        .chain(std::iter::once("--help"));
    match root.clone().try_get_matches_from(arguments) {
        Err(error) if error.kind() == clap::error::ErrorKind::DisplayHelp => {
            Ok(error.render().to_string())
        }
        Err(error) => Err(error.into()),
        Ok(_) => Err("typed help request unexpectedly selected an action".into()),
    }
}
fn document(
    root: &clap::Command,
    command: &clap::Command,
    path: Vec<String>,
) -> Result<CommandDoc, Box<dyn std::error::Error>> {
    let description = command
        .get_about()
        .map(ToString::to_string)
        .unwrap_or_default();
    let subcommands = command
        .get_subcommands()
        .filter(|child| !child.is_hide_set() && child.get_name() != "help")
        .map(|child| {
            let mut child_path = path.clone();
            child_path.push(child.get_name().to_owned());
            document(root, child, child_path)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let help = help(root, &path)?;
    Ok(CommandDoc {
        name: path.join(" "),
        description,
        help,
        subcommands,
    })
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if !args.binary.is_absolute()
        || args.binary_sha256.len() != 64
        || !args
            .binary_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err("absolute binary and lowercase SHA256 required".into());
    }
    let actual: String = Sha256::digest(fs::read(&args.binary)?)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if actual != args.binary_sha256 {
        return Err("native documentation binary SHA mismatch".into());
    }
    let mut root = linear_cli::cli::command();
    root.build();
    let commands = root
        .get_subcommands()
        .filter(|child| {
            !child.is_hide_set() && child.get_name() != "help" && child.get_name() != "completions"
        })
        .map(|child| document(&root, child, vec![child.get_name().to_owned()]))
        .collect::<Result<Vec<_>, _>>()?;
    let manifest = Manifest {
        version: env!("CARGO_PKG_VERSION"),
        binary_sha256: actual,
        root_help: help(&root, &[])?,
        commands,
    };
    println!("{}", serde_json::to_string_pretty(&manifest)?);
    Ok(())
}
