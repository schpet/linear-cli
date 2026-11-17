//! `skill`: the agent guide built into the binary.
use crate::support::Cli;

#[test]
fn show_prints_the_guide_without_configuration_credentials_or_network() {
    let run = Cli::new()
        .file("cwd/.linear.toml", "this is not toml [")
        .env("HTTPS_PROXY", "http://proxy.example.invalid:3128")
        .env("SSL_CERT_FILE", "/missing/ca.pem")
        .env_remove("HOME")
        .run(&["skill", "show", "--builtin"]);
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

const OLD_SKILL: &str = "---\nname: linear-cli\ndescription: Manage Linear issues from the \
                         command line using the linear cli. This skill allows automating \
                         linear management.\n---\n\n# Linear CLI\n";

const SHARED: &str = "home/.agents/skills/linear-cli/SKILL.md";
const CLAUDE: &str = "home/.claude/skills/linear-cli/SKILL.md";
const CLAUDE_SETTINGS: &str = "home/.claude/settings.json";
const SESSION_START: &str = "linear skill session-start";
const CODEX_HOOKS: &str = "home/.codex/hooks.json";
const CODEX_SESSION_START: &str = "linear skill session-start --agent codex";

#[test]
fn an_edited_claude_hook_does_not_block_codex_installation() {
    let edited = serde_json::json!({"hooks":{"SessionStart":[{"hooks":[{
        "type":"command","command":SESSION_START,"timeout":60
    }]}]}})
    .to_string();
    let cli = with_claude()
        .file(CLAUDE_SETTINGS, &edited)
        .file(CODEX_HOOKS, "{}");
    let dry = cli.run(&["skill", "setup", "--dry-run", "--json"]);
    dry.failure();
    assert_eq!(dry.json()["hooks"][1]["change"], "created");
    let actual = cli.run(&["skill", "setup", "--json"]);
    actual.failure();
    assert_eq!(actual.json()["hooks"][1]["change"], "created");
    assert_eq!(cli.read(CLAUDE_SETTINGS), edited);
    assert!(cli.read(CODEX_HOOKS).contains(CODEX_SESSION_START));
}

#[test]
fn project_removal_explains_how_to_remove_the_codex_account_hook() {
    let cli = Cli::new();
    let run = cli.run(&["skill", "remove", "--project", "--dry-run"]);
    run.success()
        .stderr_has("linear skill remove --agent codex");
    assert!(!run.stderr.contains("linear skill setup"));
}

#[test]
fn hook_reports_accept_non_utf8_home_and_agent_paths() {
    use std::os::unix::ffi::OsStringExt;
    let cli = Cli::new();
    let path = cli
        .path("non-utf8")
        .join(std::ffi::OsString::from_vec(b"config-\xff".to_vec()));
    for variable in ["HOME", "CODEX_HOME", "CLAUDE_CONFIG_DIR"] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_linear"))
            .env_clear()
            .env("HOME", cli.path("home"))
            .env(variable, &path)
            .env("NO_COLOR", "1")
            .current_dir(cli.path("cwd"))
            .args(["skill", "status", "--json"])
            .output()
            .expect("status");
        assert!(
            output.status.success(),
            "{variable}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON report");
        assert!(
            value["hooks"]
                .as_array()
                .expect("hooks")
                .iter()
                .any(|hook| {
                    hook["path"]
                        .as_str()
                        .expect("path")
                        .starts_with(path.to_string_lossy().as_ref())
                })
        );
    }
}

#[test]
fn project_settings_cannot_escape_through_file_or_directory_symlinks() {
    for directory_link in [false, true] {
        for existing in [false, true] {
            if !directory_link && !existing {
                continue;
            }
            let cli = Cli::new();
            std::fs::create_dir_all(cli.path("outside")).expect("mkdir");
            if existing {
                std::fs::write(cli.path("outside/settings.json"), "{\"secret\":\"keep\"}")
                    .expect("write");
            }
            if directory_link {
                std::os::unix::fs::symlink(cli.path("outside"), cli.path("cwd/.claude"))
                    .expect("link dir");
            } else {
                std::fs::create_dir_all(cli.path("cwd/.claude")).expect("mkdir");
                std::os::unix::fs::symlink(
                    cli.path("outside/settings.json"),
                    cli.path("cwd/.claude/settings.json"),
                )
                .expect("link file");
            }
            cli.run(&["skill", "setup", "--project", "--agent", "claude"])
                .failure()
                .stderr_has("outside the project");
            assert!(!cli.path("cwd/.agents").exists());
            if existing {
                assert_eq!(cli.read("outside/settings.json"), "{\"secret\":\"keep\"}");
            } else {
                assert!(!cli.path("outside/settings.json").exists());
            }
        }
    }
}

#[test]
fn existing_settings_permissions_are_preserved_in_both_scopes() {
    use std::os::unix::fs::PermissionsExt;
    for project in [false, true] {
        for mode in [0o600, 0o640, 0o644] {
            let settings = if project {
                "cwd/.claude/settings.json"
            } else {
                CLAUDE_SETTINGS
            };
            let cli = Cli::new().file(settings, "{}");
            std::fs::set_permissions(cli.path(settings), std::fs::Permissions::from_mode(mode))
                .expect("chmod");
            let mut args = vec!["skill", "setup", "--agent", "claude"];
            if project {
                args.push("--project");
            }
            cli.run(&args).success();
            assert_eq!(
                std::fs::metadata(cli.path(settings))
                    .expect("stat")
                    .permissions()
                    .mode()
                    & 0o777,
                mode
            );
            args[1] = "remove";
            cli.run(&args).success();
            assert_eq!(
                std::fs::metadata(cli.path(settings))
                    .expect("stat")
                    .permissions()
                    .mode()
                    & 0o777,
                mode
            );
        }
    }
}

#[test]
fn dangling_project_configuration_directory_is_rejected_before_installation() {
    let cli = Cli::new();
    std::os::unix::fs::symlink(cli.path("absent/outside"), cli.path("cwd/.claude")).expect("link");
    cli.run(&["skill", "setup", "--project"])
        .failure()
        .stderr_has("Failed to resolve");
    assert!(!cli.path("cwd/.agents").exists());
    assert!(!cli.path("absent").exists());
}

#[test]
fn first_created_user_settings_are_private_and_project_settings_follow_umask() {
    use std::os::unix::fs::PermissionsExt;
    let cli = Cli::new().file("cwd/umask-probe", "");
    cli.run(&["skill", "setup", "--agent", "claude"]).success();
    assert_eq!(
        std::fs::metadata(cli.path(CLAUDE_SETTINGS))
            .expect("stat")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    cli.run(&["skill", "setup", "--project", "--agent", "claude"])
        .success();
    let mode = |path: &str| {
        std::fs::metadata(cli.path(path))
            .expect("stat")
            .permissions()
            .mode()
            & 0o777
    };
    assert_eq!(mode("cwd/.claude/settings.json"), mode("cwd/umask-probe"));
}

#[test]
fn codex_startup_uses_its_skill_syntax_and_needs_no_configuration() {
    let run = Cli::new()
        .env_remove("HOME")
        .env("LINEAR_TIMEOUT", "invalid")
        .file("cwd/.linear.toml", "not toml [")
        .run(&["skill", "session-start", "--agent", "codex"]);
    run.success()
        .stdout_has("$linear-cli")
        .stdout_has("linear skill show");
    assert!(!run.stdout.contains("/linear-cli"));
    assert_eq!(run.stderr, "", "{run}");
    Cli::new()
        .run(&["skill", "session-start", "--agent", "pi"])
        .usage_error();
}

#[test]
fn codex_hook_lifecycle_preserves_existing_hooks_and_settings() {
    let original = serde_json::json!({
        "description":"existing hooks", "hooks":{
            "SessionStart":[{"hooks":[{"type":"command","command":"other-tool context"}]}],
            "Stop":[{"hooks":[{"type":"command","command":"other-tool stop"}]}]
        }
    });
    let cli = Cli::new().file(CODEX_HOOKS, &original.to_string());
    let dry = cli.run(&["skill", "setup", "--agent", "codex", "--dry-run", "--json"]);
    dry.success();
    assert_eq!(dry.json()["hooks"][0]["agent"], "codex");
    assert_eq!(cli.read(CODEX_HOOKS), original.to_string());
    let setup = cli.run(&["skill", "setup", "--agent", "codex", "--json"]);
    setup.success().stderr_has("/hooks");
    assert_eq!(setup.json()["hooks"][0]["change"], "created");
    let installed: serde_json::Value = serde_json::from_str(&cli.read(CODEX_HOOKS)).expect("JSON");
    assert_eq!(installed["description"], original["description"]);
    assert_eq!(installed["hooks"]["Stop"], original["hooks"]["Stop"]);
    assert_eq!(
        installed["hooks"]["SessionStart"][0],
        original["hooks"]["SessionStart"][0]
    );
    assert_eq!(
        installed["hooks"]["SessionStart"][1]["hooks"][0]["command"],
        CODEX_SESSION_START
    );
    let bytes = cli.read(CODEX_HOOKS);
    cli.run(&["skill", "setup", "--agent", "codex"]).success();
    assert_eq!(cli.read(CODEX_HOOKS), bytes);
    let status = cli.run(&["skill", "status", "--json"]);
    status.success().stderr_has("/hooks");
    assert_eq!(status.json()["hooks"][1]["state"], "current");
    cli.run(&["skill", "remove", "--agent", "claude"]).success();
    assert_eq!(cli.read(CODEX_HOOKS), bytes);
    cli.run(&["skill", "remove", "--agent", "codex", "--dry-run"])
        .success();
    assert_eq!(cli.read(CODEX_HOOKS), bytes);
    cli.run(&["skill", "remove", "--agent", "codex"]).success();
    let removed: serde_json::Value = serde_json::from_str(&cli.read(CODEX_HOOKS)).expect("JSON");
    assert_eq!(removed, original);
    assert!(
        !cli.path("home/.codex/config.toml").exists(),
        "hook trust is left to Codex"
    );
}

#[test]
fn codex_hooks_follow_codex_home_detection_and_project_policy() {
    let cli = Cli::new();
    cli.run(&["skill", "setup"]).success();
    assert!(
        !cli.path(CODEX_HOOKS).exists(),
        "do not create an absent agent's configuration by default"
    );
    let moved = cli.path("elsewhere/codex");
    std::fs::create_dir_all(&moved).expect("mkdir");
    let cli = cli.env("CODEX_HOME", &moved.display().to_string());
    cli.run(&["skill", "setup"]).success();
    assert!(
        std::fs::read_to_string(moved.join("hooks.json"))
            .expect("hooks")
            .contains(CODEX_SESSION_START)
    );
    assert!(!cli.path(CODEX_HOOKS).exists());
    cli.run(&["skill", "setup", "--project", "--agent", "codex"])
        .success()
        .stderr_has("linear skill setup --agent codex");
    assert!(!cli.path("cwd/.codex/hooks.json").exists());
    assert!(cli.path("cwd/.agents/skills/linear-cli/SKILL.md").exists());
}

#[test]
fn invalid_or_edited_codex_hooks_are_not_replaced() {
    let cli = Cli::new().file(CODEX_HOOKS, "not JSON");
    cli.run(&["skill", "setup", "--agent", "codex", "--force"])
        .failure()
        .stderr_has("hooks.json");
    assert_eq!(cli.read(CODEX_HOOKS), "not JSON");
    assert!(!cli.path(SHARED).exists());
    let edited = serde_json::json!({"hooks":{"SessionStart":[{"hooks":[{
        "type":"command", "command":CODEX_SESSION_START, "timeout":60
    }]}]}})
    .to_string();
    let cli = Cli::new().file(CODEX_HOOKS, &edited);
    cli.run(&["skill", "setup", "--agent", "codex", "--force"])
        .failure()
        .stderr_has("edited");
    cli.run(&["skill", "remove", "--agent", "codex"]).success();
    assert_eq!(cli.read(CODEX_HOOKS), edited);
}

#[test]
fn session_start_prints_a_small_reminder_without_loading_configuration() {
    let run = Cli::new()
        .env_remove("HOME")
        .env("LINEAR_TIMEOUT", "invalid")
        .file("cwd/.linear.toml", "not toml [")
        .run(&["skill", "session-start"]);
    run.success()
        .stdout_has("/linear-cli")
        .stdout_has("linear skill show")
        .stdout_has("before your first `linear` command");
    assert!(run.stdout.len() < 600, "{run}");
    assert_eq!(run.stderr, "", "{run}");
}

#[test]
fn setup_manages_a_startup_hook_without_replacing_other_claude_settings() {
    let original = serde_json::json!({
        "model": "opus",
        "env": {"EXAMPLE": "preserve me"},
        "hooks": {
            "SessionStart": [{"matcher": "startup", "hooks": [{"type": "command", "command": "other-tool context"}]}],
            "Stop": [{"hooks": [{"type": "command", "command": "other-tool stop"}]}]
        }
    });
    let cli = with_claude().file(CLAUDE_SETTINGS, &original.to_string());
    let run = cli.run(&["skill", "setup", "--json"]);
    run.success();
    assert_eq!(run.json()["hooks"][0]["change"], "created");
    let installed: serde_json::Value =
        serde_json::from_str(&cli.read(CLAUDE_SETTINGS)).expect("JSON");
    assert_eq!(installed["model"], original["model"]);
    assert_eq!(installed["env"], original["env"]);
    assert_eq!(installed["hooks"]["Stop"], original["hooks"]["Stop"]);
    assert_eq!(
        installed["hooks"]["SessionStart"][0],
        original["hooks"]["SessionStart"][0]
    );
    assert_eq!(
        installed["hooks"]["SessionStart"][1]["hooks"][0]["command"],
        SESSION_START
    );
    let bytes = cli.read(CLAUDE_SETTINGS);
    cli.run(&["skill", "setup"]).success();
    assert_eq!(cli.read(CLAUDE_SETTINGS), bytes, "repeat setup is a no-op");
    let status = cli.run(&["skill", "status", "--json"]);
    status.success();
    assert_eq!(status.json()["hooks"][0]["state"], "current");

    cli.run(&["skill", "remove", "--dry-run"]).success();
    assert_eq!(cli.read(CLAUDE_SETTINGS), bytes);
    cli.run(&["skill", "remove"]).success();
    let removed: serde_json::Value =
        serde_json::from_str(&cli.read(CLAUDE_SETTINGS)).expect("JSON");
    assert_eq!(removed, original, "only linear's hook is removed");
}

#[test]
fn hooks_follow_agent_selection_scope_and_dry_runs() {
    let cli = with_claude();
    cli.run(&["skill", "setup", "--dry-run"]).success();
    assert!(!cli.path(CLAUDE_SETTINGS).exists());
    cli.run(&["skill", "setup", "--agent", "codex"]).success();
    assert!(!cli.path(CLAUDE_SETTINGS).exists());
    cli.run(&["skill", "setup", "--agent", "claude"]).success();
    let settings = cli.read(CLAUDE_SETTINGS);
    cli.run(&["skill", "remove", "--agent", "codex"]).success();
    assert_eq!(cli.read(CLAUDE_SETTINGS), settings);
    cli.run(&["skill", "setup", "--project", "--agent", "claude"])
        .success();
    assert!(
        cli.read("cwd/.claude/settings.json")
            .contains(SESSION_START)
    );
    assert_eq!(cli.read(CLAUDE_SETTINGS), settings);
}

#[test]
fn malformed_claude_settings_are_rejected_before_installing_anything() {
    for contents in [
        "not json",
        "[]",
        r#"{"hooks":null}"#,
        r#"{"hooks":{"SessionStart":{}}}"#,
        r#"{"hooks":{"SessionStart":[{"matcher":null,"hooks":[]}]}}"#,
        r#"{"hooks":{"SessionStart":[{"hooks":[false]}]}}"#,
    ] {
        let cli = with_claude().file(CLAUDE_SETTINGS, contents);
        cli.run(&["skill", "setup", "--force"])
            .failure()
            .stderr_has("settings.json");
        assert_eq!(cli.read(CLAUDE_SETTINGS), contents);
        assert!(!cli.path(SHARED).exists());
        let status = cli.run(&["skill", "status", "--json"]);
        status.failure().stderr_has("settings.json");
        assert!(
            !status.json()["items"]
                .as_array()
                .expect("skill items")
                .is_empty()
        );
        assert_eq!(status.json()["hooks"][0]["state"], "unreadable");
        cli.run(&["skill", "remove"])
            .failure()
            .stderr_has("settings.json");
        assert_eq!(cli.read(CLAUDE_SETTINGS), contents);
    }
}

#[test]
fn customized_linear_hooks_are_reported_and_kept() {
    let contents = serde_json::json!({"hooks":{"SessionStart":[{"hooks":[{
        "type":"command", "command":SESSION_START, "timeout":60
    }]}]}})
    .to_string();
    let cli = with_claude().file(CLAUDE_SETTINGS, &contents);
    cli.run(&["skill", "setup", "--force"])
        .failure()
        .stderr_has("edited");
    assert_eq!(cli.read(CLAUDE_SETTINGS), contents);
    cli.run(&["skill", "remove"]).success();
    assert_eq!(cli.read(CLAUDE_SETTINGS), contents);
}

#[test]
fn removing_the_only_hook_leaves_no_empty_registration() {
    let cli = with_claude();
    cli.run(&["skill", "setup"]).success();
    cli.run(&["skill", "remove"]).success();
    let settings: serde_json::Value =
        serde_json::from_str(&cli.read(CLAUDE_SETTINGS)).expect("JSON");
    assert_eq!(settings, serde_json::json!({}));
}

#[test]
fn removal_preserves_other_commands_in_the_same_group_and_empty_groups() {
    let cli = with_claude();
    cli.run(&["skill", "setup"]).success();
    let mut settings: serde_json::Value =
        serde_json::from_str(&cli.read(CLAUDE_SETTINGS)).expect("JSON");
    let other = serde_json::json!({"type":"command", "command":"other-tool context"});
    settings["hooks"]["SessionStart"][0]["hooks"]
        .as_array_mut()
        .expect("array")
        .push(other.clone());
    settings["hooks"]["SessionStart"]
        .as_array_mut()
        .expect("array")
        .push(serde_json::json!({"hooks":[]}));
    let cli = cli.file(CLAUDE_SETTINGS, &settings.to_string());
    cli.run(&["skill", "remove"]).success();
    let removed: serde_json::Value =
        serde_json::from_str(&cli.read(CLAUDE_SETTINGS)).expect("JSON");
    assert_eq!(
        removed,
        serde_json::json!({"hooks":{"SessionStart":[{"hooks":[other]},{"hooks":[]}]}})
    );
}

#[cfg(unix)]
#[test]
fn settings_symlinks_are_preserved_when_managing_the_hook() {
    let cli = with_claude().file("dotfiles/claude.json", r#"{"model":"opus"}"#);
    std::os::unix::fs::symlink(cli.path("dotfiles/claude.json"), cli.path(CLAUDE_SETTINGS))
        .expect("symlink");
    cli.run(&["skill", "setup"]).success();
    assert!(
        std::fs::symlink_metadata(cli.path(CLAUDE_SETTINGS))
            .expect("stat")
            .is_symlink()
    );
    assert!(cli.read("dotfiles/claude.json").contains(SESSION_START));
    cli.run(&["skill", "remove"]).success();
    let settings: serde_json::Value =
        serde_json::from_str(&cli.read("dotfiles/claude.json")).expect("JSON");
    assert_eq!(settings, serde_json::json!({"model":"opus"}));
}

/// A sandbox in which Claude Code looks set up.
fn with_claude() -> Cli {
    let cli = Cli::new();
    std::fs::create_dir_all(cli.path("home/.claude")).expect("create");
    cli
}

#[test]
fn setup_installs_a_stub_that_points_at_the_guide() {
    let cli = with_claude().stub_bin("linear", "exit 0");
    let run = cli.run(&["skill", "setup"]);
    run.success()
        .stdout_has("~/.agents/skills/linear-cli  created  Codex, OpenCode, Pi")
        .stdout_has("~/.claude/skills/linear-cli  created  Claude Code");
    assert_eq!(run.stderr, "", "{run}");

    let stub = cli.read(SHARED);
    assert!(stub.starts_with("---\nname: linear-cli\n"), "{stub}");
    assert!(stub.contains("Run `linear skill show`"), "{stub}");
    assert_eq!(cli.read(CLAUDE), stub, "every agent gets the same file");

    let again = cli.run(&["skill", "setup"]);
    again
        .success()
        .stdout_has("~/.agents/skills/linear-cli  unchanged  ")
        .stdout_has("~/.claude/skills/linear-cli  unchanged  ");
    cli.run(&["skill", "status"])
        .success()
        .stdout_has("~/.agents/skills/linear-cli  current  ")
        .stdout_has("~/.claude/skills/linear-cli  current  ");
}

#[test]
fn setup_leaves_claude_code_alone_on_a_machine_without_it() {
    let cli = Cli::new();
    let run = cli.run(&["skill", "setup"]);
    run.success()
        .stdout_has("~/.agents/skills/linear-cli  created")
        .stderr_has("Claude Code's directory was not found")
        .stderr_has("linear is not on PATH");
    assert!(!cli.path("home/.claude").exists(), "{run}");

    cli.run(&["skill", "setup", "--agent", "claude"]).success();
    assert!(cli.path(CLAUDE).exists(), "naming it installs it");
}

#[test]
fn a_dry_run_creates_nothing() {
    let cli = with_claude();
    for args in [
        &["skill", "setup", "--dry-run"][..],
        &["skill", "setup", "--dry-run", "--force"],
        &["skill", "setup", "--dry-run", "--project"],
    ] {
        let run = cli.run(args);
        run.success()
            .stdout_has("would create")
            .stderr_has("Dry run: nothing was changed.");
    }
    assert!(!cli.path("home/.agents").exists());
    assert!(!cli.path("home/.claude/skills").exists());
    assert!(!cli.path("cwd/.agents").exists());
}

#[test]
fn an_old_copy_of_the_skill_is_kept_until_force_moves_it_to_a_backup() {
    let cli = with_claude().file(CLAUDE, OLD_SKILL).file(
        "home/.claude/skills/linear-cli/scripts/mine.sh",
        "echo mine",
    );
    let run = cli.run(&["skill", "setup"]);
    run.failure()
        .stdout_has("~/.agents/skills/linear-cli  created ")
        .stdout_has("~/.claude/skills/linear-cli  conflict")
        .stdout_has("the linear-cli skill from before")
        .stderr_has("1 linear-cli skill was left alone")
        .stderr_has("--force");
    assert_eq!(cli.read(CLAUDE), OLD_SKILL);
    assert!(cli.path(SHARED).exists(), "the other directory is set up");

    cli.run(&["skill", "status"])
        .success()
        .stdout_has("~/.claude/skills/linear-cli  foreign");

    let run = cli.run(&["skill", "setup", "--force", "--json"]);
    run.success();
    let report = run.json();
    assert_eq!(report["action"], "setup");
    assert_eq!(report["dryRun"], false);
    let step = &report["steps"][1];
    assert_eq!(step["change"], "replaced");
    assert_eq!(step["state"], "foreign");
    assert_eq!(step["agents"], serde_json::json!(["claude"]));
    let backup = step["backupPath"].as_str().expect("a backup path");
    assert!(
        backup.starts_with(&format!(
            "{}/",
            cli.path("home/.claude/linear-cli-skill-backups").display()
        )),
        "{backup}"
    );
    let backup = std::path::Path::new(backup);
    assert_eq!(
        std::fs::read_to_string(backup.join("SKILL.md")).expect("read"),
        OLD_SKILL
    );
    assert!(backup.join("scripts/mine.sh").exists());
    assert_eq!(cli.read(CLAUDE), cli.read(SHARED));
}

#[test]
fn a_copy_an_agent_reads_first_is_reported_as_shadowing() {
    let cli = Cli::new().file("home/.pi/agent/skills/linear-cli/SKILL.md", OLD_SKILL);
    cli.run(&["skill", "setup"])
        .failure()
        .stdout_has("~/.pi/agent/skills/linear-cli  conflict")
        .stdout_has("Pi reads this copy as well as or instead of the installed skill");
    let status = cli.run(&["skill", "status", "--json"]);
    status.success();
    let items = status.json()["items"].clone();
    assert_eq!(items[2]["location"], "alternate");
    assert_eq!(items[2]["state"], "foreign");
    assert_eq!(items[0]["state"], "current");
    assert_eq!(
        items[1]["state"], "missing",
        "Claude Code's directory is listed though it is not set up"
    );
}

#[cfg(unix)]
#[test]
fn a_symlinked_skill_is_not_followed() {
    let cli = Cli::new().file("elsewhere/linear-cli/SKILL.md", OLD_SKILL);
    std::fs::create_dir_all(cli.path("home/.agents/skills")).expect("create");
    std::os::unix::fs::symlink(
        cli.path("elsewhere/linear-cli"),
        cli.path("home/.agents/skills/linear-cli"),
    )
    .expect("symlink");

    cli.run(&["skill", "setup"])
        .failure()
        .stdout_has("a symlink to a linear-cli skill kept elsewhere");
    cli.run(&["skill", "setup", "--force"]).success();
    assert_eq!(
        cli.read("elsewhere/linear-cli/SKILL.md"),
        OLD_SKILL,
        "the link moved; its target was not touched"
    );
    assert!(cli.read(SHARED).contains("linear skill show"));
}

#[test]
fn remove_deletes_what_setup_installed_and_nothing_else() {
    let cli = with_claude().file(CLAUDE, OLD_SKILL);
    cli.run(&["skill", "setup"]).failure();
    std::fs::write(cli.path("home/.agents/skills/linear-cli/notes.md"), "mine").expect("write");

    let dry = cli.run(&["skill", "remove", "--dry-run"]);
    dry.success().stdout_has("would remove");
    assert!(cli.path(SHARED).exists());

    let run = cli.run(&["skill", "remove"]);
    run.success()
        .stdout_has("~/.agents/skills/linear-cli  removed")
        .stdout_has("the directory holds other files and was kept")
        .stdout_has("~/.claude/skills/linear-cli  kept");
    assert!(!cli.path(SHARED).exists());
    assert!(cli.path("home/.agents/skills/linear-cli/notes.md").exists());
    assert_eq!(cli.read(CLAUDE), OLD_SKILL);

    // The directory that kept the user's file is theirs now.
    cli.run(&["skill", "remove", "--agent", "codex"])
        .success()
        .stdout_has("~/.agents/skills/linear-cli  kept")
        .stdout_has("a directory with no SKILL.md");
    Cli::new()
        .run(&["skill", "remove"])
        .success()
        .stdout_has("~/.agents/skills/linear-cli  not installed");
}

#[test]
fn a_project_is_set_up_at_its_root_for_every_agent() {
    let cli = Cli::new().cwd("cwd/packages/app");
    std::fs::create_dir_all(cli.path("cwd/.git")).expect("create");
    let run = cli.run(&["skill", "setup", "--project", "--json"]);
    run.success()
        .stderr_has("Pi reads a project's skills only after the project is trusted");
    let report = run.json();
    assert_eq!(report["scope"], "project");
    assert_eq!(
        report["root"],
        cli.path("cwd").display().to_string(),
        "{run}"
    );
    assert!(cli.path("cwd/.agents/skills/linear-cli/SKILL.md").exists());
    assert!(
        cli.path("cwd/.claude/skills/linear-cli/SKILL.md").exists(),
        "teammates may use Claude Code though it is not set up here"
    );
    assert!(!cli.path("home/.agents").exists());
    cli.run(&["skill", "status", "--project"])
        .success()
        .stdout_has("/cwd/.agents/skills/linear-cli  current");
}

#[test]
fn outside_a_repository_the_project_is_the_working_directory() {
    let cli = Cli::new().cwd("cwd/scratch");
    cli.run(&["skill", "setup", "--project", "--agent", "codex"])
        .success();
    assert!(
        cli.path("cwd/scratch/.agents/skills/linear-cli/SKILL.md")
            .exists()
    );
    assert!(!cli.path("cwd/scratch/.claude").exists());
}

#[test]
fn a_copy_nested_in_the_project_is_reported_and_never_moved() {
    let nested = "cwd/packages/app/.opencode/skills/linear-cli/SKILL.md";
    let cli = Cli::new().cwd("cwd/packages/app").file(nested, OLD_SKILL);
    std::fs::create_dir_all(cli.path("cwd/.git")).expect("create");
    for args in [
        &["skill", "setup", "--project"][..],
        &["skill", "setup", "--project", "--force"],
    ] {
        let run = cli.run(args);
        run.failure()
            .stdout_has("packages/app/.opencode/skills/linear-cli  conflict")
            .stdout_has("agents started in this directory read it")
            .stderr_has("Move or remove them by hand");
        assert_eq!(cli.read(nested), OLD_SKILL);
    }
    assert!(cli.path("cwd/.agents/skills/linear-cli/SKILL.md").exists());
}

#[test]
fn skill_commands_reject_unknown_agents_and_relative_directories() {
    let cli = Cli::new();
    cli.run(&["skill", "setup", "--agent", "cursor"])
        .usage_error()
        .stderr_has("claude, codex, opencode, pi");
    cli.run(&["skill", "status", "--agent", "claude"])
        .usage_error();
    let relative = Cli::new().env("CLAUDE_CONFIG_DIR", "claude");
    relative
        .run(&["skill", "setup"])
        .usage_error()
        .stderr_has("CLAUDE_CONFIG_DIR is not an absolute path");
    assert!(!relative.path("home/.agents").exists());
    assert!(!relative.path("cwd/claude").exists());
}

#[test]
fn claude_code_s_directory_follows_its_configuration_variable() {
    let cli = Cli::new();
    let moved = cli.path("elsewhere/claude");
    std::fs::create_dir_all(&moved).expect("create");
    let cli = cli.env("CLAUDE_CONFIG_DIR", &moved.display().to_string());
    cli.run(&["skill", "setup"]).success();
    assert!(moved.join("skills/linear-cli/SKILL.md").exists());
    assert!(
        std::fs::read_to_string(moved.join("settings.json"))
            .expect("settings")
            .contains(SESSION_START)
    );
    assert!(!cli.path("home/.claude").exists());
}

#[test]
fn normal_show_is_offline_but_bad_selected_config_requires_builtin_recovery() {
    let cli = Cli::new()
        .env_remove("HOME")
        .env("LINEAR_GRAPHQL_ENDPOINT", "not a url")
        .env("LINEAR_TIMEOUT", "not a number")
        .env("HTTPS_PROXY", "not a proxy")
        .file("cwd/.env", "not dotenv [");
    cli.run(&["skill", "show"])
        .success()
        .stdout_has("# Linear CLI");
    let cli = cli.file("cwd/.linear.toml", "this is not toml [");
    let run = cli.run(&["skill", "show"]);
    run.failure()
        .stderr_has(".linear.toml")
        .stderr_has("--builtin");
    assert!(run.stdout.is_empty(), "{run}");
    cli.run(&[
        "skill",
        "show",
        "--builtin",
        "--config-file",
        "absent.toml",
        "--config",
        "bad",
    ])
    .success()
    .stdout_has("# Linear CLI");
}

#[test]
fn full_and_targeted_guides_merge_preferences_and_inherit_ancestors_once() {
    let cli = Cli::new()
        .file(
            "home/.config/linear/linear.toml",
            r#"
[templates]
guide_how_to_work = '{% include "builtin/guide_how_to_work" %}GLOBAL BASICS'
skill_issue_comment = 'COMMENT POLICY'
skill_issue_comment_update = 'UPDATE SIBLING'
skill_milestone_update = 'NO PROSE COMMAND POLICY'
"#,
        )
        .file(
            "cwd/.linear.toml",
            r#"
[templates]
skill_issue_comment_add = '{% include "builtin/skill_issue_comment_add" %}PROJECT ADD'
skill_issue_delete = 'DELETE SIBLING'
"#,
        );
    let full = cli.run(&["skill", "show"]);
    full.success()
        .stdout_has("COMMENT POLICY")
        .stdout_has("PROJECT ADD")
        .stdout_has("UPDATE SIBLING")
        .stdout_has("NO PROSE COMMAND POLICY");
    assert_eq!(full.stdout.matches("COMMENT POLICY").count(), 1);
    let target = cli.run(&["skill", "show", "i", "comment", "add"]);
    target
        .success()
        .stdout_has("GLOBAL BASICS")
        .stdout_has("COMMENT POLICY")
        .stdout_has("PROJECT ADD")
        .stdout_has("Usage: linear issue comment add");
    assert!(!target.stdout.contains("UPDATE SIBLING"));
    assert!(!target.stdout.contains("DELETE SIBLING"));
    assert_eq!(target.stdout.matches("COMMENT POLICY").count(), 1);
}

#[test]
fn target_aliases_canonicalize_and_unknown_or_hidden_commands_fail() {
    let cli = Cli::new().file(
        "cwd/.linear.toml",
        "[templates]\nskill_issue_gh_pull-request='PR POLICY'\n",
    );
    let run = cli.run(&["skill", "show", "issue", "gh", "pr"]);
    run.success()
        .stdout_has("PR POLICY")
        .stdout_has("Usage: linear issue gh pull-request");
    for path in [
        &["skill", "show", "unknown"][..],
        &["skill", "show", "issue", "bad"],
    ] {
        let run = cli.run(path);
        run.usage_error()
            .stderr_has("Unknown skill command path")
            .stderr_has("--help");
        assert!(run.stdout.is_empty());
    }
    // This command remains available for migration errors, but is hidden.
    cli.run(&["skill", "show", "issue", "list"]).usage_error();
}

#[test]
fn empty_hooks_disappear_and_custom_text_and_inheritance_are_preserved() {
    let cli = Cli::new().file("cwd/.linear.toml", r#"
[templates]
skill_issue_comment = ''
skill_milestone_update = '{% extends "builtin/skill_milestone_update" %}{% block content %}{{ super() }}MILESTONE POLICY{% endblock %}'
guide_how_to_work = '{% extends "builtin/guide_how_to_work" %}{% block content %}{{ super() }}ADDED RULE{% endblock %}'
"#);
    for args in [
        &["skill", "show"][..],
        &["skill", "show", "milestone", "update"],
    ] {
        let run = cli.run(args);
        run.success()
            .stdout_has("ADDED RULE")
            .stdout_has("MILESTONE POLICY");
        assert!(!run.stdout.contains("### Resolve or reopen comment threads"));
        assert_eq!(run.stdout.matches("ADDED RULE").count(), 1);
        assert_eq!(run.stdout.matches("MILESTONE POLICY").count(), 1);
    }
}

#[test]
fn overlays_apply_files_before_snippets_and_fail_without_partial_stdout() {
    let cli = Cli::new()
        .file("cwd/one.toml", "[templates]\nguide_intro='FILE ONE'\n")
        .file("cwd/two.toml", "[templates]\nguide_intro='FILE TWO'\n");
    let run = cli.run(&[
        "--config",
        "templates.guide_intro='FIRST SNIPPET'",
        "skill",
        "show",
        "--config-file",
        "one.toml",
        "--config",
        "templates.guide_intro='LAST SNIPPET'",
        "--config-file",
        "two.toml",
    ]);
    run.success().stdout_has("LAST SNIPPET");
    assert!(!run.stdout.contains("FILE ONE"));
    assert!(!run.stdout.contains("FILE TWO"));
    let run = cli.run(&[
        "skill",
        "show",
        "--config-file",
        "one.toml",
        "--config-file",
        "two.toml",
    ]);
    run.success().stdout_has("FILE TWO");
    for flags in [
        vec!["--config-file", "missing.toml"],
        vec!["--config", "team_id='ENG'"],
        vec![
            "--config",
            "templates.guide_intro='{% include \"missing\" %}'",
        ],
        vec!["--config", "templates.guide_intro='{{ version.missing }}'"],
    ] {
        let mut args = vec!["skill", "show"];
        args.extend(flags);
        let run = cli.run(&args);
        run.failure().stderr_has("--builtin");
        assert!(run.stdout.is_empty(), "{run}");
    }
    let cli = cli.file("cwd/scalar.toml", "team_id='ENG'");
    cli.run(&["skill", "show", "--config-file", "scalar.toml"])
        .failure()
        .stderr_has("only templates and template-aliases");
}

#[test]
fn targeting_a_parent_includes_its_subtree_and_shared_partials_merge_by_name() {
    let cli = Cli::new()
        .file(
            "home/.config/linear/linear.toml",
            r#"
[templates]
skill_issue_comment = '{% include "policy" %}'
skill_issue_comment_add = 'ADD POLICY'
skill_issue_comment_update = 'UPDATE POLICY'
skill_issue_comment_delete = 'DELETE POLICY'
skill_completions = 'SHELL POLICY'
[template-aliases]
policy = '{% include "approval" %} {% include "attribution" %}'
approval = 'GLOBAL APPROVAL'
attribution = 'GLOBAL ATTRIBUTION'
"#,
        )
        .file(
            "cwd/.linear.toml",
            "[template-aliases]\napproval = 'PROJECT APPROVAL'\n",
        );
    let run = cli.run(&["skill", "show", "issue", "comment"]);
    run.success()
        .stdout_has("PROJECT APPROVAL GLOBAL ATTRIBUTION")
        .stdout_has("ADD POLICY")
        .stdout_has("UPDATE POLICY")
        .stdout_has("DELETE POLICY");
    for text in [
        "ADD POLICY",
        "UPDATE POLICY",
        "DELETE POLICY",
        "PROJECT APPROVAL",
    ] {
        assert_eq!(run.stdout.matches(text).count(), 1, "{run}");
    }
    cli.run(&["skill", "show"])
        .success()
        .stdout_has("SHELL POLICY");
}

#[test]
fn command_hooks_preserve_long_user_preferences_verbatim() {
    let policy = format!("{}END OF POLICY", "full text\n".repeat(800));
    let overlay = format!(
        "templates.skill_milestone_update={}",
        toml::Value::String(policy.clone())
    );
    let run = Cli::new().run(&["skill", "show", "milestone", "update", "--config", &overlay]);
    run.success().stdout_has(&policy);
}

#[test]
fn installation_commands_keep_bypassing_all_template_configuration() {
    let cli = Cli::new().file("cwd/.linear.toml", "bad [");
    for command in ["setup", "status", "remove"] {
        cli.run(&[
            "skill",
            command,
            "--config-file",
            "missing.toml",
            "--config",
            "bad",
        ])
        .success();
    }
}

#[test]
fn command_hook_names_keep_hyphens_and_separate_nested_commands() {
    let cli = Cli::new().file(
        "cwd/.linear.toml",
        r#"[templates]
skill_project_update = 'EDIT PROJECT POLICY'
skill_project-update = 'PROJECT STATUS POLICY'
skill_initiative_update = 'EDIT INITIATIVE POLICY'
skill_initiative-update = 'INITIATIVE STATUS POLICY'
"#,
    );
    let full = cli.run(&["skill", "show"]);
    full.success();
    for (path, own, other) in [
        (
            vec!["project", "update"],
            "EDIT PROJECT POLICY",
            "PROJECT STATUS POLICY",
        ),
        (
            vec!["project-update"],
            "PROJECT STATUS POLICY",
            "EDIT PROJECT POLICY",
        ),
        (
            vec!["initiative", "update"],
            "EDIT INITIATIVE POLICY",
            "INITIATIVE STATUS POLICY",
        ),
        (
            vec!["initiative-update"],
            "INITIATIVE STATUS POLICY",
            "EDIT INITIATIVE POLICY",
        ),
    ] {
        assert_eq!(full.stdout.matches(own).count(), 1, "{full}");
        let mut args = vec!["skill", "show"];
        args.extend(path);
        let target = cli.run(&args);
        target.success().stdout_has(own);
        assert!(!target.stdout.contains(other), "{target}");
    }
}

#[test]
fn hyphenated_hooks_work_in_inline_config_and_inspection() {
    let value = "templates.skill_issue_gh_pull-request='HYPHENATED PR POLICY'";
    let cli = Cli::new();
    cli.run(&["--config", value, "skill", "show", "issue", "gh", "pr"])
        .success()
        .stdout_has("HYPHENATED PR POLICY");
    let inspected = cli.run(&[
        "--config",
        value,
        "config",
        "get",
        "templates.skill_issue_gh_pull-request",
    ]);
    inspected.success();
    assert_eq!(inspected.stdout, "HYPHENATED PR POLICY");
}
