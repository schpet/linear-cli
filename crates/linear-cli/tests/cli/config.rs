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
fn config_preserves_templates_and_unknown_values_while_updating_managed_settings() {
    let api = MockLinear::start();
    api.on("GetViewer", viewer())
        .on("ResolveTeam", resolved("team-eng-id", "ENG", "Engineering"));
    let original = r#"workspace = "previous"
team_id = "OPS"
issue_sort = "manual"
vcs = "jj"
pr_template = "./old-pr.md"
custom_values = ["keep", "these"]

[custom_table]
enabled = true

[template-aliases]
review_policy = "Preserved comment review policy"

[templates]
skill_issue_comment = '{% include "builtin/skill_issue_comment" %}{% include "review_policy" %}'
"#;
    let cli = Cli::for_api(&api).file("cwd/.linear.toml", original);
    cli.run(&["config", "--team", "ENG", "--sort", "priority"])
        .success();
    let mut expected: toml::Table = original.parse().expect("original TOML");
    expected.remove("vcs");
    expected.remove("pr_template");
    for (key, value) in [
        ("workspace", "acme"),
        ("team_id", "ENG"),
        ("issue_sort", "priority"),
    ] {
        expected.insert(key.to_owned(), toml::Value::String(value.to_owned()));
    }
    let written: toml::Table = cli.read("cwd/.linear.toml").parse().expect("written TOML");
    assert_eq!(written, expected);
    cli.run(&["skill", "show", "issue", "comment", "add"])
        .success()
        .stdout_has("Preserved comment review policy");
}

#[test]
fn config_does_not_overwrite_an_invalid_unselected_destination() {
    let api = MockLinear::start();
    let original = "this is not TOML [";
    let cli = Cli::for_api(&api)
        .file("cwd/.jj/repo/.keep", "")
        .file("cwd/linear.toml", "team_id = \"ENG\"\n")
        .file("cwd/.config/linear.toml", original);
    cli.run(&["config", "--team", "ENG", "--sort", "manual"])
        .failure()
        .stderr_has(".config/linear.toml")
        .stderr_has("invalid TOML");
    assert_eq!(cli.read("cwd/.config/linear.toml"), original);
    assert!(api.requests().is_empty());
}

#[test]
fn config_rejects_a_nonregular_destination_before_sending_requests() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api)
        .file("cwd/.jj/repo/.keep", "")
        .file("cwd/linear.toml", "team_id = \"ENG\"\n");
    let destination = cli.path("cwd/.config/linear.toml");
    std::fs::create_dir_all(&destination).expect("destination directory");
    cli.run(&["config", "--team", "ENG", "--sort", "manual"])
        .failure()
        .stderr_has(".config/linear.toml");
    assert!(destination.is_dir());
    assert!(api.requests().is_empty());
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

#[test]
fn retired_settings_do_not_interrupt_ordinary_commands() {
    for value in ["git", "jj", "hg", "", "GIT"] {
        let cli = Cli::new()
            .env("LINEAR_VCS", value)
            .env("LINEAR_PR_TEMPLATE", "")
            .file("cwd/.linear.toml", "vcs = 1\npr_template = false\n")
            .file(GLOBAL, "vcs = ['anything']\npr_template = {}\n");
        let run = cli.run(&["issue", "id", "ENG-7"]);
        run.success();
        assert_eq!(run.stdout, "ENG-7\n");
        assert_eq!(run.stderr, "", "{run}");
    }
}

#[test]
fn config_check_is_offline_without_credentials_or_keyring_and_does_not_write() {
    let api = MockLinear::start();
    let original = "team_id = 'ENG'\nissue_sort = 'manual'\n";
    let cli = Cli::new()
        .endpoint(&api)
        .file("cwd/.linear.toml", original)
        .credentials("not valid TOML [")
        .stub_bin("secret-tool", "exit 1");
    let run = cli.run(&["config", "check"]);
    run.success();
    assert_eq!(run.stdout, "✓ Configuration is valid.\n");
    assert_eq!(run.stderr, "", "{run}");
    assert!(api.requests().is_empty());
    assert!(cli.calls("secret-tool").is_empty());
    assert_eq!(cli.read("cwd/.linear.toml"), original);
}

#[test]
fn config_check_reports_ignored_settings_without_exposing_values() {
    let cli = Cli::new()
        .env("LINEAR_VCS", "private-legacy-value")
        .env("LINEAR_PR_TEMPLATE", "private-template-path")
        .env("LINEAR_API_KEY", "private-api-key")
        .file("cwd/.linear.toml", "vcs = 1\npr_template = false\n")
        .file(GLOBAL, "vcs = {}\npr_template = []\n");
    let run = cli.run(&["config", "check"]);
    run.success()
        .stderr_has("Note: LINEAR_VCS from process environment is ignored")
        .stderr_has("Note: vcs from project config ")
        .stderr_has("Note: vcs from global config ")
        .stderr_has("Note: LINEAR_PR_TEMPLATE from process environment is ignored")
        .stderr_has("Note: pr_template from project config ")
        .stderr_has("Note: pr_template from global config ")
        .stderr_has("safe to leave this setting in place");
    assert_eq!(run.stdout, "✓ Configuration is valid.\n");
    assert_eq!(run.stderr.matches("Note:").count(), 6, "{run}");
    assert!(!run.stderr.contains("Warning:"), "{run}");
    assert!(!run.stderr.contains("Remove "), "{run}");
    for secret in [
        "private-legacy-value",
        "private-template-path",
        "private-api-key",
    ] {
        assert!(
            !run.stdout.contains(secret) && !run.stderr.contains(secret),
            "{run}"
        );
    }
}

#[test]
fn config_check_uses_selected_dotenv_and_respects_ignore_and_precedence() {
    let cli = Cli::new()
        .env_remove("LINEAR_IGNORE_ENV_FILE")
        .file("cwd/.env", "LINEAR_VCS=hg\nLINEAR_PR_TEMPLATE=\n");
    let run = cli.run(&["config", "check"]);
    run.success()
        .stderr_has("Note: LINEAR_VCS from .env file ")
        .stderr_has("Note: LINEAR_PR_TEMPLATE from .env file ");
    assert_eq!(run.stderr.matches("Note:").count(), 2, "{run}");
    let run = cli.run(&["issue", "id", "ENG-7"]);
    run.success();
    assert_eq!(run.stderr, "", "{run}");
    let cli = cli.env("LINEAR_VCS", "jj");
    let run = cli.run(&["config", "check"]);
    run.success()
        .stderr_has("LINEAR_VCS from process environment");
    assert!(!run.stderr.contains("LINEAR_VCS from .env file"), "{run}");
    let cli = cli.env("LINEAR_IGNORE_ENV_FILE", "1");
    let run = cli.run(&["config", "check"]);
    run.success();
    assert!(!run.stderr.contains("from .env file"), "{run}");
}

#[test]
fn config_check_rejects_broken_active_settings_files_and_overlays() {
    for config in [
        "issue_sort = 'unexpected'",
        "issue_sort = 1",
        "vcs = [",
        "[templates]\npr_title = '{{'",
    ] {
        let cli = Cli::new().file("cwd/.linear.toml", config);
        let run = cli.run(&["config", "check"]);
        run.failure().stderr_has(".linear.toml");
        assert_eq!(run.stdout, "", "{run}");
    }
    let cli = Cli::new()
        .file(GLOBAL, "issue_sort = 'unexpected'")
        .env("LINEAR_ISSUE_SORT", "manual");
    cli.run(&["config", "check"])
        .failure()
        .stderr_has("issue_sort");
    let cli = Cli::new().env("LINEAR_ISSUE_SORT", "unexpected");
    cli.run(&["config", "check"])
        .failure()
        .stderr_has("LINEAR_ISSUE_SORT");
    let cli = Cli::new().env("LINEAR_GRAPHQL_ENDPOINT", "bad");
    cli.run(&["config", "check"])
        .failure()
        .stderr_has("LINEAR_GRAPHQL_ENDPOINT");
    let cli = Cli::new().file("cwd/overlay.toml", "[templates]\npr_title = 1");
    cli.run(&["config", "check", "--config-file", "overlay.toml"])
        .failure()
        .stderr_has("overlay.toml");
    let run = cli.run(&["config", "check", "--config-file", "missing.toml"]);
    run.failure().stderr_has("missing.toml");
    assert_eq!(run.code, 1);
    assert_eq!(run.stdout, "");
    cli.run(&["config", "check", "--config", "templates.pr_title=1"])
        .failure()
        .stderr_has("pr_title");
    cli.run(&[
        "config",
        "check",
        "--config",
        "templates.pr_title='Custom title'",
    ])
    .success();
}

#[test]
fn config_replaces_a_file_with_the_retired_vcs_setting_without_it() {
    let api = MockLinear::start();
    api.on("GetViewer", viewer())
        .on("ResolveTeam", resolved("team-ops-id", "OPS", "Operations"));
    let cli = Cli::for_api(&api).file("cwd/.linear.toml", "vcs = \"jj\"\n");
    cli.run(&["config", "--team", "OPS", "--sort", "manual"])
        .success();
    let written = cli.read("cwd/.linear.toml");
    assert!(!written.contains("vcs"), "{written}");
    let run = cli.run(&["team", "id"]);
    run.success();
    assert!(!run.stderr.contains("Warning"), "{run}");
}

#[test]
fn inspect_templates_offline_with_namespaces_origins_and_no_secrets() {
    let cli = Cli::new()
        .credentials("secret credential text, invalid toml [")
        .env("LINEAR_GRAPHQL_ENDPOINT", "not a url")
        .env("HTTPS_PROXY", "not a proxy")
        .file(
            "cwd/.linear.toml",
            r#"
workspace = 'SECRET WORKSPACE'
team_id = 123
[templates]
skill_issue_comment = 'COMMENT POLICY'
[template-aliases]
team_policy = 'ALIAS CONTENT'
"#,
        );
    let list = cli.run(&["config", "list", "--show-origin"]);
    list.success()
        .stdout_has("# Source:")
        .stdout_has(".linear.toml")
        .stdout_has("COMMENT POLICY");
    let parsed: toml::Table = list.stdout.parse().expect("valid TOML listing");
    assert_eq!(parsed.len(), 2);
    assert_eq!(
        parsed["template-aliases"]["team_policy"].as_str(),
        Some("ALIAS CONTENT")
    );
    assert!(!list.stdout.contains("SECRET WORKSPACE"));
    assert!(!list.stdout.contains("secret credential"));
    let get = cli.run(&["config", "get", "templates.skill_issue_comment"]);
    get.success();
    assert_eq!(get.stdout, "COMMENT POLICY");
    let get = cli.run(&["config", "get", "template-aliases.team_policy"]);
    get.success();
    assert_eq!(get.stdout, "ALIAS CONTENT");
    for key in [
        "workspace",
        "templates.team_policy",
        "template-aliases.skill",
        "templates.missing",
    ] {
        cli.run(&["config", "get", key]).usage_error();
    }
}

#[test]
fn defaults_inspection_bypasses_bad_config_and_overlays() {
    let cli = Cli::new()
        .env_remove("HOME")
        .file("cwd/.linear.toml", "bad [");
    cli.run(&["config", "list"])
        .failure()
        .stderr_has(".linear.toml");
    let run = cli.run(&["config", "list", "--defaults", "--config-file", "missing"]);
    run.success();
    let parsed: toml::Table = run.stdout.parse().expect("TOML defaults");
    let hook = parsed["templates"]["skill_milestone_update"]
        .as_str()
        .expect("generated command hook");
    let get = cli.run(&[
        "config",
        "get",
        "templates.skill_milestone_update",
        "--defaults",
    ]);
    get.success();
    assert_eq!(get.stdout, hook);
    cli.run(&["config", "get", "templates.guide", "--defaults"])
        .success()
        .stdout_has("command_guidance");
    cli.run(&["config", "--team", "ENG", "list"]).usage_error();
    cli.run(&["config", "--sort", "priority", "get", "templates.guide"])
        .usage_error();
}

#[test]
fn ordinary_startup_applies_template_overlays_before_authentication() {
    let cli = Cli::new();
    let run = cli.run(&[
        "team",
        "list",
        "--config",
        "templates.guide='{{ missing }}'",
    ]);
    run.failure()
        .stderr_has("--config #1")
        .stderr_has("unknown context field");
    assert!(run.stdout.is_empty());
    cli.run(&["team", "list", "--config-file", "missing.toml"])
        .failure()
        .stderr_has("missing.toml");
}

#[test]
fn config_check_preserves_dotenv_warnings_alongside_ignored_setting_notes() {
    let run = Cli::new()
        .env_remove("LINEAR_IGNORE_ENV_FILE")
        .file("cwd/.env", "LINEAR_VCS=git\nLINEAR_TEAM_ID=${TEAM}\n")
        .run(&["config", "check"]);
    run.success()
        .stderr_has("Warning: Ignoring LINEAR_TEAM_ID")
        .stderr_has("Note: LINEAR_VCS from .env file ");
    assert_eq!(run.stdout, "✓ Configuration is valid.\n");
}
