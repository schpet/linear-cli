//! `markdown`, `completions` and `config` (configuration generation).
use crate::support::{Cli, MockLinear};

#[test]
fn markdown_prints_the_reference() {
    let run = Cli::new().run(&["markdown"]);
    run.success()
        .stdout_has("+++ [")
        .stdout_has("linear team members");
}

#[test]
fn completions_print_a_registration_script_for_every_shell() {
    let cli = Cli::new();
    for shell in ["bash", "zsh", "fish", "elvish", "powershell"] {
        let run = cli.run(&["completions", shell]);
        run.success()
            .stdout_has("COMPLETE")
            .stdout_has(shell)
            .stdout_has(env!("CARGO_BIN_EXE_linear"));
    }
}

#[test]
fn completions_register_under_a_custom_name() {
    let run = Cli::new().run(&["completions", "fish", "--name", "lin"]);
    run.success().stdout_has("--command lin ");
}

#[test]
fn completions_reject_unknown_shells_and_unsafe_names() {
    Cli::new().run(&["completions", "tcsh"]).usage_error();
    Cli::new()
        .run(&["completions", "bash", "--name", "x;rm"])
        .usage_error();
    Cli::new().run(&["completions"]).usage_error();
}

/// Candidates fish would get for the command line `words`, the last being
/// the word under the cursor.
fn fish_candidates(cli: &Cli, words: &[&str]) -> Vec<String> {
    let mut args = vec!["--", "linear"];
    args.extend(words);
    let run = cli.run(&args);
    run.success();
    run.stdout
        .lines()
        .map(|line| line.split('\t').next().unwrap_or_default().to_owned())
        .collect()
}

#[test]
fn completion_requests_reach_every_command_depth() {
    let cli = Cli::new().env("COMPLETE", "fish");
    let flags = fish_candidates(&cli, &["issue", "comment", "add", "--b"]);
    assert_eq!(flags, ["--body", "--body-file"]);
    let statuses = fish_candidates(&cli, &["issue", "agent-session", "list", "--status", ""]);
    assert!(
        statuses.contains(&"awaitingInput".to_owned()),
        "{statuses:?}"
    );
    let commands = fish_candidates(&cli, &["iss"]);
    assert_eq!(commands, ["issue"]);
}

#[test]
fn completion_requests_skip_hidden_flags_and_configuration() {
    let cli = Cli::new()
        .env("COMPLETE", "fish")
        .file("cwd/.linear.toml", "issue_sort = \"sideways\"\n");
    let flags = fish_candidates(&cli, &["issue", "mine", "--a"]);
    assert!(flags.contains(&"--all-states".to_owned()), "{flags:?}");
    assert!(!flags.contains(&"--assignee".to_owned()), "{flags:?}");
}

#[test]
fn config_without_credentials_fails_before_any_request() {
    let api = MockLinear::start();
    Cli::new()
        .endpoint(&api)
        .run(&["config"])
        .failure()
        .stderr_has("linear auth login");
    assert!(api.requests().is_empty());
}
