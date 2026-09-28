use std::collections::BTreeMap;
use std::path::PathBuf;

use linear_cli::app::{AppContext, run, write_final_error};
use linear_cli::error::ExitStatus;
use serde_json::Value;

const PARENTS: &[&str] = &[
    "auth",
    "cycle",
    "document",
    "document-comment",
    "initiative",
    "initiative-comment",
    "initiative-update",
    "issue",
    "issue-agent-session",
    "issue-comment",
    "issue-relation",
    "label",
    "milestone",
    "project",
    "project-comment",
    "project-update",
    "team",
    "template",
    "user",
];
const OTHER: &[&str] = &[
    "parent-root-bare",
    "parent-root-short-help",
    "alias-configure-help",
    "alias-docs-bare",
    "alias-docs-help",
    "alias-i-bare",
    "alias-i-help",
    "alias-issue-l-help",
    "alias-issue-list-help",
    "alias-issue-q-help",
    "help-root",
    "help-issue-mine",
    "help-api",
    "color-help-no-color-empty",
    "color-help-no-color-one",
    "version",
    "version-no-color",
    "root-version-long-no-color-empty",
    "root-version-short-default",
    "root-version-short-no-color-empty",
    "root-version-short-no-color-one",
    "c2-api-short-help",
    "c2-label-list-help",
    "c2-mine-help-no-color-one",
    "c2-schema-help",
];

fn fixture(id: &str) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../parity/runner/cases")
        .join(format!("{id}.json"));
    let source = std::fs::read_to_string(path).expect("fixture exists");
    let case: Value = serde_json::from_str(&source).expect("valid fixture JSON");
    let args = case["argv"]
        .as_array()
        .expect("argv array")
        .iter()
        .map(|arg| arg.as_str().expect("argv string").to_owned())
        .collect::<Vec<_>>();
    let env = case["env"]
        .as_object()
        .expect("environment object")
        .iter()
        .filter(|(key, _)| key.as_str() == "NO_COLOR")
        .map(|(key, value)| {
            (
                key.clone(),
                value.as_str().expect("environment string").to_owned(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut context = AppContext {
        startup: super::startup::empty_startup(
            std::env::temp_dir(),
            &env.iter()
                .map(|(key, value)| (key.as_str(), value.as_str()))
                .collect::<Vec<_>>(),
        ),
        cwd: std::env::temp_dir(),
        stdout: &mut stdout,
        stderr: &mut stderr,
        stdin_tty: false,
        stdout_tty: false,
        stderr_tty: false,
        stdout_finalization: None,
    };
    let status = match run(&args, &mut context) {
        Ok(status) => status,
        Err(error) => write_final_error(&mut context, &error).expect("stderr writable"),
    };
    assert_eq!(status, ExitStatus::Success, "{id} status");
    let expected = super::golden::expected_for_case(id, &case);
    assert_eq!(expected["exit"]["code"], 0, "{id} oracle exit");
    let expected_stdout = expected["stdout"]["utf8"]
        .as_str()
        .expect("stdout string")
        .as_bytes();
    if stdout != expected_stdout {
        let first = stdout
            .iter()
            .zip(expected_stdout)
            .position(|(actual, expected)| actual != expected)
            .unwrap_or(stdout.len().min(expected_stdout.len()));
        let start = first.saturating_sub(30);
        let end_actual = (first + 80).min(stdout.len());
        let end_expected = (first + 80).min(expected_stdout.len());
        panic!(
            "{id} stdout differs at byte {first}: actual {:?}, expected {:?}; lengths {} vs {}",
            String::from_utf8_lossy(&stdout[start..end_actual]),
            String::from_utf8_lossy(&expected_stdout[start..end_expected]),
            stdout.len(),
            expected_stdout.len()
        );
    }
    assert_eq!(
        stderr,
        expected["stderr"]["utf8"]
            .as_str()
            .expect("stderr string")
            .as_bytes(),
        "{id} stderr"
    );
}

#[test]
fn frozen_terminal_help_and_version_bytes() {
    for parent in PARENTS {
        for suffix in ["bare", "short-help", "long-help"] {
            fixture(&format!("parent-{parent}-{suffix}"));
        }
    }
    for id in OTHER {
        fixture(id);
    }
}

#[test]
fn generated_completion_help_uses_untyped_option_layout() {
    for (argv, expected) in [
        (
            &["completions", "--help"][..],
            &include_bytes!("expected/completions-parent-help.txt")[..],
        ),
        (
            &["completions", "complete", "--help"][..],
            &include_bytes!("expected/completions-complete-help.txt")[..],
        ),
    ] {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut context = AppContext {
            startup: super::startup::empty_startup(std::env::temp_dir(), &[]),
            cwd: std::env::temp_dir(),
            stdout: &mut stdout,
            stderr: &mut stderr,
            stdin_tty: false,
            stdout_tty: false,
            stderr_tty: false,
            stdout_finalization: None,
        };
        let args = argv.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>();
        let status = run(&args, &mut context).expect("completion help is valid");
        assert_eq!(status, ExitStatus::Success, "{argv:?} status");
        assert_eq!(stdout, expected, "{argv:?} stdout");
        assert!(stderr.is_empty(), "{argv:?} stderr");
    }
}

#[test]
fn long_help_retains_multiline_option_description() {
    use linear_cli::cli::{OptionDefault, OptionMeta, OptionScope, ROUTES, render};

    static OPTION: OptionMeta = OptionMeta {
        scope: OptionScope::Local,
        name: "multiline",
        flags: &["--multiline"],
        description: "First line\n  second_line_marker",
        type_definition: "<item:string>",
        args: &[],
        default: OptionDefault::Null,
        required: false,
        collect: false,
        hidden: false,
        global: false,
    };
    let mut route = *ROUTES.first().expect("root metadata exists");
    route.local_options = std::slice::from_ref(&OPTION);
    let short = render::help(&route, true, false).expect("short help renders");
    let long = render::help(&route, true, true).expect("long help renders");
    assert!(short.contains("First line"));
    assert!(!short.contains("second_line_marker"));
    assert!(long.contains("second_line_marker"));
}
