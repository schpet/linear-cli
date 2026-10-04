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
    cli.run(&["label", "create"])
        .usage_error()
        .stderr_has("--name");
    cli.run(&["team", "create"])
        .usage_error()
        .stderr_has("--name");
    cli.run(&["issue", "create"])
        .usage_error()
        .stderr_has("--title");
    cli.run(&["document", "create", "--team", "ENG"])
        .usage_error()
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

#[test]
fn missing_values_under_no_input_do_not_suggest_a_terminal() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api).env("LINEAR_TEAM_ID", "ENG");
    let run = cli.run(&["--no-input", "issue", "create"]);
    run.usage_error().stderr_has("Pass --title");
    assert!(!run.stderr.contains("terminal"), "{run}");
    let run = cli.run(&["issue", "create"]);
    run.usage_error()
        .stderr_has("or run in a terminal to be asked");
    assert!(api.requests().is_empty());
}

#[test]
fn the_issue_start_picker_without_a_terminal_names_what_it_would_ask() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .run(&["--no-input", "issue", "start"])
        .usage_error()
        .stderr_has("No issue to start")
        .stderr_has("Pass an issue ID.");
    assert!(api.requests().is_empty());
}

#[test]
fn values_typed_at_prompts_are_confirmed_before_anything_is_created() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api).env("LINEAR_TEAM_ID", "ENG");
    for (args, prompt, question) in [
        (&["label", "create"][..], "Name:", "Create label \"Typed\"?"),
        (&["team", "create"], "Name:", "Create team \"Typed\"?"),
        (
            &["initiative", "create"],
            "Name:",
            "Create initiative \"Typed\"?",
        ),
        (&["project", "create"], "Name:", "Create project \"Typed\"?"),
    ] {
        cli.run_tty(args, &[(prompt, "Typed\r"), (question, "\r")])
            .success()
            .stdout_has("Canceled.");
    }
    assert!(api.requests().is_empty(), "{:?}", api.operations());
    // A document's attachment is looked up before its content is written.
    api.on(
        "ResolveTeam",
        crate::team::resolved("team-eng", "ENG", "Engineering"),
    );
    cli.run_tty(
        &["document", "create", "--team", "ENG", "--content", "Body"],
        &[("Title:", "Typed\r"), ("Create document \"Typed\"?", "\r")],
    )
    .success()
    .stdout_has("Canceled.");
    assert_eq!(api.operations(), ["ResolveTeam"]);
}

#[test]
fn yes_skips_the_question_after_typed_values() {
    let api = MockLinear::start();
    api.on(
        "CreateTeam",
        serde_json::json!({ "teamCreate": {
            "success": true,
            "team": { "id": "team-1", "key": "TYP", "name": "Typed" }
        } }),
    );
    let run = Cli::for_api(&api).run_tty(&["team", "create", "--yes"], &[("Name:", "Typed\r")]);
    run.success().stdout_has("Created team TYP: Typed");
    assert!(!run.stdout.contains("(y/N)"), "{run}");
}

#[test]
fn yes_no_questions_take_readline_keys_and_explain_bad_answers() {
    let api = MockLinear::start();
    // Ctrl-A then Ctrl-K clears the bad answer; typed as letters they
    // would leave one the question refuses again.
    Cli::for_api(&api)
        .run_tty(
            &["team", "create"],
            &[
                ("Name:", "Typed\r"),
                ("Create team \"Typed\"? (y/N)", "maybe\r"),
                ("✗ Type y for yes or n for no", "\u{1}\u{b}n\r"),
            ],
        )
        .success()
        .stdout_has("Create team \"Typed\"? No")
        .stdout_has("Canceled.");
    assert!(api.requests().is_empty());
}

#[test]
fn pasted_tabs_become_spaces() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run_tty(
            &["team", "create"],
            &[
                ("Name:", "\u{1b}[200~tab\tand\u{1b}[201~\r"),
                ("Create team \"tab and\"?", "\r"),
            ],
        )
        .success()
        .stdout_has("Canceled.");
}
