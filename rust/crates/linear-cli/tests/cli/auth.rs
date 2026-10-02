//! Credential resolution and the `auth` command group.
use serde_json::json;

use crate::harness::viewer;
use crate::support::{Cli, MockLinear};

const INLINE: &str = "default = \"beta\"\nacme = \"key-acme\"\nbeta = \"key-beta\"\n";

fn token(cli: &Cli, args: &[&str]) -> String {
    let mut argv = vec!["auth", "token"];
    argv.extend_from_slice(args);
    let run = cli.run(&argv);
    run.success();
    run.stdout.trim_end().to_owned()
}

#[test]
fn env_key_is_used() {
    assert_eq!(
        token(&Cli::new().env("LINEAR_API_KEY", "lin_env"), &[]),
        "lin_env"
    );
}

#[test]
fn missing_key_fails_with_guidance() {
    Cli::new()
        .run(&["auth", "token"])
        .failure()
        .stderr_has("No API key configured")
        .stderr_has("linear auth login");
}

#[test]
fn inline_credentials_use_default_or_selected_workspace() {
    let cli = Cli::new().credentials(INLINE);
    assert_eq!(token(&cli, &[]), "key-beta");
    assert_eq!(token(&cli, &["--workspace", "acme"]), "key-acme");
    assert_eq!(
        cli.run(&["--workspace", "acme", "auth", "token"])
            .success()
            .stdout
            .trim_end(),
        "key-acme"
    );
}

#[test]
fn unknown_workspace_fails() {
    Cli::new()
        .credentials(INLINE)
        .run(&["auth", "token", "--workspace", "nope"])
        .failure()
        .stderr_has("\"nope\"");
}

#[test]
fn env_key_conflicts_with_workspace_flag() {
    Cli::new()
        .credentials(INLINE)
        .env("LINEAR_API_KEY", "lin_env")
        .run(&["auth", "token", "--workspace", "acme"])
        .failure()
        .stderr_has("--workspace");
}

#[test]
fn credentials_fall_back_to_home_config_without_xdg() {
    let cli = Cli::new().env_remove("XDG_CONFIG_HOME").credentials(INLINE);
    assert_eq!(token(&cli, &[]), "key-beta");
}

#[test]
fn whoami_sends_the_selected_workspace_key() {
    let api = MockLinear::start();
    api.on("AuthStatus", viewer());
    Cli::new()
        .endpoint(&api)
        .credentials(INLINE)
        .run(&["auth", "whoami", "--workspace", "acme"])
        .success()
        .stdout_has("Acme")
        .stdout_has("alice@example.com");
    assert_eq!(
        api.request("AuthStatus").header("authorization"),
        Some("key-acme")
    );
}

#[test]
fn whoami_reports_api_errors() {
    let api = MockLinear::start();
    api.on_error("AuthStatus", "Authentication required");
    Cli::for_api(&api)
        .run(&["auth", "whoami"])
        .failure()
        .stderr_has("Authentication required");
}

fn login_viewer() -> serde_json::Value {
    json!({
        "viewer": {
            "name": "Alice Example", "email": "alice@example.com",
            "organization": { "name": "Acme", "urlKey": "acme" }
        }
    })
}

fn credentials_toml(cli: &Cli) -> toml::Table {
    cli.read("home/.config/linear/credentials.toml")
        .parse()
        .expect("credentials file is TOML")
}

#[test]
fn plaintext_login_writes_inline_credentials() {
    let api = MockLinear::start();
    api.on("AuthLoginViewer", login_viewer());
    let cli = Cli::new().endpoint(&api);
    cli.run(&["auth", "login", "--key", "lin_new", "--plaintext"])
        .success()
        .stdout_has("acme");
    assert_eq!(
        api.request("AuthLoginViewer").header("authorization"),
        Some("lin_new")
    );
    let file = credentials_toml(&cli);
    assert_eq!(file.get("acme").and_then(|v| v.as_str()), Some("lin_new"));
    assert_eq!(token(&cli, &[]), "lin_new");
}

#[test]
fn plaintext_login_adds_to_existing_inline_credentials() {
    let api = MockLinear::start();
    let mut viewer = login_viewer();
    viewer["viewer"]["organization"]["urlKey"] = json!("gamma");
    api.on("AuthLoginViewer", viewer);
    // Declines the offer to move existing plaintext credentials to the keyring.
    let cli = Cli::new().endpoint(&api).credentials(INLINE).stdin(b"n\n");
    cli.run(&["auth", "login", "--key", "key-gamma", "--plaintext"])
        .success();
    assert_eq!(token(&cli, &["--workspace", "gamma"]), "key-gamma");
    assert_eq!(token(&cli, &["--workspace", "acme"]), "key-acme");
}

#[test]
fn rejected_login_key_is_not_saved() {
    let api = MockLinear::start();
    api.on_raw(
        "AuthLoginViewer",
        401,
        r#"{"errors":[{"message":"Authentication required, not authenticated"}]}"#,
    );
    let cli = Cli::new().endpoint(&api);
    cli.run(&["auth", "login", "--key", "lin_bad", "--plaintext"])
        .failure();
    assert!(!cli.path("home/.config/linear/credentials.toml").exists());
}

#[test]
fn default_switches_the_default_workspace() {
    let cli = Cli::new().credentials(INLINE);
    cli.run(&["auth", "default", "acme"])
        .success()
        .stdout_has("acme");
    assert_eq!(token(&cli, &[]), "key-acme");
    cli.run(&["auth", "default", "nope"]).failure();
}

#[test]
fn logout_removes_a_workspace() {
    let cli = Cli::new().credentials(INLINE);
    cli.run(&["auth", "logout", "beta", "--force"])
        .success()
        .stdout_has("beta");
    let file = credentials_toml(&cli);
    assert!(file.get("beta").is_none());
    assert_eq!(token(&cli, &[]), "key-acme");
}

#[test]
fn logout_without_force_needs_a_confirmation() {
    let cli = Cli::new().credentials(INLINE);
    cli.run(&["auth", "logout", "acme"]).failure();
    assert_eq!(token(&cli, &["--workspace", "acme"]), "key-acme");
    let cli = cli.stdin(b"y\n");
    cli.run(&["auth", "logout", "acme"]).success();
    cli.run(&["auth", "token", "--workspace", "acme"]).failure();
}

#[test]
fn list_shows_each_workspace_with_its_organization() {
    let api = MockLinear::start();
    let mut beta = login_viewer();
    beta["viewer"]["organization"]["name"] = json!("Beta Org");
    api.on("AuthListViewer", login_viewer())
        .on("AuthListViewer", beta);
    let run = Cli::new()
        .endpoint(&api)
        .credentials(INLINE)
        .run(&["auth", "list"]);
    run.success().stdout_has("Acme").stdout_has("Beta Org");
    let mut keys: Vec<_> = api
        .requests()
        .iter()
        .map(|request| {
            request
                .header("authorization")
                .unwrap_or_default()
                .to_owned()
        })
        .collect();
    keys.sort();
    assert_eq!(keys, ["key-acme", "key-beta"]);
}
