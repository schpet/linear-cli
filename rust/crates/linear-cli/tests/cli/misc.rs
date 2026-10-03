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
fn completions_cover_every_supported_shell() {
    let cli = Cli::new();
    for shell in ["bash", "zsh", "fish", "elvish", "powershell"] {
        let run = cli.run(&["completions", shell]);
        run.success();
        for word in [
            "issue",
            "initiative",
            "project-update",
            "workspace",
            "priority",
        ] {
            assert!(run.stdout.contains(word), "{shell} lacks {word}\n{run}");
        }
        assert!(
            !run.stdout.contains("Removed: use"),
            "{shell} offers hidden flags"
        );
    }
}

#[test]
fn completions_register_under_a_custom_name() {
    let run = Cli::new().run(&["completions", "fish", "--name", "lin"]);
    run.success().stdout_has("complete -c lin ");
    assert!(!run.stdout.contains("complete -c linear "), "{run}");
}

#[test]
fn completions_reject_unknown_shells_and_unsafe_names() {
    Cli::new().run(&["completions", "tcsh"]).usage_error();
    Cli::new()
        .run(&["completions", "bash", "--name", "x;rm"])
        .usage_error();
    Cli::new().run(&["completions"]).usage_error();
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
