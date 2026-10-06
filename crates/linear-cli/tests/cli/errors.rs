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
fn failures_show_the_first_nonempty_graphql_message_or_the_http_status() {
    let api = MockLinear::start();
    api.on_raw(
        "GetIssuesForQuery",
        200,
        r#"{"errors":[{"message":""},{"message":"boom"}]}"#,
    )
    .on_text("GetIssuesForQuery", 500, "text/plain", "upstream exploded");
    let cli = Cli::for_api(&api);
    let args = ["issue", "query", "--all-teams", "--json"];
    let run = cli.run(&args);
    run.failure()
        .stderr_has("✗ Failed to query issues: ")
        .stderr_has("boom");
    assert_eq!(run.stdout, "");
    let run = cli.run(&args);
    run.unavailable()
        .stderr_has("✗ Failed to query issues: ")
        .stderr_has("500");
    assert_eq!(run.stdout, "");
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
    run.unavailable()
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
        .unavailable()
        .stderr_has("unexpected HTTP status 500 Internal Server Error: upstream [31mexploded[0m key=<redacted> xxx")
        .stderr_has("x…");
    assert!(!plain.stderr.contains('\x1b'), "{plain}");
    assert!(!plain.stderr.contains(&"x".repeat(300)), "{plain}");
    assert!(!plain.stderr.contains(crate::support::API_KEY), "{plain}");

    let debug = Cli::for_api(&api)
        .env("LINEAR_DEBUG", "1")
        .run(&["auth", "whoami"]);
    debug
        .unavailable()
        .stderr_has("  debug: HTTP 500 Internal Server Error body: upstream")
        .stderr_has(&"x".repeat(300));
    assert!(!debug.stderr.contains(crate::support::API_KEY), "{debug}");

    let html = Cli::for_api(&api).run(&["auth", "whoami"]);
    html.unavailable()
        .stderr_has("unexpected HTTP status 502 Bad Gateway\n");
    assert!(!html.stderr.contains("<html>"), "{html}");
}

#[test]
fn missing_credentials_fail_with_a_login_hint() {
    let api = MockLinear::start();
    Cli::new()
        .endpoint(&api)
        .run(&["team", "list"])
        .auth_failure()
        .stderr_has("No API key configured")
        .stderr_has("linear auth login");
    assert!(api.requests().is_empty());
}

#[test]
fn help_version_and_usage_errors_ignore_invalid_configuration() {
    let cli = Cli::new()
        .env("LINEAR_GRAPHQL_ENDPOINT", "not a url")
        .file("cwd/.linear.toml", "issue_sort = 'sideways'\n");
    cli.run(&["--help"]).success().stdout_has("Usage");
    cli.run(&["issue", "--help"]).success().stdout_has("Usage");
    cli.run(&["--version"]).success();
    let usage = cli.run(&["frobnicate"]);
    usage.usage_error();
    assert!(!usage.stderr.contains("issue_sort"), "{}", usage.stderr);
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

/// Linear's answer to a revoked or mistyped API key.
const UNAUTHENTICATED: &str = r#"{"errors":[{"message":"Authentication required, not authenticated","extensions":{"type":"authentication error","code":"AUTHENTICATION_ERROR","statusCode":401,"userError":true,"userPresentableMessage":"You need to authenticate to access this operation.","meta":{},"http":{"status":401}}}]}"#;
const ISSUE_NOT_FOUND: &str = r#"{"errors":[{"message":"Entity not found: Issue","extensions":{"type":"invalid input","code":"INPUT_ERROR","statusCode":400,"userError":true,"userPresentableMessage":"Could not find referenced Issue."}}],"data":null}"#;
const RATE_LIMITED: &str =
    r#"{"errors":[{"message":"Rate limit exceeded","extensions":{"code":"RATELIMITED"}}]}"#;

const VIEW: [&str; 4] = ["issue", "view", "ENG-1", "--json"];
const VIEW_OP: &str = "GetIssueDetailsWithComments";

#[test]
fn a_rejected_api_key_exits_4() {
    let api = MockLinear::start();
    api.on_raw(VIEW_OP, 401, UNAUTHENTICATED)
        .on_raw(VIEW_OP, 403, r#"{"errors":[{"message":"Forbidden"}]}"#)
        .on_raw(VIEW_OP, 200, UNAUTHENTICATED);
    let cli = Cli::for_api(&api);
    cli.run(&VIEW)
        .auth_failure()
        .stderr_has("You need to authenticate");
    cli.run(&VIEW).auth_failure();
    cli.run(&VIEW).auth_failure();
}

#[test]
fn missing_credentials_exit_4() {
    Cli::new()
        .run(&VIEW)
        .auth_failure()
        .stderr_has("No API key configured");
    Cli::new()
        .credentials("default = \"acme\"\nacme = \"lin_api_x\"\n")
        .run(&["--workspace", "other", "issue", "view", "ENG-1"])
        .auth_failure()
        .stderr_has("not found in credentials");
}

#[test]
fn a_missing_issue_exits_3() {
    let api = MockLinear::start();
    api.on_raw(VIEW_OP, 200, ISSUE_NOT_FOUND);
    Cli::for_api(&api)
        .run(&VIEW)
        .not_found()
        .stderr_has("Issue not found: ENG-1");
}

#[test]
fn a_missing_entity_alongside_a_rejected_key_is_an_authentication_failure() {
    let both = r#"{"errors":[{"message":"Entity not found: Issue"},{"message":"Authentication required","extensions":{"code":"AUTHENTICATION_ERROR"}}]}"#;
    let api = MockLinear::start();
    api.on_raw(VIEW_OP, 200, both);
    Cli::for_api(&api).run(&VIEW).auth_failure();
}

#[test]
fn an_unreachable_or_struggling_api_exits_5() {
    // Nothing listens on port 1, so the connection is refused.
    Cli::new()
        .env("LINEAR_API_KEY", crate::support::API_KEY)
        .env("LINEAR_GRAPHQL_ENDPOINT", "http://127.0.0.1:1/graphql")
        .run(&VIEW)
        .unavailable();
    let api = MockLinear::start();
    api.on_text(VIEW_OP, 503, "text/plain", "maintenance")
        .on_text(VIEW_OP, 429, "text/plain", "slow down")
        .on_raw(VIEW_OP, 400, RATE_LIMITED);
    let cli = Cli::for_api(&api);
    cli.run(&VIEW).unavailable();
    cli.run(&VIEW).unavailable();
    cli.run(&VIEW)
        .unavailable()
        .stderr_has("Rate limit exceeded");
}

#[test]
fn other_graphql_errors_and_unexpected_statuses_exit_1() {
    let api = MockLinear::start();
    api.on_raw(
        VIEW_OP,
        400,
        r#"{"errors":[{"message":"Argument invalid","extensions":{"code":"INPUT_ERROR"}}]}"#,
    )
    .on_text(VIEW_OP, 404, "text/plain", "no such route")
    .on_raw(
        VIEW_OP,
        200,
        r#"{"errors":[{"message":"Entity not found: Issue"},{"message":"Something else broke"}]}"#,
    );
    let cli = Cli::for_api(&api);
    cli.run(&VIEW).failure();
    cli.run(&VIEW).failure();
    cli.run(&VIEW).failure();
}

#[test]
fn root_help_documents_the_exit_statuses() {
    let run = Cli::new().run(&["--help"]);
    run.success()
        .stdout_has("Exit status:")
        .stdout_has("3  ")
        .stdout_has("4  ")
        .stdout_has("5  ");
}
