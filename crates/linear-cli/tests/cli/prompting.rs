//! When commands ask questions: `--no-input`, `--interactive`, and missing
//! values without a terminal. The tests run without a terminal.
use crate::support::{Cli, MockLinear};

#[test]
fn no_input_refuses_confirmations_and_names_the_flag_that_skips_them() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api);
    for args in [
        &["--no-input", "label", "delete", "Bug"][..],
        &["label", "delete", "Bug", "--no-input"],
        &["label", "delete", "Bug", "--no-interactive"],
    ] {
        cli.run(args)
            .failure()
            .stderr_has("--no-input is set")
            .stderr_has("--yes");
    }
    assert!(api.requests().is_empty());
}

#[test]
fn interactive_conflicts_with_no_input_wherever_it_is_given() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api);
    for args in [
        &["--no-input", "label", "create", "-i", "-n", "Bug"][..],
        &["label", "create", "-n", "Bug", "-i", "--no-input"],
        &["--no-input", "team", "create", "-i"],
        &["--no-input", "project-update", "create", "P", "-i"],
        &["--no-input", "issue", "create", "-i"],
    ] {
        cli.run(args)
            .usage_error()
            .stderr_has("'--interactive' cannot be used with '--no-input'");
    }
    assert!(api.requests().is_empty());
}

#[test]
fn interactive_without_a_terminal_fails_before_any_request() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api);
    for args in [
        &["label", "create", "-n", "Bug", "-i"][..],
        &["team", "create", "-n", "Core", "-i"],
        &["initiative", "create", "-n", "Q3", "-i"],
        &["document", "create", "-t", "Notes", "--team", "ENG", "-i"],
    ] {
        cli.run(args)
            .failure()
            .stderr_has("--interactive needs a terminal");
    }
    assert!(api.requests().is_empty());
}

#[test]
fn missing_required_values_without_a_terminal_name_the_flag() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api).env("LINEAR_TEAM_ID", "ENG");
    cli.run(&["label", "create"]).failure().stderr_has("--name");
    cli.run(&["team", "create"]).failure().stderr_has("--name");
    cli.run(&["issue", "create"])
        .failure()
        .stderr_has("--title");
    cli.run(&["document", "create", "--team", "ENG"])
        .failure()
        .stderr_has("--title");
    assert!(api.requests().is_empty());
}

#[test]
fn issue_create_interactive_takes_no_field_flags() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&["issue", "create", "-i", "--title", "x"])
        .usage_error()
        .stderr_has("--title");
    assert!(api.requests().is_empty());
}
