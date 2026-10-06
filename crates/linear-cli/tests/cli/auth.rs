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
        .auth_failure()
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
        .auth_failure()
        .stderr_has("\"nope\"");
}

#[test]
fn env_key_conflicts_with_workspace_flag() {
    Cli::new()
        .credentials(INLINE)
        .env("LINEAR_API_KEY", "lin_env")
        .run(&["auth", "token", "--workspace", "acme"])
        .usage_error()
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

const CREDENTIALS: &str = "home/.config/linear/credentials.toml";

fn credentials_toml(cli: &Cli) -> toml::Table {
    cli.read(CREDENTIALS)
        .parse()
        .expect("credentials file is TOML")
}

#[test]
fn plaintext_login_writes_inline_credentials() {
    let api = MockLinear::start();
    api.on("GetViewerAccount", login_viewer());
    let cli = Cli::new().endpoint(&api);
    cli.run(&["auth", "login", "--key", "lin_new", "--plaintext"])
        .success()
        .stdout_has("acme");
    assert_eq!(
        api.request("GetViewerAccount").header("authorization"),
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
    api.on("GetViewerAccount", viewer);
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
        "GetViewerAccount",
        401,
        r#"{"errors":[{"message":"Authentication required, not authenticated"}]}"#,
    );
    let cli = Cli::new().endpoint(&api);
    cli.run(&["auth", "login", "--key", "lin_bad", "--plaintext"])
        .auth_failure();
    assert!(!cli.path("home/.config/linear/credentials.toml").exists());
}

#[test]
fn default_switches_the_default_workspace() {
    let cli = Cli::new().credentials(INLINE);
    cli.run(&["auth", "default", "acme"])
        .success()
        .stdout_has("acme");
    assert_eq!(token(&cli, &[]), "key-acme");
    cli.run(&["auth", "default", "nope"]).usage_error();
}

#[test]
fn logout_removes_a_workspace() {
    let cli = Cli::new().credentials(INLINE);
    cli.run(&["auth", "logout", "beta", "--yes"])
        .success()
        .stdout_has("beta");
    let file = credentials_toml(&cli);
    assert!(file.get("beta").is_none());
    assert_eq!(token(&cli, &[]), "key-acme");
}

#[test]
fn logout_and_default_take_the_workspace_from_either_spelling() {
    let cli = Cli::new().credentials(INLINE);
    cli.run(&["auth", "default", "--workspace", "beta"])
        .success()
        .stdout_has("beta");
    cli.run(&["auth", "logout", "acme", "--workspace", "beta", "--yes"])
        .failure()
        .stderr_has("Two different workspaces");
    cli.run(&["auth", "logout", "--workspace", "acme", "--yes"])
        .success()
        .stdout_has("acme");
    let file = credentials_toml(&cli);
    assert!(file.get("acme").is_none());
}

#[test]
fn logout_without_yes_needs_a_terminal() {
    let cli = Cli::new().credentials(INLINE).stdin(b"y\n");
    cli.run(&["auth", "logout", "acme"])
        .usage_error()
        .stderr_has("--yes");
    assert_eq!(cli.read(CREDENTIALS), INLINE);
}

#[test]
fn list_shows_each_workspace_with_its_organization() {
    let api = MockLinear::start();
    let mut beta = login_viewer();
    beta["viewer"]["organization"]["name"] = json!("Beta Org");
    api.on("GetViewerAccount", login_viewer())
        .on("GetViewerAccount", beta);
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

#[test]
fn login_reads_a_piped_key() {
    let api = MockLinear::start();
    api.on("GetViewerAccount", login_viewer());
    let cli = Cli::new().endpoint(&api).stdin(b"  lin_piped\n");
    cli.run(&["auth", "login", "--plaintext"]).success();
    assert_eq!(
        api.request("GetViewerAccount").header("authorization"),
        Some("lin_piped")
    );
    assert_eq!(token(&cli, &[]), "lin_piped");
}

#[test]
fn login_rejects_a_key_that_is_only_punctuation_before_any_request() {
    let api = MockLinear::start();
    let cli = Cli::new().endpoint(&api);
    cli.run(&["auth", "login", "--key", " \"!\" ", "--plaintext"])
        .failure()
        .stderr_has("No API key provided");
    assert!(api.requests().is_empty());
    assert!(!cli.path(CREDENTIALS).exists());
}

#[test]
fn login_reports_an_authentication_error_as_an_invalid_key() {
    let api = MockLinear::start();
    api.on_raw(
        "GetViewerAccount",
        400,
        r#"{"errors":[{"message":"Authentication required","extensions":{"code":"AUTHENTICATION_ERROR"}}]}"#,
    );
    let cli = Cli::new().endpoint(&api);
    cli.run(&["auth", "login", "--key", "lin_bad", "--plaintext"])
        .auth_failure()
        .stderr_has("Invalid API key");
    assert!(!cli.path(CREDENTIALS).exists());
}

#[test]
fn login_does_not_save_an_unexpected_viewer() {
    let api = MockLinear::start();
    let mut viewer = login_viewer();
    viewer["viewer"]["name"] = json!(null);
    api.on("GetViewerAccount", viewer);
    let cli = Cli::new().endpoint(&api);
    cli.run(&["auth", "login", "--key", "lin_new", "--plaintext"])
        .failure();
    assert!(!cli.path(CREDENTIALS).exists());
}

#[test]
fn login_into_plaintext_credentials_suggests_migrating_without_a_terminal() {
    let api = MockLinear::start();
    let mut viewer = login_viewer();
    viewer["viewer"]["organization"]["urlKey"] = json!("gamma");
    api.on("GetViewerAccount", viewer);
    // Linux checks for secret-tool before offering the keyring.
    let cli = Cli::new()
        .endpoint(&api)
        .credentials(INLINE)
        .stub_bin("secret-tool", "exit 0");
    cli.run(&["auth", "login", "--key", "key-gamma"])
        .success()
        .stdout_has("stored as plaintext to match existing format")
        .stderr_has("linear auth migrate");
    assert_eq!(
        credentials_toml(&cli).get("gamma").and_then(|v| v.as_str()),
        Some("key-gamma")
    );
}

#[test]
fn login_warns_on_stderr_when_linear_api_key_is_set() {
    for (name, cli) in [
        ("env", Cli::new().env("LINEAR_API_KEY", "lin_env")),
        (
            "dotenv",
            Cli::new()
                .env_remove("LINEAR_IGNORE_ENV_FILE")
                .file("cwd/.env", "LINEAR_API_KEY=lin_env\n"),
        ),
        (
            "config file",
            Cli::new().file("cwd/.linear.toml", "api_key = \"lin_toml\"\n"),
        ),
    ] {
        let api = MockLinear::start();
        api.on("GetViewerAccount", login_viewer());
        let run = cli
            .endpoint(&api)
            .run(&["auth", "login", "--key", "lin_new", "--plaintext"]);
        run.success();
        assert_eq!(
            run.stderr.contains("LINEAR_API_KEY is set"),
            name != "config file",
            "{name}: {run}"
        );
        assert!(!run.stdout.contains("LINEAR_API_KEY"), "{name}: {run}");
    }
}

#[test]
fn credential_commands_refuse_an_invalid_credentials_file() {
    let cli = Cli::new().credentials("workspaces = 23\n");
    for command in [
        vec!["auth", "login", "--key", "lin_new"],
        vec!["auth", "logout", "acme", "--yes"],
        vec!["auth", "migrate"],
        vec!["auth", "default", "acme"],
    ] {
        cli.run(&command)
            .failure()
            .stderr_has("invalid credentials file");
    }
    assert_eq!(cli.read(CREDENTIALS), "workspaces = 23\n");
}

#[test]
fn migrate_reports_keyring_credentials_as_done() {
    Cli::new()
        .credentials("default = \"acme\"\nworkspaces = [\"acme\"]\n")
        .run(&["auth", "migrate"])
        .success()
        .stdout_has("already using the system keyring");
}

/// A fake `secret-tool` that logs each call's action and stdin under `calls/`.
#[cfg(target_os = "linux")]
fn secret_tool(cli: Cli, on_store_or_clear: &str) -> Cli {
    cli.stub_bin(
        "secret-tool",
        &format!(
            "[ \"$#\" -eq 0 ] && exit 2\n\
             if [ \"$1\" = lookup ]; then printf 'key-%s' \"$5\"; exit 0; fi\n\
             if [ \"$1\" = store ]; then cat > \"$0.stdin\"; fi\n\
             {on_store_or_clear}"
        ),
    )
}

#[cfg(target_os = "linux")]
#[test]
fn login_stores_the_key_with_secret_tool() {
    let api = MockLinear::start();
    api.on("GetViewerAccount", login_viewer());
    let cli = secret_tool(Cli::new().endpoint(&api), "exit 0");
    cli.run(&["auth", "login", "--key", "lin_new"])
        .success()
        .stdout_has("Logged in to workspace: Acme (acme)");
    assert_eq!(
        cli.read(CREDENTIALS),
        "default = \"acme\"\nworkspaces = [\"acme\"]\n"
    );
    assert_eq!(cli.read("bin/secret-tool.stdin"), "lin_new");
    assert_eq!(cli.calls("secret-tool")[1][0], "store");
}

#[cfg(target_os = "linux")]
#[test]
fn a_keyring_failure_is_not_reported_as_an_invalid_key() {
    let api = MockLinear::start();
    api.on("GetViewerAccount", login_viewer());
    let cli = secret_tool(
        Cli::new().endpoint(&api),
        "echo 'error 401 from the secret service' >&2; exit 3",
    );
    let run = cli.run(&["auth", "login", "--key", "lin_new"]);
    run.failure()
        .stderr_has("Failed to store API key in system keyring for workspace \"acme\"")
        .stderr_has("secret-tool store failed (exit 3)");
    assert!(!run.stderr.contains("Invalid API key"), "{run}");
    assert!(!cli.path(CREDENTIALS).exists());
}

#[cfg(target_os = "linux")]
#[test]
fn logout_deletes_the_keyring_entry_first() {
    let keyring = "default = \"acme\"\nworkspaces = [\"acme\", \"beta\"]\n";
    let cli = secret_tool(Cli::new().credentials(keyring), "exit 4");
    cli.run(&["auth", "logout", "beta", "--yes"])
        .failure()
        .stderr_has("secret-tool clear failed (exit 4)");
    assert_eq!(cli.read(CREDENTIALS), keyring);
    let cli = secret_tool(Cli::new().credentials(keyring), "exit 0");
    cli.run(&["auth", "logout", "beta", "--yes"]).success();
    assert_eq!(
        cli.read(CREDENTIALS),
        "default = \"acme\"\nworkspaces = [\"acme\"]\n"
    );
    assert_eq!(
        cli.calls("secret-tool"),
        [vec!["clear", "service", "linear-cli", "account", "beta"]]
    );
}

#[test]
fn a_configured_workspace_without_credentials_never_falls_back_to_the_default() {
    let cli = Cli::new()
        .credentials(INLINE)
        .file("cwd/.linear.toml", "workspace = \"ghost\"\n");
    let run = cli.run(&["auth", "token"]);
    run.auth_failure()
        .stderr_has("Workspace \"ghost\" (workspace set in project config")
        .stderr_has("not found in credentials");
    assert!(!run.stdout.contains("key-beta"), "{run}");
    let run = Cli::new()
        .credentials(INLINE)
        .env("LINEAR_WORKSPACE", "ghost")
        .run(&["auth", "token"]);
    run.auth_failure()
        .stderr_has("(workspace set in process environment)");
}

#[test]
fn a_dotenv_api_key_conflict_names_the_file() {
    Cli::new()
        .credentials(INLINE)
        .file("cwd/.env", "LINEAR_API_KEY=lin_env\n")
        .env_remove("LINEAR_IGNORE_ENV_FILE")
        .run(&["auth", "token", "--workspace", "acme"])
        .usage_error()
        .stderr_has("Cannot use --workspace while LINEAR_API_KEY is set in ")
        .stderr_has(".env");
}

#[test]
fn urls_are_checked_against_the_workspace_of_the_credential_in_use() {
    // An API key from the environment belongs to no stored workspace, so the
    // stored default does not decide which URLs are foreign.
    let api = MockLinear::start();
    api.on(
        "ResolveInitiativeBySlug",
        json!({ "initiatives": { "nodes": [] } }),
    );
    let url = "https://linear.app/acme/initiative/roadmap-1a2b3c4d5e6f";
    let run = Cli::for_api(&api)
        .credentials("default = \"other\"\nother = \"key-other\"\n")
        .run(&["initiative", "view", url]);
    assert!(!run.stderr.contains("That URL is for"), "{run}");
    // The default workspace's stored key is in use: its URLs are local.
    Cli::new()
        .credentials("default = \"other\"\nother = \"key-other\"\n")
        .run(&["initiative", "view", url])
        .failure()
        .stderr_has("That URL is for the \"acme\" workspace, but this is the \"other\" workspace.");
}
