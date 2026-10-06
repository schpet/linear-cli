//! Config file discovery, precedence and validation.
use crate::support::{Cli, MockLinear};
use crate::team::resolved;

const INLINE: &str = "default = \"beta\"\nacme = \"key-acme\"\nbeta = \"key-beta\"\n";
const GLOBAL: &str = "home/.config/linear/linear.toml";

fn stdout(cli: &Cli, args: &[&str]) -> String {
    let run = cli.run(args);
    run.success();
    run.stdout.trim_end().to_owned()
}

#[test]
fn api_key_precedence_is_env_then_project_then_global_then_credentials() {
    let cli = Cli::new().credentials(INLINE);
    assert_eq!(stdout(&cli, &["auth", "token"]), "key-beta");
    let cli = cli.file(GLOBAL, "api_key = \"key-global\"\n");
    assert_eq!(stdout(&cli, &["auth", "token"]), "key-global");
    let cli = cli.file("cwd/.linear.toml", "api_key = \"key-project\"\n");
    assert_eq!(stdout(&cli, &["auth", "token"]), "key-project");
    let cli = cli.env("LINEAR_API_KEY", "key-env");
    assert_eq!(stdout(&cli, &["auth", "token"]), "key-env");
}

#[test]
fn project_config_may_be_named_linear_toml() {
    let cli = Cli::new().file("cwd/linear.toml", "team_id = \"OPS\"\n");
    assert_eq!(stdout(&cli, &["team", "id"]), "OPS");
}

#[test]
fn project_workspace_selects_credentials() {
    let cli = Cli::new()
        .credentials(INLINE)
        .file("cwd/.linear.toml", "workspace = \"acme\"\n");
    assert_eq!(stdout(&cli, &["auth", "token"]), "key-acme");
}

#[test]
fn team_id_comes_from_env_over_config() {
    let cli = Cli::new();
    cli.run(&["team", "id"]).failure().stderr_has("team");
    let cli = cli.file(GLOBAL, "team_id = \"GLOBAL\"\n");
    assert_eq!(stdout(&cli, &["team", "id"]), "GLOBAL");
    let cli = cli.file("cwd/.linear.toml", "team_id = \"OPS\"\n");
    assert_eq!(stdout(&cli, &["team", "id"]), "OPS");
    let cli = cli.env("LINEAR_TEAM_ID", "ENG");
    assert_eq!(stdout(&cli, &["team", "id"]), "ENG");
}

#[test]
fn project_config_is_found_at_the_repository_root() {
    let cli = Cli::new();
    let repo = cli.path("repo");
    let cli = cli
        .stub_bin(
            "git",
            &format!(
                "[ \"$*\" = 'rev-parse --show-toplevel' ] && echo '{}' && exit 0\nexit 1",
                repo.display()
            ),
        )
        .file("repo/.git/HEAD", "ref: refs/heads/main\n")
        .file("repo/.linear.toml", "team_id = \"ROOT\"\n")
        .cwd("repo/nested/dir");
    assert_eq!(stdout(&cli, &["team", "id"]), "ROOT");
}

#[test]
fn dotenv_supplies_linear_variables_unless_ignored() {
    let cli = Cli::new().file("cwd/.env", "LINEAR_API_KEY=key-dotenv\n");
    cli.run(&["auth", "token"]).auth_failure();
    let cli = cli.env_remove("LINEAR_IGNORE_ENV_FILE");
    assert_eq!(stdout(&cli, &["auth", "token"]), "key-dotenv");
    let cli = cli.env("LINEAR_API_KEY", "key-env");
    assert_eq!(stdout(&cli, &["auth", "token"]), "key-env");
}

#[test]
fn dotenv_never_sends_a_variable_reference_as_the_key() {
    let cli = Cli::new()
        .file("cwd/.env", "LINEAR_API_KEY=${SECRET_KEY}\n")
        .env_remove("LINEAR_IGNORE_ENV_FILE")
        .env("SECRET_KEY", "key-secret");
    cli.run(&["auth", "token"])
        .auth_failure()
        .stderr_has("Ignoring LINEAR_API_KEY")
        .stderr_has("${SECRET_KEY} would be used literally")
        .stderr_has("single-quote");
}

#[test]
fn invalid_config_fails_with_the_file_path() {
    Cli::new()
        .file("cwd/.linear.toml", "team_id = \"unterminated\n")
        .run(&["team", "id"])
        .failure()
        .stderr_has(".linear.toml");
    Cli::new()
        .file("cwd/.linear.toml", "team_id = 5\n")
        .run(&["team", "id"])
        .failure()
        .stderr_has("team_id");
}

#[test]
fn invalid_endpoint_fails_before_any_request() {
    Cli::new()
        .env("LINEAR_API_KEY", "key")
        .env("LINEAR_GRAPHQL_ENDPOINT", "ftp://example.com/graphql")
        .run(&["auth", "whoami"])
        .failure()
        .stderr_has("LINEAR_GRAPHQL_ENDPOINT");
}

/// Workspace keys listed in the metadata-form credentials file live in the Secret Service,
/// read through `secret-tool`.
#[cfg(target_os = "linux")]
#[test]
fn metadata_credentials_read_keys_from_secret_tool() {
    let cli = Cli::new()
        .credentials("default = \"acme\"\nworkspaces = [\"acme\", \"beta\"]\n")
        .stub_bin(
            "secret-tool",
            "[ \"$1 $2 $3 $4\" = 'lookup service linear-cli account' ] || exit 2\n\
             printf 'key-%s\\n' \"$5\"",
        );
    assert_eq!(stdout(&cli, &["auth", "token"]), "key-acme");
    assert_eq!(
        stdout(&cli, &["auth", "token", "--workspace", "beta"]),
        "key-beta"
    );
}

fn viewer() -> serde_json::Value {
    serde_json::json!({ "viewer": { "organization": { "urlKey": "acme" } } })
}

#[test]
fn config_writes_the_flags_at_the_repository_root() {
    let api = MockLinear::start();
    api.on("GetViewer", viewer())
        .on("ResolveTeam", resolved("team-eng-id", "ENG", "Engineering"));
    let cli = Cli::new()
        .endpoint(&api)
        .credentials(INLINE)
        .file("cwd/.jj/repo/.keep", "")
        .cwd("cwd/nested");
    cli.run(&[
        "config",
        "--workspace",
        "acme",
        "--team",
        "eng",
        "--sort",
        "priority",
    ])
    .success()
    .stdout_has("Configuration written to");
    assert_eq!(
        api.request("GetViewer").header("authorization"),
        Some("key-acme")
    );
    assert_eq!(
        cli.read("cwd/.linear.toml"),
        "# linear cli\n# https://github.com/schpet/linear-cli\n\nworkspace = \"acme\"\nteam_id = \"ENG\"\nissue_sort = \"priority\"\n"
    );
    assert_eq!(stdout(&cli, &["team", "id"]), "ENG");
}

#[test]
fn config_uses_the_api_key_without_a_workspace() {
    let api = MockLinear::start();
    api.on("GetViewer", viewer())
        .on("ResolveTeam", resolved("team-ops-id", "OPS", "Operations"));
    let cli = Cli::for_api(&api);
    cli.run(&["config", "--team", "OPS", "--sort", "manual"])
        .success();
    assert!(
        cli.read("cwd/.linear.toml")
            .contains("team_id = \"OPS\"\nissue_sort = \"manual\"\n")
    );
}

#[test]
fn config_without_a_terminal_names_the_missing_flags() {
    let api = MockLinear::start();
    let cli = Cli::new().endpoint(&api).credentials(INLINE);
    cli.run(&["config", "--team", "ENG"])
        .failure()
        .stderr_has("--sort")
        .stderr_has("--workspace");
    assert!(api.requests().is_empty());
}

#[test]
fn config_without_credentials_points_to_login() {
    Cli::new()
        .run(&["config"])
        .auth_failure()
        .stderr_has("linear auth login");
}
