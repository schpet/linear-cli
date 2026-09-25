//! Real-fish contract for the project fish completion generator. Probes run
//! `complete -C` in `fish --no-config` with private HOME/XDG directories, an
//! empty PATH and a working directory holding only `somefile`, so file
//! completion is visible as that one candidate.
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::process::Command as Process;

use clap::{Arg, ArgAction, Command};
use linear_cli::cli::fish_completion;
use linear_cli::error::AppErrorKind;
use serde_json::Value;

use super::startup::BinarySandbox;

/// A completion candidate and its description, empty when fish shows none.
pub type Candidate = (String, String);

pub const FILE: &str = "somefile";

fn fish_program() -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .flat_map(|directory| [directory.join("fish"), directory.join("fish.exe")])
            .find(|program| program.is_file())
    })
}

/// Real-shell assertions run where fish is installed. The generator's pure
/// rejection test below still runs without it.
pub fn fish_available() -> bool {
    fish_program().is_some()
}

/// Source `script` once, then run `complete -C` for each line in order.
pub fn complete_lines(script: &[u8], lines: &[String]) -> Vec<BTreeSet<Candidate>> {
    let sandbox = BinarySandbox::new();
    let root = sandbox.root();
    for name in ["config", "data", "fish-cwd", "empty-path"] {
        fs::create_dir_all(root.join(name)).expect("create private fish directory");
    }
    fs::write(root.join("fish-cwd").join(FILE), "").expect("create file candidate");
    let script_path = root.join("completion.fish");
    fs::write(&script_path, script).expect("write fish completion script");
    let output = Process::new(fish_program().expect("real-fish test requires fish on PATH"))
        .env_clear()
        .current_dir(root.join("fish-cwd"))
        .env("HOME", root.join("home"))
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("XDG_DATA_HOME", root.join("data"))
        .env("XDG_DATA_DIRS", root.join("data"))
        .env("PATH", root.join("empty-path"))
        .env("LANG", "C.UTF-8")
        .arg("--no-config")
        .arg("-c")
        .arg("source $argv[1]; for line in $argv[2..-1]; echo \\x1f; complete -C $line; end")
        .arg(&script_path)
        .args(lines)
        .output()
        .expect("fish runs");
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 fish stderr");
    assert!(
        output.status.success() && stderr.is_empty(),
        "fish: {stderr}"
    );
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 fish stdout");
    let mut probes = stdout.split("\u{1f}\n");
    assert_eq!(probes.next(), Some(""), "output before the first probe");
    let result = probes
        .map(|probe| {
            probe
                .lines()
                .map(|line| match line.split_once('\t') {
                    Some((word, description)) => (word.to_owned(), description.to_owned()),
                    None => (line.to_owned(), String::new()),
                })
                .collect::<BTreeSet<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(result.len(), lines.len(), "one result per probe");
    result
}

pub fn words(candidates: &BTreeSet<Candidate>) -> BTreeSet<&str> {
    candidates.iter().map(|(word, _)| word.as_str()).collect()
}

fn switch(id: &'static str, short: Option<char>, long: &'static str) -> Arg {
    let mut arg = Arg::new(id)
        .long(long)
        .help(format!("{long} switch"))
        .action(ArgAction::SetTrue);
    if let Some(short) = short {
        arg = arg.short(short);
    }
    arg
}

/// Four levels, an alias, a hyphenated sibling that joins two command words,
/// valued parent options with a short spelling, and an enum value.
fn tree() -> Command {
    Command::new("tool")
        .disable_help_flag(true)
        .disable_version_flag(true)
        .disable_help_subcommand(true)
        .arg(switch("help", Some('h'), "help"))
        .arg(
            Arg::new("profile")
                .short('p')
                .long("profile")
                .help("Profile")
                .action(ArgAction::Set),
        )
        .subcommand(
            Command::new("a")
                .about("First level")
                .visible_alias("x")
                .arg(
                    Arg::new("mode")
                        .short('m')
                        .long("mode")
                        .help("Mode")
                        .value_parser(["fast", "slow"]),
                )
                .subcommand(
                    Command::new("b")
                        .about("Second level")
                        .arg(switch("bee", None, "bee"))
                        .subcommand(
                            Command::new("c").about("Third level").subcommand(
                                Command::new("d")
                                    .about("Fourth level")
                                    .arg(switch("deep", None, "deep"))
                                    .arg(
                                        Arg::new("leaf")
                                            .short('l')
                                            .long("leaf")
                                            .help("Leaf value")
                                            .num_args(1),
                                    )
                                    .arg(Arg::new("item").index(1)),
                            ),
                        ),
                ),
        )
        .subcommand(
            Command::new("a-b")
                .about("Hyphenated sibling")
                .arg(switch("hyphen", None, "hyphen")),
        )
}

fn set(words: &[&'static str]) -> BTreeSet<&'static str> {
    words.iter().copied().collect()
}

#[test]
fn fish_resolves_exact_paths_through_a_synthetic_tree() {
    if !fish_available() {
        eprintln!("skipping real-fish path test: fish is not on PATH");
        return;
    }
    let script = fish_completion::script(tree(), "tool").expect("valid synthetic tree");
    let root = set(&["a", "x", "a-b"]);
    let probes: Vec<(&str, BTreeSet<&str>)> = vec![
        ("tool ", root.clone()),
        ("tool -", set(&["-h", "--help", "-p", "--profile"])),
        ("tool a b c d -", set(&["--deep", "-l", "--leaf"])),
        ("tool x b c ", set(&["d"])),
        ("tool a b c d ", set(&[FILE])),
        ("tool a b c d item ", set(&[FILE])),
        ("tool a b -", set(&["--bee"])),
        ("tool a-b -", set(&["--hyphen"])),
        ("tool -p a ", root.clone()),
        ("tool -pa ", root.clone()),
        ("tool -hp a ", root.clone()),
        ("tool -hpa ", root.clone()),
        ("tool --profile a ", root.clone()),
        ("tool --profile=a ", root.clone()),
        ("tool --profile=a a -", set(&["-m", "--mode"])),
        ("tool --bogus ", set(&[FILE])),
        ("tool --bogus=value ", set(&[FILE])),
        ("tool -z ", set(&[FILE])),
        ("tool -hpz ", root.clone()),
        ("tool a -z ", set(&[FILE])),
        ("tool a --mode ", set(&["fast", "slow"])),
        ("tool a -m b ", set(&["b"])),
        ("tool a -mb ", set(&["b"])),
        ("tool a b c d b -", set(&["--deep", "-l", "--leaf"])),
        ("tool a b c d -lfoo -", set(&["--deep", "-l", "--leaf"])),
        (
            "tool a b c d --leaf -urgent -",
            set(&["--deep", "-l", "--leaf"]),
        ),
        ("tool -- ", set(&[FILE])),
        ("tool a -- b ", set(&[FILE])),
        ("tool a b c d -- -", set(&[])),
        ("tool z ", set(&[FILE])),
        ("tool z -", set(&[])),
        ("tool a b z -", set(&[])),
    ];
    let lines = probes
        .iter()
        .map(|(line, _)| (*line).to_owned())
        .collect::<Vec<_>>();
    for ((line, expected), actual) in probes.iter().zip(complete_lines(&script, &lines)) {
        assert_eq!(&words(&actual), expected, "{line:?}");
    }
}

#[test]
fn fish_navigates_every_aliased_parent_and_its_enum_descendants() {
    if !fish_available() {
        eprintln!("skipping real-fish alias test: fish is not on PATH");
        return;
    }
    let manifest_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../parity/manifest.json");
    let manifest: Value = serde_json::from_slice(&fs::read(manifest_path).expect("read manifest"))
        .expect("parse manifest");
    let routes = manifest["routes"].as_array().expect("routes array");
    let mut probes: Vec<(String, BTreeSet<String>)> = Vec::new();
    let mut aliased_parents = 0;
    for parent in routes {
        let path = parent["path"].as_str().expect("parent path");
        let prefix = format!("{path} ");
        let children = routes
            .iter()
            .filter(|route| {
                route["path"].as_str().is_some_and(|child_path| {
                    child_path.starts_with(&prefix) && !child_path[prefix.len()..].contains(' ')
                }) && route["hidden"] == false
            })
            .collect::<Vec<_>>();
        if children.is_empty() {
            continue;
        }
        let aliases = parent["aliases"].as_array().expect("alias array");
        if aliases.is_empty() {
            continue;
        }
        aliased_parents += 1;
        let expected_children = children
            .iter()
            .flat_map(|child| {
                std::iter::once(child["name"].as_str().expect("child name").to_owned()).chain(
                    child["aliases"]
                        .as_array()
                        .expect("child aliases")
                        .iter()
                        .map(|alias| alias.as_str().expect("child alias").to_owned()),
                )
            })
            .collect::<BTreeSet<_>>();
        for alias in aliases {
            let alias = alias.as_str().expect("parent alias");
            let (base, _) = path.rsplit_once(' ').expect("aliased parent has a parent");
            let alias_path = format!("{base} {alias}");
            probes.push((format!("{alias_path} "), expected_children.clone()));
            for descendant in routes {
                let descendant_path = descendant["path"].as_str().expect("descendant path");
                let Some(suffix) = descendant_path.strip_prefix(&prefix) else {
                    continue;
                };
                let types = descendant["localTypes"].as_array().expect("local types");
                for option in descendant["localOptions"]
                    .as_array()
                    .expect("local options")
                {
                    if option["hidden"] == true {
                        continue;
                    }
                    let Some(type_name) = option["args"]
                        .as_array()
                        .expect("option args")
                        .first()
                        .and_then(|arg| arg["type"].as_str())
                    else {
                        continue;
                    };
                    let Some(enum_type) = types.iter().find(|item| {
                        item["name"] == type_name && item["handlerKind"] == "EnumType"
                    }) else {
                        continue;
                    };
                    let Some(flag) = option["flags"]
                        .as_array()
                        .expect("option flags")
                        .iter()
                        .filter_map(Value::as_str)
                        .find(|flag| flag.starts_with("--"))
                    else {
                        continue;
                    };
                    let values = enum_type["values"]
                        .as_array()
                        .expect("enum values")
                        .iter()
                        .map(|value| value.as_str().expect("enum value").to_owned())
                        .collect();
                    probes.push((format!("{alias_path} {suffix} {flag} "), values));
                }
            }
        }
    }
    assert_eq!(aliased_parents, 11);
    let lines = probes
        .iter()
        .map(|(line, _)| line.clone())
        .collect::<Vec<_>>();
    let script = include_bytes!("expected/completions-fish.txt");
    for ((line, expected), actual) in probes.iter().zip(complete_lines(script, &lines)) {
        let actual = words(&actual)
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        assert_eq!(&actual, expected, "{line:?}");
    }
}

#[test]
fn fish_generator_rejects_trees_its_path_helper_cannot_model() {
    let hidden = tree().subcommand(Command::new("secret").about("Hidden").hide(true));
    let positional_parent = tree().arg(Arg::new("target").index(1));
    let multi_valued_parent = tree().arg(
        Arg::new("many")
            .long("many")
            .help("Many")
            .num_args(1..)
            .action(ArgAction::Set),
    );
    let glob_name = tree().subcommand(Command::new("a*").about("Glob"));
    let undescribed = tree().subcommand(Command::new("bare"));
    for (label, command) in [
        ("hidden", hidden),
        ("positional parent", positional_parent),
        ("multi-valued parent", multi_valued_parent),
        ("glob name", glob_name),
        ("undescribed", undescribed),
    ] {
        let error = fish_completion::script(command, "tool").expect_err(label);
        assert_eq!(error.kind, AppErrorKind::Invariant, "{label}");
    }
    for name in ["", "-tool", "a b", "t;x", "t'x"] {
        let error = fish_completion::script(tree(), name).expect_err(name);
        assert_eq!(error.kind, AppErrorKind::Invariant, "{name:?}");
    }
}
