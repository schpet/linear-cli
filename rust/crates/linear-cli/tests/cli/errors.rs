//! How failures are reported: the `✗` line, hints, `LINEAR_DEBUG` detail, exit statuses, and
//! the closed-stdout rule.
use std::process::{Command, Stdio};

use crate::support::{Cli, MockLinear};

#[test]
fn graphql_errors_show_debug_detail_only_under_linear_debug() {
    let api = MockLinear::start();
    let body = r#"{"errors":[{"message":"Backend failed","extensions":{"userPresentableMessage":"Try again"}}]}"#;
    api.on_raw("AuthStatus", 400, body)
        .on_raw("AuthStatus", 400, body);
    let plain = Cli::for_api(&api).run(&["auth", "whoami"]);
    plain.failure().stderr_has("✗ ").stderr_has("Try again");
    assert!(!plain.stderr.contains("debug:"), "{plain}");
    let debug = Cli::for_api(&api)
        .env("LINEAR_DEBUG", "1")
        .run(&["auth", "whoami"]);
    debug
        .failure()
        .stderr_has("  debug: GraphQL HTTP 400 Bad Request; errors=1; partial_data=false");
    assert!(!debug.stderr.contains("Backend failed"), "{debug}");
    assert!(!debug.stderr.contains(crate::support::API_KEY), "{debug}");
}

#[test]
fn network_failures_show_a_cause_chain_without_secrets() {
    // Nothing listens on port 1, so the connection is refused.
    let run = Cli::new()
        .env("LINEAR_API_KEY", crate::support::API_KEY)
        .env(
            "LINEAR_GRAPHQL_ENDPOINT",
            "http://127.0.0.1:1/graphql?sentinel_query=private",
        )
        .env("LINEAR_DEBUG", "1")
        .run(&["auth", "whoami"]);
    run.failure()
        .stderr_has("✗ ")
        .stderr_has("http://127.0.0.1:1")
        .stderr_has("  caused by: ");
    assert!(!run.stderr.contains("sentinel_query"), "{run}");
    assert!(!run.stderr.contains(crate::support::API_KEY), "{run}");
}

#[test]
fn http_failures_show_a_sanitized_body_excerpt() {
    let api = MockLinear::start();
    let long = format!(
        "upstream \x1b[31mexploded\x1b[0m\n\tkey={} {}",
        crate::support::API_KEY,
        "x".repeat(300)
    );
    api.on_text("AuthStatus", 500, "text/plain", &long)
        .on_text("AuthStatus", 500, "text/plain", &long)
        .on_text("AuthStatus", 502, "text/html", "<html>bad gateway</html>");

    let plain = Cli::for_api(&api).run(&["auth", "whoami"]);
    plain
        .failure()
        .stderr_has("unexpected HTTP status 500 Internal Server Error: upstream [31mexploded[0m key=<redacted> xxx")
        .stderr_has("x…");
    assert!(!plain.stderr.contains('\x1b'), "{plain}");
    assert!(!plain.stderr.contains(&"x".repeat(300)), "{plain}");
    assert!(!plain.stderr.contains(crate::support::API_KEY), "{plain}");

    let debug = Cli::for_api(&api)
        .env("LINEAR_DEBUG", "1")
        .run(&["auth", "whoami"]);
    debug
        .failure()
        .stderr_has("  debug: HTTP 500 Internal Server Error body: upstream")
        .stderr_has(&"x".repeat(300));
    assert!(!debug.stderr.contains(crate::support::API_KEY), "{debug}");

    let html = Cli::for_api(&api).run(&["auth", "whoami"]);
    html.failure()
        .stderr_has("unexpected HTTP status 502 Bad Gateway\n");
    assert!(!html.stderr.contains("<html>"), "{html}");
}

#[test]
fn missing_credentials_fail_with_a_login_hint() {
    let api = MockLinear::start();
    Cli::new()
        .endpoint(&api)
        .run(&["team", "list"])
        .failure()
        .stderr_has("No API key configured")
        .stderr_has("linear auth login");
    assert!(api.requests().is_empty());
}

#[test]
fn help_and_version_ignore_invalid_configuration() {
    let cli = Cli::new()
        .env("LINEAR_GRAPHQL_ENDPOINT", "not a url")
        .file("cwd/.linear.toml", "issue_sort = 'sideways'\n");
    cli.run(&["--help"]).success().stdout_has("Usage");
    cli.run(&["issue", "--help"]).success().stdout_has("Usage");
    cli.run(&["--version"]).success();
    cli.run(&["team", "id"]).failure().stderr_has("issue_sort");
}

#[test]
fn a_missing_subcommand_prints_help_as_a_usage_error() {
    let run = Cli::new().run(&["issue"]);
    run.usage_error().stderr_has("Usage: linear issue");
}

#[test]
fn a_closed_stdout_ends_the_command_quietly() {
    let (reader, writer) = std::io::pipe().expect("pipe");
    drop(reader);
    let output = Command::new(env!("CARGO_BIN_EXE_linear"))
        .arg("markdown")
        .env_clear()
        .stdin(Stdio::null())
        .stdout(writer)
        .stderr(Stdio::piped())
        .output()
        .expect("run linear");
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty(), "{output:?}");
}
