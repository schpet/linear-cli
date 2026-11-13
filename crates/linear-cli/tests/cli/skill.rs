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

const OLD_SKILL: &str = "---\nname: linear-cli\ndescription: Manage Linear issues from the \
                         command line using the linear cli. This skill allows automating \
                         linear management.\n---\n\n# Linear CLI\n";

const SHARED: &str = "home/.agents/skills/linear-cli/SKILL.md";
const CLAUDE: &str = "home/.claude/skills/linear-cli/SKILL.md";

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
    assert!(!cli.path("home/.claude").exists());
}
