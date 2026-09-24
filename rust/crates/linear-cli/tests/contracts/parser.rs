use std::collections::BTreeMap;
use std::path::PathBuf;

use linear_cli::app::{AppContext, run, write_final_error};
use serde_json::Value;

const CASES: &[&str] = &[
    "c2-global-before-issue-bare",
    "c2-global-before-issue-help",
    "c2-issue-workspace-bare",
    "c2-issue-workspace-missing",
    "c2-issue-short-version",
    "c2-mine-hidden-option-typo",
    "c2-root-literal-double-dash",
    "c2-root-negated-workspace-help",
    "c2-root-short-bundle-help-version",
    "c2-root-workspace-equals-help",
    "grammar-help-then-version",
    "grammar-issue-unknown-command",
    "grammar-issue-unknown-option",
    "grammar-issue-version",
    "grammar-issue-workspace-help",
    "grammar-root-workspace-help",
    "grammar-root-workspace-missing",
    "grammar-version-then-help",
    "color-unknown-command-no-color-empty",
    "color-unknown-command-no-color-one",
    "parser-unknown-command",
    "parser-invalid-variable",
    "parser-unknown-option",
];

fn assert_bytes(id: &str, surface: &str, actual: &[u8], expected: &[u8]) {
    if actual == expected {
        return;
    }
    let first = actual
        .iter()
        .zip(expected)
        .position(|(a, b)| a != b)
        .unwrap_or(actual.len().min(expected.len()));
    let start = first.saturating_sub(25);
    let actual_end = (first + 90).min(actual.len());
    let expected_end = (first + 90).min(expected.len());
    panic!(
        "{id} {surface} differs at byte {first}: actual {:?}, expected {:?}; lengths {} vs {}",
        String::from_utf8_lossy(&actual[start..actual_end]),
        String::from_utf8_lossy(&expected[start..expected_end]),
        actual.len(),
        expected.len()
    );
}

#[test]
fn frozen_parser_contracts() {
    for id in CASES {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../parity/runner/cases")
            .join(format!("{id}.json"));
        let source = std::fs::read_to_string(path).expect("fixture exists");
        let case: Value = serde_json::from_str(&source).expect("valid fixture JSON");
        let args = case["argv"]
            .as_array()
            .expect("argv array")
            .iter()
            .map(|value| value.as_str().expect("argv string").to_owned())
            .collect::<Vec<_>>();
        let env = case["env"]
            .as_object()
            .expect("env object")
            .iter()
            .filter(|(key, _)| key.as_str() == "NO_COLOR")
            .map(|(key, value)| (key.clone(), value.as_str().expect("env string").to_owned()))
            .collect::<BTreeMap<_, _>>();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut context = AppContext {
            env,
            cwd: std::env::temp_dir(),
            stdout: &mut stdout,
            stderr: &mut stderr,
            stdout_tty: false,
            stderr_tty: false,
            startup_diagnostics: Vec::new(),
            stdout_finalization: None,
        };
        let status = match run(&args, &mut context) {
            Ok(status) => status,
            Err(error) => write_final_error(&mut context, &error).expect("stderr writable"),
        };
        let expected = super::golden::expected_for_case(id, &case);
        assert_eq!(
            u64::from(status.code()),
            expected["exit"]["code"].as_u64().expect("exit code"),
            "{id} exit"
        );
        assert_bytes(
            id,
            "stdout",
            &stdout,
            expected["stdout"]["utf8"]
                .as_str()
                .expect("stdout text")
                .as_bytes(),
        );
        assert_bytes(
            id,
            "stderr",
            &stderr,
            expected["stderr"]["utf8"]
                .as_str()
                .expect("stderr text")
                .as_bytes(),
        );
    }
}

fn run_args(args: &[&str]) -> (u8, String, String) {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut context = AppContext {
        env: BTreeMap::new(),
        cwd: std::env::temp_dir(),
        stdout: &mut stdout,
        stderr: &mut stderr,
        stdout_tty: false,
        stderr_tty: false,
        startup_diagnostics: Vec::new(),
        stdout_finalization: None,
    };
    let args = args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>();
    let status = match run(&args, &mut context) {
        Ok(status) => status,
        Err(error) => write_final_error(&mut context, &error).expect("writable stderr"),
    };
    (
        status.code(),
        String::from_utf8(stdout).expect("UTF-8 stdout"),
        String::from_utf8(stderr).expect("UTF-8 stderr"),
    )
}

#[test]
fn source_derived_standalone_action_order() {
    let version = (0, "3.0.0-alpha.1\n".to_owned(), String::new());
    assert_eq!(run_args(&["-h", "-V"]), version);
    assert_eq!(run_args(&["--help", "-V"]), version);
    let (status, stdout, stderr) = run_args(&["-h", "--version"]);
    assert_eq!(status, 0);
    assert_eq!(
        stdout,
        linear_cli::cli::render::help(linear_cli::cli::root().expect("root route"), true, false)
            .expect("root help")
    );
    assert!(stderr.is_empty());
}

#[test]
fn source_derived_required_values_consume_dash_tokens() {
    let expected = (
        0,
        "Use --help to see available commands\n".to_owned(),
        String::new(),
    );
    assert_eq!(run_args(&["--workspace", "--help"]), expected);
    assert_eq!(run_args(&["--workspace", "--"]), expected);
}

#[test]
fn direct_binary_parser_contracts() {
    use std::process::Command;
    for id in [
        "c2-global-before-issue-bare",
        "c2-global-before-issue-help",
        "grammar-issue-unknown-command",
        "grammar-issue-unknown-option",
        "c2-issue-workspace-missing",
        "color-unknown-command-no-color-one",
        "color-unknown-command-no-color-empty",
        "c2-root-literal-double-dash",
        "parser-invalid-variable",
    ] {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../parity/runner/cases")
            .join(format!("{id}.json"));
        let case: Value = serde_json::from_slice(&std::fs::read(path).expect("fixture exists"))
            .expect("valid fixture JSON");
        let args = case["argv"]
            .as_array()
            .expect("argv array")
            .iter()
            .map(|value| value.as_str().expect("argv string"));
        let mut command = Command::new(env!("CARGO_BIN_EXE_linear"));
        command
            .args(args)
            .env_clear()
            .env("HOME", std::env::temp_dir())
            .env("XDG_CONFIG_HOME", std::env::temp_dir())
            .env("APPDATA", std::env::temp_dir())
            .env("PATH", "/usr/bin:/bin")
            .env("TZ", "UTC")
            .env("LANG", "C.UTF-8")
            .env("LINEAR_IGNORE_ENV_FILE", "1")
            .env("LINEAR_GRAPHQL_ENDPOINT", "http://127.0.0.1:1/graphql");
        if let Some(no_color) = case["env"].get("NO_COLOR") {
            command.env("NO_COLOR", no_color.as_str().expect("NO_COLOR string"));
        }
        let output = command.output().expect("binary runs");
        let expected = super::golden::expected_for_case(id, &case);
        assert_eq!(
            output.status.code(),
            expected["exit"]["code"]
                .as_i64()
                .and_then(|code| i32::try_from(code).ok()),
            "{id} exit"
        );
        assert_bytes(
            id,
            "binary stdout",
            &output.stdout,
            expected["stdout"]["utf8"]
                .as_str()
                .expect("stdout text")
                .as_bytes(),
        );
        assert_bytes(
            id,
            "binary stderr",
            &output.stderr,
            expected["stderr"]["utf8"]
                .as_str()
                .expect("stderr text")
                .as_bytes(),
        );
    }
    let output = Command::new(env!("CARGO_BIN_EXE_linear"))
        .args(["issue", "view", "ABC-1"])
        .env_clear()
        .env("HOME", std::env::temp_dir())
        .env("XDG_CONFIG_HOME", std::env::temp_dir())
        .env("APPDATA", std::env::temp_dir())
        .env("LINEAR_IGNORE_ENV_FILE", "1")
        .env("LINEAR_GRAPHQL_ENDPOINT", "http://127.0.0.1:1/graphql")
        .output()
        .expect("binary runs");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"\xe2\x9c\x97 linear issue view is registered, but this action is not implemented yet\n"
    );
}

#[test]
fn source_derived_inherited_option_wins_equal_distance() {
    let (status, _stdout, stderr) = run_args(&["issue", "--wwrks"]);
    assert_eq!(status, 2);
    assert!(stderr.contains("Did you mean option \"--workspace\"?"));
}

#[test]
fn source_derived_action_keeps_positionals_options_and_literal() {
    use linear_cli::cli::parser::{ParseOutcome, parse};
    let args = ["issue", "view", "ABC-1", "--workspace", "ws", "--", "raw"].map(str::to_owned);
    let outcome = parse(&args).expect("registered route parses");
    match outcome {
        ParseOutcome::Action {
            route,
            positionals,
            literal,
            options,
        } => {
            assert_eq!(route.path, "linear issue view");
            assert_eq!(positionals, ["ABC-1"]);
            assert_eq!(literal, ["raw"]);
            assert_eq!(options.len(), 1);
            assert_eq!(options.first().map(|option| option.name), Some("workspace"));
            assert_eq!(
                options.first().map(|option| option.values.as_slice()),
                Some(["ws".to_owned()].as_slice())
            );
        }
        ParseOutcome::Help { .. } | ParseOutcome::Version { .. } => {
            panic!("expected a registered action");
        }
    }
}

fn assert_source_usage(args: &[&str], route_path: &str, message: &str) {
    let (status, stdout, stderr) = run_args(args);
    assert_eq!(status, 2, "{args:?} exit");
    let route = linear_cli::cli::ROUTES
        .iter()
        .find(|route| route.path == route_path)
        .expect("route exists");
    assert_eq!(
        stdout,
        linear_cli::cli::render::help(route, true, false).expect("route help"),
        "{args:?} stdout"
    );
    assert_eq!(
        stderr,
        format!("\x1b[31m  \x1b[1merror\x1b[22m: {message}\n\x1b[39m\n"),
        "{args:?} stderr"
    );
}

#[test]
fn source_derived_standalone_persists_across_parse_passes() {
    let message = "Option \"--help\" cannot be combined with other options.";
    assert_source_usage(
        &["issue", "view", "-h", "--json"],
        "linear issue view",
        message,
    );
    assert_source_usage(
        &["--help", "issue", "view", "--json"],
        "linear issue view",
        message,
    );
    assert_source_usage(
        &["issue", "view", "--json", "-h"],
        "linear issue view",
        message,
    );
    assert_source_usage(
        &["issue", "-h", "--workspace", "ws"],
        "linear issue",
        message,
    );
    assert_source_usage(
        &["issue", "mine", "-h", "--sort", "priority"],
        "linear issue mine",
        message,
    );
    assert_source_usage(
        &["--help", "issue", "mine", "--sort", "priority"],
        "linear issue mine",
        message,
    );
    assert_eq!(run_args(&["--workspace", "ws", "issue", "--help"]).0, 0);
    assert_eq!(
        run_args(&["-h", "-V"]),
        (0, "3.0.0-alpha.1\n".to_owned(), String::new())
    );
}

#[test]
fn source_derived_empty_next_value_and_zero_argument_inline_errors() {
    assert_source_usage(
        &["--workspace", "", "issue"],
        "linear",
        "Missing value for option \"--workspace\".",
    );
    assert_source_usage(
        &["issue", "--workspace", ""],
        "linear issue",
        "Missing value for option \"--workspace\".",
    );
    assert_source_usage(
        &["issue", "archive", "--bulk", ""],
        "linear issue archive",
        "Missing value for option \"--bulk\".",
    );
    assert_source_usage(
        &["issue", "--help=1"],
        "linear issue",
        "Option \"--help\" doesn't take a value, but got \"1\".",
    );
    assert_eq!(run_args(&["issue", "--help="]).0, 0);
}

#[test]
fn source_derived_canonical_missing_value_names() {
    for args in [["issue", "mine", "-s"], ["issue", "mine", "--state"]] {
        assert_source_usage(
            &args,
            "linear issue mine",
            "Missing value for option \"--state\".",
        );
    }
    for args in [["issue", "create", "-t"], ["issue", "create", "--title"]] {
        assert_source_usage(
            &args,
            "linear issue create",
            "Missing value for option \"--title\".",
        );
    }
}

#[test]
fn source_derived_short_equals_and_variable_order() {
    let combination = "Option \"--help\" cannot be combined with other options.";
    for args in [
        vec!["issue", "mine", "-s=priority", "--help"],
        vec!["issue", "mine", "--state=priority", "--help"],
        vec!["issue", "mine", "-s", "priority", "--help"],
        vec!["--help", "issue", "mine", "-s=priority"],
        vec!["--help", "issue", "mine", "--state=priority"],
    ] {
        assert_source_usage(&args, "linear issue mine", combination);
    }
    assert_source_usage(
        &["issue", "view", "-j=1"],
        "linear issue view",
        "Option \"--json\" doesn't take a value, but got \"1\".",
    );
    assert_source_usage(
        &["issue", "view", "--json=1"],
        "linear issue view",
        "Option \"--json\" doesn't take a value, but got \"1\".",
    );
    assert_source_usage(
        &["api", "--variable", "badformat", "--help"],
        "linear api",
        "Invalid variable format: badformat. Variables must be in key=value format, e.g. --variable teamId=abc",
    );
    assert_source_usage(
        &["api", "--variable", "key=a=b", "--help"],
        "linear api",
        combination,
    );
    for value in ["key=", "=value", "key=a=b"] {
        assert_eq!(
            run_args(&["api", "--variable", value]).0,
            1,
            "{value} passes Variable syntax and reaches visible unimplemented action"
        );
    }
}

#[test]
fn source_derived_short_equals_preserves_typed_option_value() {
    use linear_cli::cli::parser::{ParseOutcome, parse};
    for args in [
        vec!["issue", "mine", "-s=priority"],
        vec!["issue", "mine", "--state=priority"],
        vec!["issue", "mine", "-s", "priority"],
    ] {
        let args = args.into_iter().map(str::to_owned).collect::<Vec<_>>();
        match parse(&args).expect("option parses") {
            ParseOutcome::Action { route, options, .. } => {
                assert_eq!(route.path, "linear issue mine");
                assert!(
                    options
                        .iter()
                        .any(|option| option.name == "state" && option.values == ["priority"])
                );
            }
            ParseOutcome::Help { .. } | ParseOutcome::Version { .. } => panic!("expected action"),
        }
    }
}

#[test]
fn direct_binary_source_derived_parser_matrix() {
    use std::process::Command;
    let variable = "Invalid variable format: badformat. Variables must be in key=value format, e.g. --variable teamId=abc";
    let combination = "Option \"--help\" cannot be combined with other options.";
    let cases: &[(&[&str], &str, &str)] = &[
        (
            &["issue", "mine", "-s"],
            "linear issue mine",
            "Missing value for option \"--state\".",
        ),
        (
            &["issue", "mine", "--state"],
            "linear issue mine",
            "Missing value for option \"--state\".",
        ),
        (
            &["issue", "create", "-t"],
            "linear issue create",
            "Missing value for option \"--title\".",
        ),
        (
            &["issue", "create", "--title"],
            "linear issue create",
            "Missing value for option \"--title\".",
        ),
        (
            &["issue", "mine", "-s=priority", "--help"],
            "linear issue mine",
            combination,
        ),
        (
            &["issue", "mine", "--state=priority", "--help"],
            "linear issue mine",
            combination,
        ),
        (
            &["issue", "mine", "-s", "priority", "--help"],
            "linear issue mine",
            combination,
        ),
        (
            &["--help", "issue", "mine", "-s=priority"],
            "linear issue mine",
            combination,
        ),
        (
            &["--help", "issue", "mine", "--state=priority"],
            "linear issue mine",
            combination,
        ),
        (
            &["issue", "view", "-j=1"],
            "linear issue view",
            "Option \"--json\" doesn't take a value, but got \"1\".",
        ),
        (
            &["issue", "view", "--json=1"],
            "linear issue view",
            "Option \"--json\" doesn't take a value, but got \"1\".",
        ),
        (
            &["api", "--variable", "badformat", "--help"],
            "linear api",
            variable,
        ),
        (
            &["api", "--variable", "key=a=b", "--help"],
            "linear api",
            combination,
        ),
    ];
    for (args, route_path, message) in cases {
        let output = Command::new(env!("CARGO_BIN_EXE_linear"))
            .args(*args)
            .env_clear()
            .env("HOME", std::env::temp_dir())
            .env("XDG_CONFIG_HOME", std::env::temp_dir())
            .env("APPDATA", std::env::temp_dir())
            .env("PATH", "/usr/bin:/bin")
            .env("TZ", "UTC")
            .env("LANG", "C.UTF-8")
            .env("LINEAR_IGNORE_ENV_FILE", "1")
            .env("LINEAR_GRAPHQL_ENDPOINT", "http://127.0.0.1:1/graphql")
            .output()
            .expect("binary runs");
        let route = linear_cli::cli::ROUTES
            .iter()
            .find(|route| route.path == *route_path)
            .expect("route exists");
        assert_eq!(output.status.code(), Some(2), "{args:?} exit");
        assert_bytes(
            &format!("{args:?}"),
            "binary stdout",
            &output.stdout,
            linear_cli::cli::render::help(route, true, false)
                .expect("route help")
                .as_bytes(),
        );
        assert_bytes(
            &format!("{args:?}"),
            "binary stderr",
            &output.stderr,
            format!("\x1b[31m  \x1b[1merror\x1b[22m: {message}\n\x1b[39m\n").as_bytes(),
        );
    }
}

#[test]
fn observed_registered_enum_usage_from_binary() {
    use std::process::Command;

    let mine_fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../parity/runner/cases/c2-mine-help-no-color-one.json");
    let mine_case: Value = serde_json::from_slice(&std::fs::read(mine_fixture).expect("fixture"))
        .expect("fixture JSON");
    let mine_expected = super::golden::expected_for_case("c2-mine-help-no-color-one", &mine_case);
    let mine_help = mine_expected["stdout"]["utf8"]
        .as_str()
        .expect("reviewed Rust mine help");
    let cases: &[(&[&str], &str, &str)] = &[
        (
            &["issue", "mine", "--sort", "nonsense"],
            "linear issue mine",
            "Option \"--sort\" must be of type \"sort\", but got \"nonsense\". Expected values: \"manual\", \"priority\"",
        ),
        (
            &["issue", "mine", "--sort=nonsense"],
            "linear issue mine",
            "Option \"--sort\" must be of type \"sort\", but got \"nonsense\". Expected values: \"manual\", \"priority\"",
        ),
        (
            &["issue", "mine", "--sort", "NONSENSE"],
            "linear issue mine",
            "Option \"--sort\" must be of type \"sort\", but got \"NONSENSE\". Expected values: \"manual\", \"priority\"",
        ),
        (
            &["issue", "mine", "--sort", "MANUAL"],
            "linear issue mine",
            "Option \"--sort\" must be of type \"sort\", but got \"MANUAL\". Expected values: \"manual\", \"priority\"",
        ),
        (
            &["issue", "mine", "--sort=nonsense", "--help"],
            "linear issue mine",
            "Option \"--sort\" must be of type \"sort\", but got \"nonsense\". Expected values: \"manual\", \"priority\"",
        ),
        (
            &["issue", "mine", "-h", "--sort", "nonsense"],
            "linear issue mine",
            "Option \"--sort\" must be of type \"sort\", but got \"nonsense\". Expected values: \"manual\", \"priority\"",
        ),
        (
            &["--help", "issue", "mine", "--sort", "nonsense"],
            "linear issue mine",
            "Option \"--sort\" must be of type \"sort\", but got \"nonsense\". Expected values: \"manual\", \"priority\"",
        ),
        (
            &["issue", "mine", "--sort", "--help"],
            "linear issue mine",
            "Option \"--sort\" must be of type \"sort\", but got \"--help\". Expected values: \"manual\", \"priority\"",
        ),
        (
            &["issue", "l", "--sort", "nonsense"],
            "linear issue mine",
            "Option \"--sort\" must be of type \"sort\", but got \"nonsense\". Expected values: \"manual\", \"priority\"",
        ),
        (
            &["issue", "q", "--sort", "nonsense"],
            "linear issue query",
            "Option \"--sort\" must be of type \"sort\", but got \"nonsense\". Expected values: \"manual\", \"priority\"",
        ),
        (
            &["issue", "query", "--sort", "nonsense"],
            "linear issue query",
            "Option \"--sort\" must be of type \"sort\", but got \"nonsense\". Expected values: \"manual\", \"priority\"",
        ),
        (
            &["issue", "agent-session", "list", "--status", "nonsense"],
            "linear issue agent-session list",
            "Option \"--status\" must be of type \"agentSessionStatus\", but got \"nonsense\". Expected values: \"pending\", \"active\", \"complete\", \"awaitingInput\", \"error\", \"stale\"",
        ),
        (
            &["template", "list", "--type", "nonsense"],
            "linear template list",
            "Option \"--type\" must be of type \"template-type\", but got \"nonsense\". Expected values: \"issue\", \"project\", \"document\"",
        ),
        (
            &["issue", "mine", "--sort="],
            "linear issue mine",
            "Missing value for option \"--sort\".",
        ),
    ];
    for (args, route_path, message) in cases {
        let output = Command::new(env!("CARGO_BIN_EXE_linear"))
            .args(*args)
            .env_clear()
            .env("HOME", std::env::temp_dir())
            .env("XDG_CONFIG_HOME", std::env::temp_dir())
            .env("APPDATA", std::env::temp_dir())
            .env("PATH", "/usr/bin:/bin")
            .env("TZ", "UTC")
            .env("LANG", "C.UTF-8")
            .env("LINEAR_IGNORE_ENV_FILE", "1")
            .env("LINEAR_GRAPHQL_ENDPOINT", "http://127.0.0.1:1/graphql")
            .env("NO_COLOR", "1")
            .output()
            .expect("binary runs");
        let route = linear_cli::cli::ROUTES
            .iter()
            .find(|route| route.path == *route_path)
            .expect("route exists");
        let help = if *route_path == "linear issue mine" {
            mine_help.to_owned()
        } else {
            // These routes use Rust-rendered help; only mine has a frozen fixture.
            linear_cli::cli::render::help(route, false, false).expect("route help")
        };
        assert_eq!(output.status.code(), Some(2), "{args:?} exit");
        assert_bytes(
            &format!("{args:?}"),
            "binary stdout",
            &output.stdout,
            help.as_bytes(),
        );
        assert_bytes(
            &format!("{args:?}"),
            "binary stderr",
            &output.stderr,
            format!("  error: {message}\n\n").as_bytes(),
        );
    }
}

#[test]
fn registered_enum_values_and_unrelated_strings_reach_actions() {
    use linear_cli::cli::parser::{ParseOutcome, parse};
    for (args, expected_name, expected_value) in [
        (vec!["issue", "mine", "--sort", "manual"], "sort", "manual"),
        (
            vec!["issue", "query", "--sort=priority"],
            "sort",
            "priority",
        ),
        (
            vec!["issue", "agent-session", "list", "--status", "active"],
            "status",
            "active",
        ),
        (
            vec!["template", "list", "--type=project"],
            "type",
            "project",
        ),
        (
            vec!["issue", "mine", "--team", "NONSENSE"],
            "team",
            "NONSENSE",
        ),
    ] {
        let args = args.into_iter().map(str::to_owned).collect::<Vec<_>>();
        match parse(&args).expect("registered option parses") {
            ParseOutcome::Action { options, .. } => assert!(options.iter().any(|option| {
                option.name == expected_name && option.values == [expected_value]
            })),
            ParseOutcome::Help { .. } | ParseOutcome::Version { .. } => panic!("expected action"),
        }
    }
}

#[test]
fn observed_registered_positional_arity_from_binary() {
    use std::process::Command;

    let cases: &[(&[&str], &str, &str)] = &[
        (
            &["issue", "attach"],
            "linear issue attach",
            "Missing argument(s): issueId, filepath",
        ),
        (
            &["issue", "attach", "ABC-1"],
            "linear issue attach",
            "Missing argument: filepath",
        ),
        (
            &["issue", "attach", "ABC-1", "/tmp/x", "extra"],
            "linear issue attach",
            "Too many arguments: extra",
        ),
        (
            &["issue", "attach", "ABC-1", "--", "/tmp/x"],
            "linear issue attach",
            "Missing argument: filepath",
        ),
        (
            &["issue", "attach", "ABC-1", "--help"],
            "linear issue attach",
            "Missing argument: filepath",
        ),
        (
            &["issue", "attach", "--help", "ABC-1"],
            "linear issue attach",
            "Missing argument: filepath",
        ),
        (
            &["issue", "attach", "ABC-1", "/tmp/x", "extra", "--help"],
            "linear issue attach",
            "Too many arguments: extra",
        ),
        (
            &["issue", "attach", "ABC-1", "/tmp/x", "--help", "extra"],
            "linear issue attach",
            "Too many arguments: extra",
        ),
        (
            &["issue", "relation", "add"],
            "linear issue relation add",
            "Missing argument(s): issueId, relationType, relatedIssueId",
        ),
        (
            &["issue", "relation", "add", "ABC-1"],
            "linear issue relation add",
            "Missing argument: relationType",
        ),
        (
            &["issue", "relation", "add", "ABC-1", "blocks"],
            "linear issue relation add",
            "Missing argument: relatedIssueId",
        ),
        (
            &[
                "issue", "relation", "add", "ABC-1", "blocks", "ABC-2", "extra",
            ],
            "linear issue relation add",
            "Too many arguments: extra",
        ),
        (
            &["issue", "view", "ABC-1", "extra", "more"],
            "linear issue view",
            "Too many arguments: extra more",
        ),
        (
            &["issue", "link", "A", "B", "C"],
            "linear issue link",
            "Too many arguments: C",
        ),
        (
            &["completions", "complete"],
            "linear completions complete",
            "Missing argument(s): action",
        ),
        (
            &["auth", "list", "extra"],
            "linear auth list",
            "No arguments allowed for command \"linear auth list\".",
        ),
        (
            &["issue", "archive", "--bulk", "A", "", "B"],
            "linear issue archive",
            "Too many arguments: B",
        ),
    ];
    for (args, route_path, message) in cases {
        let output = Command::new(env!("CARGO_BIN_EXE_linear"))
            .args(*args)
            .env_clear()
            .env("HOME", std::env::temp_dir())
            .env("XDG_CONFIG_HOME", std::env::temp_dir())
            .env("APPDATA", std::env::temp_dir())
            .env("PATH", "/usr/bin:/bin")
            .env("TZ", "UTC")
            .env("LANG", "C.UTF-8")
            .env("LINEAR_IGNORE_ENV_FILE", "1")
            .env("LINEAR_GRAPHQL_ENDPOINT", "http://127.0.0.1:1/graphql")
            .env("NO_COLOR", "1")
            .output()
            .expect("binary runs");
        let route = linear_cli::cli::ROUTES
            .iter()
            .find(|route| route.path == *route_path)
            .expect("route exists");
        assert_eq!(output.status.code(), Some(2), "{args:?} exit");
        assert_bytes(
            &format!("{args:?}"),
            "binary stdout (Rust-rendered short no-color help)",
            &output.stdout,
            linear_cli::cli::render::help(route, false, false)
                .expect("Rust route help")
                .as_bytes(),
        );
        assert_bytes(
            &format!("{args:?}"),
            "binary stderr (direct frozen observation)",
            &output.stderr,
            format!("  error: {message}\n\n").as_bytes(),
        );
    }
}

#[test]
fn registered_positional_help_and_parent_routes() {
    use linear_cli::cli::parser::{ParseOutcome, parse};
    for (args, route_path) in [
        (vec!["issue", "attach", "--help"], "linear issue attach"),
        (
            vec!["issue", "relation", "add", "--help"],
            "linear issue relation add",
        ),
        (
            vec![
                "completions",
                "complete",
                "fish",
                "one",
                "two",
                "three",
                "--help",
            ],
            "linear completions complete",
        ),
        (vec!["issue", "--help"], "linear issue"),
    ] {
        let args = args.into_iter().map(str::to_owned).collect::<Vec<_>>();
        match parse(&args).expect("standalone help parses") {
            ParseOutcome::Help { route, .. } => assert_eq!(route.path, route_path),
            ParseOutcome::Action { .. } | ParseOutcome::Version { .. } => panic!("expected help"),
        }
    }
    let (status, stdout, stderr) = run_args(&["issue", "--help"]);
    let issue = linear_cli::cli::ROUTES
        .iter()
        .find(|route| route.path == "linear issue")
        .expect("parent route");
    assert_eq!(status, 0);
    assert_eq!(
        stdout,
        linear_cli::cli::render::help(issue, true, false).expect("parent help")
    );
    assert!(stderr.is_empty());
}

#[test]
fn registered_positional_accepted_arity_reaches_action() {
    use linear_cli::cli::parser::{ParseOutcome, parse};
    for (args, route_path, values) in [
        (vec!["issue", "view", "A"], "linear issue view", vec!["A"]),
        (
            vec!["issue", "attach", "A", "B"],
            "linear issue attach",
            vec!["A", "B"],
        ),
        (
            vec!["issue", "relation", "add", "A", "blocks", "B"],
            "linear issue relation add",
            vec!["A", "blocks", "B"],
        ),
        (vec!["issue", "link", "A"], "linear issue link", vec!["A"]),
        (
            vec!["issue", "link", "A", "B"],
            "linear issue link",
            vec!["A", "B"],
        ),
        (
            vec!["completions", "complete", "fish"],
            "linear completions complete",
            vec!["fish"],
        ),
        (
            vec!["completions", "complete", "fish", "one", "two"],
            "linear completions complete",
            vec!["fish", "one", "two"],
        ),
    ] {
        let args = args.into_iter().map(str::to_owned).collect::<Vec<_>>();
        match parse(&args).expect("registered arity parses") {
            ParseOutcome::Action {
                route, positionals, ..
            } => {
                assert_eq!(route.path, route_path);
                assert_eq!(positionals, values);
            }
            ParseOutcome::Help { .. } | ParseOutcome::Version { .. } => panic!("expected action"),
        }
    }
}

#[test]
fn registered_variadic_option_consumes_remaining_values() {
    use linear_cli::cli::parser::{ParseOutcome, parse};

    for (args, expected) in [
        (
            vec!["issue", "archive", "--bulk", "A", "B", "C"],
            vec!["A", "B", "C"],
        ),
        (
            vec!["issue", "archive", "--bulk", "A", "B", "--help"],
            vec!["A", "B", "--help"],
        ),
        (
            vec![
                "issue",
                "archive",
                "--bulk",
                "A",
                "B",
                "--definitely-not-an-option",
            ],
            vec!["A", "B", "--definitely-not-an-option"],
        ),
    ] {
        let args = args.into_iter().map(str::to_owned).collect::<Vec<_>>();
        match parse(&args).expect("variadic option parses") {
            ParseOutcome::Action {
                route,
                positionals,
                options,
                ..
            } => {
                assert_eq!(route.path, "linear issue archive");
                assert!(positionals.is_empty());
                assert_eq!(options.len(), 1);
                assert_eq!(options[0].name, "bulk");
                assert_eq!(options[0].values, expected);
            }
            ParseOutcome::Help { .. } | ParseOutcome::Version { .. } => panic!("expected action"),
        }
    }

    let args = ["issue", "archive", "--bulk", "A", ""].map(str::to_owned);
    match parse(&args).expect("empty token stops variadic option") {
        ParseOutcome::Action {
            positionals,
            options,
            ..
        } => {
            assert_eq!(positionals, [""]);
            assert_eq!(options[0].values, ["A"]);
        }
        ParseOutcome::Help { .. } | ParseOutcome::Version { .. } => panic!("expected action"),
    }
}

#[test]
fn observed_variadic_option_reaches_action_from_binary() {
    use std::process::Command;

    let unimplemented = b"\xe2\x9c\x97 linear issue archive is registered, but this action is not implemented yet\n";
    for args in [
        vec!["issue", "archive", "--bulk", "A", "B", "C"],
        vec!["issue", "archive", "--bulk", "A", "B", "--help"],
        vec![
            "issue",
            "archive",
            "--bulk",
            "A",
            "B",
            "--definitely-not-an-option",
        ],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_linear"))
            .args(&args)
            .env_clear()
            .env("HOME", std::env::temp_dir())
            .env("XDG_CONFIG_HOME", std::env::temp_dir())
            .env("APPDATA", std::env::temp_dir())
            .env("PATH", "/usr/bin:/bin")
            .env("TZ", "UTC")
            .env("LANG", "C.UTF-8")
            .env("LINEAR_IGNORE_ENV_FILE", "1")
            .env("LINEAR_GRAPHQL_ENDPOINT", "http://127.0.0.1:1/graphql")
            .env("NO_COLOR", "1")
            .output()
            .expect("binary runs");
        assert_eq!(output.status.code(), Some(1), "{args:?} exit");
        assert!(output.stdout.is_empty(), "{args:?} stdout");
        assert_bytes(
            &format!("{args:?}"),
            "binary stderr (Rust action boundary)",
            &output.stderr,
            unimplemented,
        );
    }
}
