//! `skill`: the agent guide built into the binary.
use crate::support::Cli;

#[test]
fn show_prints_the_guide_without_configuration_credentials_or_network() {
    let run = Cli::new()
        .file("cwd/.linear.toml", "this is not toml [")
        .env("HTTPS_PROXY", "http://proxy.example.invalid:3128")
        .env("SSL_CERT_FILE", "/missing/ca.pem")
        .env_remove("HOME")
        .run(&["skill", "show"]);
    run.success()
        .stdout_has(&format!(
            "# Linear CLI (linear {})",
            env!("CARGO_PKG_VERSION")
        ))
        .stdout_has("linear issue comment add ")
        .stdout_has("Exit status:")
        .stdout_has("+++ [");
    assert_eq!(run.stderr, "", "{run}");
}

#[test]
fn skill_without_a_subcommand_prints_help_and_changes_nothing() {
    let cli = Cli::new();
    let run = cli.run(&["skill"]);
    run.usage_error().stderr_has("Usage: linear skill");
    assert!(!cli.path("home/.agents").exists());
}
