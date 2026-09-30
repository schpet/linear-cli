//! C086 public binary contract for `completions` and the hidden `complete` shim.
//! Route, flag, alias and enum expectations are derived from the pinned
//! `rust/parity/manifest.json`, never read back from clap.
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

use super::fish_completion::{Candidate, FILE, complete_lines, fish_available, words};
use super::startup::BinarySandbox;

const BASH: &[u8] = include_bytes!("expected/completions-bash.txt");
const FISH: &[u8] = include_bytes!("expected/completions-fish.txt");
const ZSH: &[u8] = include_bytes!("expected/completions-zsh.txt");
const NAME_SUGGESTION: &str =
    "  Use ASCII letters, digits, '_', '-' or '.', starting with a letter, digit or '_'.\n";

struct Route {
    path: String,
    name: String,
    aliases: Vec<String>,
    summary: String,
    hidden: bool,
    has_arguments: bool,
    offered_flags: BTreeSet<String>,
    enums: Vec<(String, Vec<String>)>,
    enum_options: Vec<(String, Vec<String>)>,
}

impl Route {
    fn words(&self) -> Vec<&str> {
        self.path.split(' ').skip(1).collect()
    }
}

fn parity_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../parity")
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .expect("string array")
        .iter()
        .map(|item| item.as_str().expect("string item").to_owned())
        .collect()
}

/// The v3 spelling rules the reviewed clap tree applies to the inventory.
fn inventory() -> Vec<Route> {
    let manifest: Value = serde_json::from_slice(
        &fs::read(parity_root().join("manifest.json")).expect("read pinned manifest"),
    )
    .expect("valid manifest JSON");
    let routes = manifest["routes"].as_array().expect("manifest routes");
    assert_eq!(routes.len(), 110);
    let mut result = Vec::new();
    for route in routes {
        let path = route["path"].as_str().expect("route path").to_owned();
        let mut offered_flags = BTreeSet::from(["-h".to_owned(), "--help".to_owned()]);
        if path == "linear" {
            offered_flags.extend(["-V".to_owned(), "--version".to_owned()]);
        }
        offered_flags.insert("--workspace".to_owned());
        let types = route["localTypes"].as_array().expect("local types");
        let enums = types
            .iter()
            .filter(|definition| definition["handlerKind"] == "EnumType")
            .map(|definition| {
                (
                    definition["name"].as_str().expect("type name").to_owned(),
                    strings(&definition["values"]),
                )
            })
            .collect::<Vec<_>>();
        let mut enum_options = Vec::new();
        for option in route["localOptions"].as_array().expect("local options") {
            if option["hidden"].as_bool().expect("hidden flag") {
                continue;
            }
            let mut flags = strings(&option["flags"]);
            if path == "linear label list" && option["name"] == "workspace" {
                flags = vec!["--workspace-only".to_owned()];
            }
            if let Some(type_name) = option["args"]
                .as_array()
                .expect("option arguments")
                .first()
                .and_then(|argument| argument["type"].as_str())
                && let Some((_, values)) = enums.iter().find(|(name, _)| name == type_name)
                && let Some(long) = flags.iter().find(|flag| flag.starts_with("--"))
            {
                enum_options.push((long.clone(), values.clone()));
            }
            offered_flags.extend(flags);
        }
        let description = route["description"].as_str().expect("description");
        result.push(Route {
            name: route["name"].as_str().expect("route name").to_owned(),
            aliases: strings(&route["aliases"]),
            summary: description
                .split('\n')
                .next()
                .expect("summary line")
                .to_owned(),
            hidden: route["hidden"].as_bool().expect("route hidden"),
            has_arguments: !route["arguments"].as_array().expect("arguments").is_empty(),
            offered_flags,
            enums,
            enum_options,
            path,
        });
    }
    result
}

fn visible(routes: &[Route]) -> Vec<&Route> {
    routes
        .iter()
        .filter(|route| {
            !routes.iter().any(|ancestor| {
                ancestor.hidden
                    && (route.path == ancestor.path
                        || route.path.starts_with(&format!("{} ", ancestor.path)))
            })
        })
        .collect()
}

fn parent<'a>(routes: &[&'a Route], route: &Route) -> Option<&'a Route> {
    let (parent, _) = route.path.rsplit_once(' ')?;
    routes
        .iter()
        .copied()
        .find(|candidate| candidate.path == parent)
}

fn children<'a>(routes: &[&'a Route], route: &Route) -> Vec<&'a Route> {
    routes
        .iter()
        .copied()
        .filter(|candidate| parent(routes, candidate).is_some_and(|p| p.path == route.path))
        .collect()
}

fn command(sandbox: &BinarySandbox) -> Command {
    let root = sandbox.root();
    fs::create_dir_all(root.join("config")).expect("create private config home");
    let mut command = Command::new(env!("CARGO_BIN_EXE_linear"));
    command
        .env_clear()
        .current_dir(root.join("cwd"))
        .env("HOME", root.join("home"))
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("APPDATA", root.join("config"))
        .env("PATH", root.join("bin"))
        .env("TZ", "UTC")
        .env("LANG", "C.UTF-8")
        .env("LINEAR_IGNORE_ENV_FILE", "1")
        .env("LINEAR_GRAPHQL_ENDPOINT", "http://127.0.0.1:1/graphql")
        .env("NO_COLOR", "1");
    command
}

fn run(args: &[&str]) -> Output {
    let sandbox = BinarySandbox::new();
    command(&sandbox)
        .args(args)
        .output()
        .expect("completions public binary runs")
}

fn text(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).expect("UTF-8 output")
}

fn assert_success(output: &Output, stdout: &[u8], label: &str) {
    assert_eq!(output.status.code(), Some(0), "{label} exit");
    assert!(
        output.stderr.is_empty(),
        "{label} stderr: {:?}",
        text(&output.stderr)
    );
    assert!(output.stdout == stdout, "{label} stdout differs");
}

#[test]
fn scripts_match_reviewed_v3_goldens_with_the_literal_default_name() {
    for (shell, golden) in [("bash", BASH), ("fish", FISH), ("zsh", ZSH)] {
        assert_success(&run(&["completions", shell]), golden, shell);
    }
    assert!(text(BASH).ends_with("    complete -F _linear -o bashdefault -o default linear\nfi\n"));
    assert!(
        text(FISH)
            .lines()
            .filter(|line| line.starts_with("complete "))
            .all(|line| line.starts_with("complete -c linear "))
    );
    assert!(text(ZSH).starts_with("#compdef linear\n"));
    assert!(text(ZSH).ends_with("    compdef _linear linear\nfi\n"));
}

#[test]
fn name_option_forms_rename_every_registration() {
    let bash = run(&["completions", "bash", "-n", "foo"]);
    assert_eq!(bash.status.code(), Some(0));
    let bash = text(&bash.stdout);
    assert!(bash.starts_with("_foo() {\n"));
    assert!(bash.contains("    complete -F _foo -o nosort -o bashdefault -o default foo\n"));
    assert!(!bash.contains("complete -F _linear"));
    let fish = run(&["completions", "fish", "--name", "foo"]);
    assert_eq!(fish.status.code(), Some(0));
    let fish = text(&fish.stdout);
    assert!(fish.contains("function __fish_foo_command_path\n"));
    assert!(!fish.contains("__fish_linear_"));
    assert!(
        fish.lines()
            .filter(|line| line.starts_with("complete "))
            .all(|line| line.starts_with("complete -c foo -n \"__fish_foo_using_command '"))
    );
    let dotted = run(&["completions", "fish", "--name", "linear-v3.dev_1"]);
    assert_eq!(dotted.status.code(), Some(0));
    assert!(text(&dotted.stdout).contains(
        "complete -c linear-v3.dev_1 -n \"__fish_linear_v3_dev_1_using_command 'linear issue'\""
    ));
    let zsh = run(&["completions", "zsh", "--name=foo"]);
    assert_eq!(zsh.status.code(), Some(0));
    let zsh = text(&zsh.stdout);
    assert!(zsh.starts_with("#compdef foo\n"));
    assert!(zsh.ends_with("    compdef _foo foo\nfi\n"));
    let dotted = run(&["completions", "bash", "--name", "linear-v3.dev_1"]);
    assert_eq!(dotted.status.code(), Some(0));
    assert!(text(&dotted.stdout).contains("complete -F _linear__v3.dev_1 "));
}

#[test]
fn unsafe_or_missing_names_fail_before_generation() {
    for name in ["-foo", "a;b $(c)", "x y", "q'q", "é"] {
        let output = run(&["completions", "bash", &format!("--name={name}")]);
        assert_eq!(output.status.code(), Some(1), "{name}");
        assert!(output.stdout.is_empty(), "{name}");
        assert_eq!(
            text(&output.stderr),
            format!("✗ Invalid command name \"{name}\"\n{NAME_SUGGESTION}"),
            "{name}"
        );
    }
    let help = run(&["completions", "bash", "--help"]);
    assert_eq!(help.status.code(), Some(0));
    for args in [
        &["completions", "bash", "--name"][..],
        &["completions", "bash", "--name", ""][..],
    ] {
        let output = run(args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        let words = std::iter::once("linear").chain(args.iter().copied());
        let native = linear_cli::cli::command()
            .try_get_matches_from(words)
            .expect_err("native missing name");
        assert!(output.stdout.is_empty(), "{args:?}");
        assert_eq!(
            text(&output.stderr),
            native.render().to_string(),
            "{args:?}"
        );
    }
}

#[test]
fn completions_parent_prints_native_help_and_inherits_workspace() {
    let help = run(&["completions", "--help"]);
    assert_success(&run(&["completions"]), &help.stdout, "bare parent");
    assert_success(
        &run(&["completions", "bash", "--workspace", "x"]),
        BASH,
        "global workspace",
    );
    assert_success(
        &run(&["--workspace", "x", "completions", "bash"]),
        BASH,
        "root global workspace",
    );
    assert_success(
        &run(&[
            "completions",
            "complete",
            "sort",
            "issue",
            "mine",
            "--workspace",
            "x",
        ]),
        b"manual\npriority",
        "hidden complete global workspace",
    );
}

fn bash_function(words: &[&str]) -> String {
    let mut name = String::from("linear");
    for word in words {
        name.push_str("__subcmd__");
        name.push_str(&word.replace('-', "__subcmd__"));
    }
    name
}

/// Every `opts="..."` word list of a bash `case "${cmd}"` arm, in order.
fn bash_arms<'a>(script: &'a str, function: &str) -> Vec<BTreeSet<&'a str>> {
    let marker = format!("\n        {function})\n            opts=\"");
    script
        .match_indices(&marker)
        .map(|(start, _)| {
            let rest = &script[start + marker.len()..];
            let end = rest.find('"').expect("closing opts quote");
            rest[..end].split(' ').collect()
        })
        .collect()
}

fn expected_bash_words<'a>(routes: &[&'a Route], route: &'a Route) -> BTreeSet<&'a str> {
    let mut words = route
        .offered_flags
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    for child in children(routes, route) {
        words.insert(&child.name);
        words.extend(child.aliases.iter().map(String::as_str));
    }
    if !children(routes, route).is_empty() {
        words.insert("help");
    }
    words
}

#[test]
fn bash_offers_every_manifest_route_alias_flag_and_enum() {
    let all = inventory();
    let routes = visible(&all);
    assert_eq!(routes.len(), 109);
    let script = text(BASH);
    let mut functions: BTreeMap<String, Vec<&Route>> = BTreeMap::new();
    for route in &routes {
        functions
            .entry(bash_function(&route.words()))
            .or_default()
            .push(route);
    }
    // Reviewed deviation C086-BASH-HYPHEN-COLLISION: clap_complete maps `-`
    // and a subcommand boundary to the same separator, and bash takes the
    // first matching arm, so `project update` completes as `project-update`.
    let collisions = functions
        .iter()
        .filter(|(_, routes)| routes.len() > 1)
        .map(|(function, routes)| {
            (
                function.as_str(),
                routes
                    .iter()
                    .map(|route| route.path.as_str())
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        collisions,
        [
            (
                "linear__subcmd__initiative__subcmd__update",
                vec!["linear initiative update", "linear initiative-update"]
            ),
            (
                "linear__subcmd__project__subcmd__update",
                vec!["linear project update", "linear project-update"]
            ),
        ]
    );
    for (function, owners) in &functions {
        let arms = bash_arms(script, function);
        let owner = match owners.as_slice() {
            [route] => {
                assert_eq!(arms.len(), 1, "{function}");
                route
            }
            [update, hyphenated] => {
                assert_eq!(arms.len(), 2, "{function}");
                assert_eq!(arms[1], expected_bash_words(&routes, update), "{function}");
                hyphenated
            }
            _ => panic!("unexpected collision {function}"),
        };
        assert_eq!(
            arms[0],
            expected_bash_words(&routes, owner),
            "{}",
            owner.path
        );
    }
    let mut alias_pairs = 0;
    for route in &routes {
        let words = route.words();
        let Some((_, parent_words)) = words.split_last() else {
            continue;
        };
        let parent = bash_function(parent_words);
        let target = bash_function(&words);
        for word in std::iter::once(&route.name).chain(&route.aliases) {
            let arm = format!(
                "\n            {parent},{word})\n                cmd=\"{target}\"\n                ;;"
            );
            assert_eq!(script.matches(&arm).count(), 1, "{} via {word}", route.path);
        }
        alias_pairs += route.aliases.len();
    }
    assert_eq!(alias_pairs, 36);
    for route in &routes {
        for (flag, values) in &route.enum_options {
            let branch = format!(
                "\n                {flag})\n                    COMPREPLY=($(compgen -W \"{}\" -- \"${{cur}}\"))\n",
                values.join(" ")
            );
            assert!(script.contains(&branch), "{} {flag}", route.path);
        }
    }
}

#[test]
fn scripts_omit_hidden_routes_hidden_options_and_internal_ids() {
    for (label, script) in [("bash", BASH), ("fish", FISH), ("zsh", ZSH)] {
        let script = text(script);
        for internal in [
            "internal:",
            "pos:",
            "opt:",
            "global:",
            "help:short",
            "version:long",
        ] {
            assert!(!script.contains(internal), "{label} exposes {internal}");
        }
        assert!(!script.contains("completions complete"), "{label}");
        assert!(
            !script.contains("Get completions for given action"),
            "{label}"
        );
    }
    let bash = text(BASH);
    assert!(!bash.contains("linear__subcmd__completions__subcmd__complete"));
    let mine = &bash_arms(bash, &bash_function(&["issue", "mine"]))[0];
    for hidden in ["--assignee", "-A", "--all-assignees", "-U", "--unassigned"] {
        assert!(!mine.contains(hidden), "issue mine offers hidden {hidden}");
    }
    let comment_add = &bash_arms(bash, &bash_function(&["issue", "comment", "add"]))[0];
    assert!(!comment_add.contains("--id"));
    assert!(comment_add.contains("--reply-to") && comment_add.contains("--workspace"));
    let label_list = &bash_arms(bash, &bash_function(&["label", "list"]))[0];
    assert!(label_list.contains("--workspace") && label_list.contains("--workspace-only"));
    let completions = &bash_arms(bash, &bash_function(&["completions"]))[0];
    assert_eq!(
        completions,
        &BTreeSet::from(["-h", "--help", "--workspace", "bash", "fish", "zsh", "help"])
    );
}

fn zsh_escape(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('\'', "'\\''")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace(':', "\\:")
        .replace('$', "\\$")
        .replace('`', "\\`")
        .replace('\n', " ")
}

#[test]
fn zsh_lists_every_alias_and_quotes_descriptions() {
    let all = inventory();
    let routes = visible(&all);
    let script = text(ZSH);
    for route in &routes {
        let words = route.words();
        let Some((_, parent_words)) = words.split_last() else {
            continue;
        };
        let function = format!(
            "\n_{}_commands() {{\n    local commands; commands=(\n",
            std::iter::once("linear")
                .chain(parent_words.iter().copied())
                .collect::<Vec<_>>()
                .join("__subcmd__")
        );
        let start = script.find(&function).expect("zsh commands function") + function.len();
        let body = &script[start..start + script[start..].find("\n    )").expect("list end")];
        for word in std::iter::once(&route.name).chain(&route.aliases) {
            let entry = format!("'{word}:{}' \\", zsh_escape(&route.summary));
            assert!(
                body.lines().any(|line| line == entry),
                "{} via {word}",
                route.path
            );
            let arm = format!("({word})");
            assert!(
                script.lines().any(|line| line.trim_start() == arm),
                "{} arm {word}",
                route.path
            );
        }
    }
    let describe = all
        .iter()
        .find(|route| route.path == "linear issue describe")
        .expect("issue describe");
    assert!(describe.offered_flags.contains("--ref"));
    assert!(script.contains(
        "'-r[Use '\\''References'\\'' instead of '\\''Fixes'\\'' for the Linear issue link]' \\\n"
    ));
    assert!(script.contains(
        "'--ref[Use '\\''References'\\'' instead of '\\''Fixes'\\'' for the Linear issue link]' \\\n"
    ));
    assert!(
        script.contains(
            "'--workspace-only[Show only workspace-level labels (not team-specific)]' \\\n"
        )
    );
}

fn route<'a>(routes: &[&'a Route], path: &str) -> &'a Route {
    routes
        .iter()
        .copied()
        .find(|route| route.path == path)
        .unwrap_or_else(|| panic!("visible route {path}"))
}

fn flag_words(route: &Route) -> BTreeSet<&str> {
    route.offered_flags.iter().map(String::as_str).collect()
}

/// Every command word a parent offers, with its first description line.
fn child_candidates(routes: &[&Route], route: &Route) -> BTreeSet<Candidate> {
    let mut candidates = children(routes, route)
        .into_iter()
        .flat_map(|child| {
            std::iter::once(&child.name)
                .chain(&child.aliases)
                .map(|word| (word.clone(), child.summary.clone()))
        })
        .collect::<BTreeSet<_>>();
    if !children(routes, route).is_empty() {
        candidates.insert((
            "help".to_owned(),
            "Print this message or the help of the given subcommand(s)".to_owned(),
        ));
    }
    candidates
}

/// The command line that selects `words`, ending with a space.
fn line(words: &[&str]) -> String {
    let mut line = String::from("linear ");
    for word in words {
        line.push_str(word);
        line.push(' ');
    }
    line
}

#[test]
fn fish_completes_every_route_alias_flag_and_enum_at_every_depth() {
    if !fish_available() {
        eprintln!("skipping real-fish inventory test: fish is not on PATH");
        return;
    }
    let all = inventory();
    let routes = visible(&all);
    let mut probes: Vec<(String, BTreeSet<Candidate>)> = Vec::new();
    let mut flag_probes: Vec<(String, BTreeSet<&str>)> = Vec::new();
    let mut alias_pairs = 0;
    for route in &routes {
        let words = route.words();
        flag_probes.push((format!("{}-", line(&words)), flag_words(route)));
        if !children(&routes, route).is_empty() {
            probes.push((line(&words), child_candidates(&routes, route)));
        }
        for (flag, values) in &route.enum_options {
            let expected = values
                .iter()
                .map(|value| (value.clone(), String::new()))
                .collect();
            probes.push((format!("{}{flag} ", line(&words)), expected));
        }
        if let Some((_, parent_words)) = words.split_last() {
            for alias in &route.aliases {
                let mut alias_words = parent_words.to_vec();
                alias_words.push(alias.as_str());
                flag_probes.push((format!("{}-", line(&alias_words)), flag_words(route)));
                alias_pairs += 1;
            }
        }
    }
    assert_eq!(alias_pairs, 36);
    let deep = routes
        .iter()
        .filter(|route| route.words().len() == 3)
        .count();
    assert_eq!(deep, 15);
    let lines = probes
        .iter()
        .map(|(line, _)| line.clone())
        .chain(flag_probes.iter().map(|(line, _)| line.clone()))
        .collect::<Vec<_>>();
    let results = complete_lines(FISH, &lines);
    let (exact, flags) = results.split_at(probes.len());
    for ((line, expected), actual) in probes.iter().zip(exact) {
        assert_eq!(actual, expected, "{line:?}");
    }
    for ((line, expected), actual) in flag_probes.iter().zip(flags) {
        assert_eq!(&words(actual), expected, "{line:?}");
    }
}

#[test]
fn fish_selects_exact_paths_at_collisions_values_and_boundaries() {
    if !fish_available() {
        eprintln!("skipping real-fish path test: fish is not on PATH");
        return;
    }
    let all = inventory();
    let routes = visible(&all);
    let flags = |path: &str| flag_words(route(&routes, path));
    let commands = |path: &str| {
        child_candidates(&routes, route(&routes, path))
            .into_iter()
            .map(|(word, _)| word)
            .collect::<BTreeSet<_>>()
    };
    let file = BTreeSet::from([FILE]);
    let nothing = BTreeSet::new();
    let owned = |set: BTreeSet<&str>| set.into_iter().map(str::to_owned).collect::<BTreeSet<_>>();
    assert_ne!(
        flags("linear issue update"),
        flags("linear issue comment update")
    );
    assert_ne!(
        flags("linear project update"),
        flags("linear project-update")
    );
    assert!(route(&routes, "linear project update").has_arguments);
    assert!(route(&routes, "linear initiative update").has_arguments);
    assert!(!route(&routes, "linear issue mine").has_arguments);
    let probes: Vec<(&str, BTreeSet<String>)> = vec![
        // Nested and hyphenated command paths stay distinct.
        ("linear issue update -", owned(flags("linear issue update"))),
        (
            "linear issue comment update -",
            owned(flags("linear issue comment update")),
        ),
        (
            "linear project update -",
            owned(flags("linear project update")),
        ),
        (
            "linear project-update -",
            owned(flags("linear project-update")),
        ),
        ("linear project update ", owned(file.clone())),
        ("linear project-update ", commands("linear project-update")),
        ("linear pu -", owned(flags("linear project-update"))),
        ("linear p update -", owned(flags("linear project update"))),
        ("linear initiative update ", owned(file.clone())),
        (
            "linear initiative-update ",
            commands("linear initiative-update"),
        ),
        (
            "linear initiative-update list -",
            owned(flags("linear initiative-update list")),
        ),
        // A valued option's word is a value, never a command.
        ("linear --workspace issue ", commands("linear")),
        ("linear --workspace=issue ", commands("linear")),
        (
            "linear issue --workspace comment ",
            commands("linear issue"),
        ),
        (
            "linear issue --workspace comment -",
            owned(flags("linear issue")),
        ),
        (
            "linear issue comment --workspace update -",
            owned(flags("linear issue comment")),
        ),
        (
            "linear --workspace x i --workspace y agent-session list --status ",
            owned(BTreeSet::from([
                "pending",
                "active",
                "complete",
                "awaitingInput",
                "error",
                "stale",
            ])),
        ),
        ("linear issue comment add --body-file ", owned(file.clone())),
        // A valued short option on a leaf consumes the remainder of its
        // cluster; its attached value is not another short option.
        (
            "linear issue mine -sactive -",
            owned(flags("linear issue mine")),
        ),
        (
            "linear issue mine -s active -",
            owned(flags("linear issue mine")),
        ),
        (
            "linear issue create -p1 --st",
            owned(BTreeSet::from(["--start", "--state"])),
        ),
        (
            "linear issue create -tBug --st",
            owned(BTreeSet::from(["--start", "--state"])),
        ),
        (
            "linear issue create --title -urgent --st",
            owned(BTreeSet::from(["--start", "--state"])),
        ),
        (
            "linear issue comment add -bhi -",
            owned(flags("linear issue comment add")),
        ),
        // `--` ends command and option completion.
        ("linear -- ", owned(file.clone())),
        ("linear issue -- ", owned(file.clone())),
        ("linear issue -- comment -", owned(nothing.clone())),
        ("linear issue mine -- -", owned(nothing.clone())),
        ("linear issue view abc -- -", owned(nothing.clone())),
        // A selected command offers no further command words.
        ("linear issue mine ", owned(nothing.clone())),
        (
            "linear issue mine update -",
            owned(flags("linear issue mine")),
        ),
        ("linear issue bogus ", owned(file.clone())),
        ("linear issue bogus -", owned(nothing.clone())),
        // Hidden routes and root globals follow the tree.
        ("linear completions ", commands("linear completions")),
        ("linear completions complete ", owned(file.clone())),
        ("linear completions complete -", owned(nothing.clone())),
        (
            "linear completions bash -",
            owned(BTreeSet::from([
                "-h",
                "--help",
                "-n",
                "--name",
                "--workspace",
            ])),
        ),
        (
            "linear label list --workspace",
            owned(BTreeSet::from(["--workspace", "--workspace-only"])),
        ),
    ];
    let lines = probes
        .iter()
        .map(|(line, _)| (*line).to_owned())
        .collect::<Vec<_>>();
    for ((line, expected), actual) in probes.iter().zip(complete_lines(FISH, &lines)) {
        let actual = words(&actual)
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        assert_eq!(&actual, expected, "{line:?}");
    }
}

fn complete(args: &[&str]) -> Output {
    let mut argv = vec!["completions", "complete"];
    argv.extend_from_slice(args);
    run(&argv)
}

#[test]
fn hidden_complete_returns_every_manifest_enum_in_frozen_order() {
    let all = inventory();
    let mut checked = 0;
    for route in visible(&all) {
        for (type_name, values) in &route.enums {
            let mut args = vec![type_name.as_str()];
            args.extend(route.words());
            assert_success(&complete(&args), values.join("\n").as_bytes(), &route.path);
            checked += 1;
        }
    }
    assert!(checked > 0);
    for (args, expected) in [
        (&["sort", "i", "list"][..], "manual\npriority"),
        (&["sort", "issue", "l"][..], "manual\npriority"),
        (
            &["agentSessionStatus", "i", "agent-session", "list"][..],
            "pending\nactive\ncomplete\nawaitingInput\nerror\nstale",
        ),
        (&["boolean"][..], "true\nfalse"),
        (&["boolean", "completions", "bash"][..], "true\nfalse"),
        (&["sort", "issue"][..], ""),
        (&["string", "issue", "create"][..], ""),
        (&["number", "milestone", "update"][..], ""),
        (&["variable", "api"][..], ""),
        (&["nope", "issue", "mine"][..], ""),
        (&["sort", "--", "issue", "mine"][..], ""),
    ] {
        assert_success(&complete(args), expected.as_bytes(), &args.join(" "));
    }
    let root_workspace = run(&[
        "--workspace",
        "x",
        "completions",
        "complete",
        "sort",
        "issue",
        "mine",
    ]);
    assert_success(&root_workspace, b"manual\npriority", "root workspace");
}

#[test]
fn hidden_complete_rejects_unknown_and_hidden_command_words() {
    for (args, message) in [
        (&["sort", "issue", "nope"][..], "Unknown command \"nope\"."),
        (
            &["sort", "completions", "complete"][..],
            "Unknown command \"complete\".",
        ),
        (&["sort", "iss"][..], "Unknown command \"iss\"."),
        (
            &["sort", "issue", "mine", "extra"][..],
            "Unknown command \"extra\".",
        ),
    ] {
        let output = complete(args);
        assert_eq!(output.status.code(), Some(1), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        assert_eq!(
            text(&output.stderr),
            format!("✗ Auto-completion failed. {message}\n"),
            "{args:?}"
        );
    }
    let missing = complete(&[]);
    assert_eq!(missing.status.code(), Some(2));
    let native = linear_cli::cli::command()
        .try_get_matches_from(["linear", "completions", "complete"])
        .unwrap_err();
    assert_eq!(text(&missing.stderr), native.render().to_string());
}

fn with_credentials(fixture: &str, args: &[&str]) -> (Output, PathBuf) {
    let sandbox = BinarySandbox::new();
    let source = parity_root()
        .join("runner/c086-frozen-cases/fixtures")
        .join(fixture)
        .join("linear/credentials.toml");
    let config = sandbox.root().join("config");
    fs::create_dir_all(config.join("linear")).expect("create credential directory");
    fs::copy(source, config.join("linear/credentials.toml")).expect("copy credential fixture");
    let output = command(&sandbox)
        .args(args)
        .output()
        .expect("completions public binary runs");
    (output, config)
}

fn credentials_error(config: &Path) -> String {
    format!(
        "✗ invalid credentials file {}/linear/credentials.toml: invalid TOML\n  Fix or remove the credentials file, then run `linear auth login`.\n",
        config.display()
    )
}

#[test]
fn startup_warnings_keep_script_stdout_clean_and_fatal_errors_precede_output() {
    let (warning, _) = with_credentials("c086-invalid-default", &["completions", "bash"]);
    assert_eq!(warning.status.code(), Some(0));
    assert!(warning.stdout == BASH);
    assert_eq!(
        text(&warning.stderr),
        "Warning: Default workspace \"ghost\" is not in the workspaces list. Run `linear auth default <workspace>` to set a valid default.\nWarning: Failed to read keyring for workspace \"fake-workspace\": keyring tool unavailable\n"
    );
    for args in [
        &["completions", "bash"][..],
        &["completions", "complete", "sort", "issue", "mine"][..],
    ] {
        let (fatal, config) = with_credentials("c086-malformed", args);
        assert_eq!(fatal.status.code(), Some(1), "{args:?}");
        assert!(fatal.stdout.is_empty(), "{args:?}");
        assert_eq!(text(&fatal.stderr), credentials_error(&config), "{args:?}");
    }
}

#[cfg(unix)]
#[test]
fn closed_stdout_is_quiet_for_scripts_and_strict_for_hidden_complete() {
    use std::io::Read;
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;
    use std::process::Stdio;

    let closed = |args: &[&str]| {
        let sandbox = BinarySandbox::new();
        let (reader, writer) = UnixStream::pair().expect("create closed output pipe");
        drop(reader);
        command(&sandbox)
            .args(args)
            .stdout(Stdio::from(OwnedFd::from(writer)))
            .output()
            .expect("completions public binary runs")
    };
    let script = closed(&["completions", "bash"]);
    assert_eq!(script.status.code(), Some(0));
    assert!(script.stderr.is_empty());
    let shim = closed(&["completions", "complete", "sort", "issue", "mine"]);
    assert_eq!(shim.status.code(), Some(1));
    assert_eq!(text(&shim.stderr), "✗ failed to write stdout\n");
    let empty = closed(&["completions", "complete", "string", "issue", "create"]);
    assert_eq!(empty.status.code(), Some(0));
    assert!(empty.stderr.is_empty());

    // Every script is larger than a 64 KiB pipe buffer, so closing the pipe
    // after a prefix makes a later script write fail.
    for (shell, golden) in [("bash", BASH), ("fish", FISH), ("zsh", ZSH)] {
        assert!(golden.len() > 65_536, "{shell}");
        let sandbox = BinarySandbox::new();
        let (mut reader, writer) = std::io::pipe().expect("create output pipe");
        let child = command(&sandbox)
            .args(["completions", shell])
            .stdout(Stdio::from(writer))
            .stderr(Stdio::piped())
            .spawn()
            .expect("completions public binary starts");
        let mut prefix = [0; 64];
        reader.read_exact(&mut prefix).expect("read script prefix");
        assert_eq!(&prefix[..], &golden[..64], "{shell}");
        drop(reader);
        let output = child.wait_with_output().expect("completions binary exits");
        assert_eq!(output.status.code(), Some(0), "{shell}");
        assert!(
            output.stderr.is_empty(),
            "{shell}: {}",
            text(&output.stderr)
        );
    }
}
