//! Issue commands that read the current branch or drive git, jj and gh.
use serde_json::{Value, json};

use crate::support::{Cli, MockLinear};

const URL: &str = "https://linear.app/acme/issue/ENG-7/repair-the-widget";

fn details() -> Value {
    let last = json!({ "hasNextPage": false, "endCursor": null });
    json!({ "issue": {
        "identifier": "ENG-7", "title": "Repair the widget", "description": null,
        "url": URL, "branchName": "eng-7-repair-the-widget",
        "state": { "name": "Todo", "type": "unstarted", "color": "#123456" }, "assignee": null, "delegate": null, "priority": 2,
        "project": null, "projectMilestone": null, "cycle": null,
        "team": { "key": "ENG", "activeCycle": null }, "labels": { "nodes": [], "pageInfo": last }, "parent": null,
        "children": { "nodes": [], "pageInfo": last },
        "attachments": { "nodes": [], "pageInfo": last },
        "documents": { "nodes": [], "pageInfo": last },
        "creator": null, "createdAt": "2024-01-01T00:00:00.000Z", "updatedAt": "2024-01-01T00:00:00.000Z"
    } })
}

fn states() -> Value {
    json!({ "team": { "states": { "nodes": [
        { "id": "state-todo", "name": "Todo", "type": "unstarted", "position": 1 },
        { "id": "state-review", "name": "In Review", "type": "started", "position": 3 },
        { "id": "state-progress", "name": "In Progress", "type": "started", "position": 2 },
    ], "pageInfo": { "hasNextPage": false, "endCursor": null } } } })
}

/// A git on the branch `main`, where no other branch exists yet. `HEAD` is
/// commit 0123456789ab and `origin/main` fedcba987654.
fn git(cli: Cli) -> Cli {
    cli.stub_bin(
        "git",
        "case \"$1 $2\" in\n\
         'show-ref --verify') exit 1 ;;\n\
         'symbolic-ref --quiet') echo main ;;\n\
         'rev-parse --verify') case \"$6\" in\n\
           'HEAD^{commit}') echo 0123456789ab ;;\n\
           'origin/main^{commit}') echo fedcba987654 ;;\n\
           *) exit 1 ;;\n\
           esac ;;\n\
         'checkout -b') echo \"Switched to a new branch '$3'\" >&2 ;;\n\
         *) exit 1 ;;\n\
         esac",
    )
}

/// The change `@` is before `issue jj start`, and the one `jj new` makes.
const CHANGE: &str = "qpvuntsmzwxyqpvuntsmzwxyqpvuntsm";
const NEW_CHANGE: &str = "zyxwvutsrqponmlkzyxwvutsrqponmlk";

/// One record of the `jj log -r @` probe of `issue jj start`.
fn jj_at(change: &str, description: &str, empty: bool) -> String {
    json!([{ "change_id": change, "description": description, "commit_id": "c0ffee" }, empty])
        .to_string()
}

/// A jj whose `@` (`CHANGE`) already has a description, so `issue jj start`
/// must make a new change (`NEW_CHANGE`).
fn jj(cli: Cli) -> Cli {
    jj_starting_on(cli, &jj_at(CHANGE, "earlier work\n", false), "")
}

/// A jj whose `@` probe answers `at` until `jj new` runs, then the new
/// empty change. `describe` runs `on_describe`.
fn jj_starting_on(cli: Cli, at: &str, on_describe: &str) -> Cli {
    let created = jj_at(NEW_CHANGE, "", true);
    cli.file("cwd/.jj/repo/store", "").stub_bin(
        "jj",
        &format!(
            "case \"$*\" in\n\
             'log -r @ --no-graph '*) if [ -e made-new ]; then printf '%s\\n' '{created}'; else printf '%s\\n' '{at}'; fi ;;\n\
             new) touch made-new ;;\n\
             'describe '*) {on_describe} ;;\n\
             *) echo \"unexpected jj $*\" >&2; exit 1 ;;\n\
             esac"
        ),
    )
}

/// Calls of `tool` other than the startup repository-root probe.
fn calls(cli: &Cli, tool: &str) -> Vec<Vec<String>> {
    cli.calls(tool)
        .into_iter()
        .filter(|args| args[..] != ["rev-parse", "--show-toplevel"])
        .collect()
}

#[test]
fn describe_prints_the_title_and_trailer() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details())
        .on("GetIssueDetails", details());
    let cli = Cli::for_api(&api);
    cli.run(&["issue", "describe", "eng-7"])
        .success()
        .stdout_has("ENG-7 Repair the widget")
        .stdout_has("Linear-issue: Fixes ENG-7")
        .stdout_has(&format!("Linear-issue-url: {URL}"));
    cli.run(&["issue", "describe", "ENG-7", "-r"])
        .success()
        .stdout_has("Linear-issue: References ENG-7");
    assert!(
        api.requests()
            .iter()
            .all(|request| request.variables == json!({ "id": "ENG-7" }))
    );
}

#[test]
fn git_start_creates_the_issue_branch_and_marks_it_started() {
    let api = MockLinear::start();
    api.on("GetIssueTransitionDetails", details())
        .on("GetWorkflowStates", states())
        .on(
            "UpdateIssueState",
            json!({ "issueUpdate": { "success": true } }),
        );
    let cli = git(Cli::for_api(&api));
    let run = cli.run(&["issue", "git", "start", "ENG-7"]);
    run.success();
    assert_eq!(
        run.stdout,
        "✓ Created and switched to git branch \"eng-7-repair-the-widget\" from \"HEAD\" (0123456789ab); previous branch: \"main\"\n\
         ✓ Moved Linear issue ENG-7 from \"Todo\" to \"In Progress\"\n"
    );
    assert!(
        calls(&cli, "git").contains(&vec![
            "checkout".to_owned(),
            "-b".to_owned(),
            "eng-7-repair-the-widget".to_owned(),
            "HEAD".to_owned(),
        ]),
        "{:?}",
        cli.calls("git")
    );
    assert!(calls(&cli, "git").contains(&vec![
        "show-ref".to_owned(),
        "--verify".to_owned(),
        "--quiet".to_owned(),
        "refs/heads/eng-7-repair-the-widget".to_owned(),
    ]));
    assert_eq!(
        api.variables("GetIssueTransitionDetails"),
        json!({ "id": "ENG-7" })
    );
    assert_eq!(
        api.variables("GetWorkflowStates"),
        json!({ "teamKey": "ENG", "first": 100 })
    );
    assert_eq!(
        api.variables("UpdateIssueState"),
        json!({ "issueId": "ENG-7", "stateId": "state-progress" })
    );
}

#[test]
fn git_start_accepts_a_custom_branch_and_base_ref() {
    let api = MockLinear::start();
    api.on("GetIssueTransitionDetails", details())
        .on("GetWorkflowStates", states())
        .on(
            "UpdateIssueState",
            json!({ "issueUpdate": { "success": true } }),
        );
    let cli = git(Cli::for_api(&api).env("LINEAR_TEAM_ID", "ENG"));
    let run = cli.run(&[
        "issue",
        "git",
        "start",
        "7",
        "--branch",
        "my-branch",
        "--from-ref",
        "origin/main",
    ]);
    run.success().stdout_has(
        "✓ Created and switched to git branch \"my-branch\" from \"origin/main\" (fedcba987654); previous branch: \"main\"\n",
    );
    assert!(
        calls(&cli, "git").contains(&vec![
            "rev-parse".to_owned(),
            "--verify".to_owned(),
            "--quiet".to_owned(),
            "--short=12".to_owned(),
            "--end-of-options".to_owned(),
            "origin/main^{commit}".to_owned(),
        ]),
        "{:?}",
        cli.calls("git")
    );
    let checkout = calls(&cli, "git")
        .into_iter()
        .find(|args| args[0] == "checkout")
        .expect("git checkout ran");
    assert_eq!(checkout, ["checkout", "-b", "my-branch", "origin/main"]);
}

#[test]
fn git_start_fails_after_preparing_the_branch_when_the_state_update_fails() {
    let api = MockLinear::start();
    api.on("GetIssueTransitionDetails", details())
        .on_error("GetWorkflowStates", "Team not found");
    let cli = git(Cli::for_api(&api));
    let run = cli.run(&["issue", "git", "start", "ENG-7"]);
    run.failure()
        .stderr_has("Could not move the issue to a started status")
        .stderr_has("Team not found")
        .stderr_has("The branch is ready");
    // What was done stays reported, and no Linear result is claimed.
    assert_eq!(
        run.stdout,
        "✓ Created and switched to git branch \"eng-7-repair-the-widget\" from \"HEAD\" (0123456789ab); previous branch: \"main\"\n"
    );
    assert!(calls(&cli, "git").iter().any(|args| args[0] == "checkout"));
}

#[test]
fn jj_start_fails_when_linear_does_not_update_the_state() {
    let api = MockLinear::start();
    api.on("GetIssueTransitionDetails", details())
        .on("GetWorkflowStates", states())
        .on(
            "UpdateIssueState",
            json!({ "issueUpdate": { "success": false } }),
        );
    let cli = jj(Cli::for_api(&api));
    cli.run(&["issue", "jj", "start", "ENG-7"])
        .failure()
        .stderr_has("Linear did not update the issue")
        .stderr_has("The jj change is ready");
}

#[test]
fn git_start_uses_the_workflow_states_of_the_issue_team() {
    let api = MockLinear::start();
    api.on("GetIssueTransitionDetails", details())
        .on("GetWorkflowStates", states())
        .on(
            "UpdateIssueState",
            json!({ "issueUpdate": { "success": true } }),
        );
    let cli = git(Cli::for_api(&api).env("LINEAR_TEAM_ID", "OPS"));
    cli.run(&["issue", "git", "start", "ENG-7"]).success();
    assert_eq!(
        api.variables("GetWorkflowStates"),
        json!({ "teamKey": "ENG", "first": 100 })
    );
}

#[test]
fn jj_start_makes_a_new_described_change() {
    let api = MockLinear::start();
    api.on("GetIssueTransitionDetails", details())
        .on("GetWorkflowStates", states())
        .on(
            "UpdateIssueState",
            json!({ "issueUpdate": { "success": true } }),
        );
    let cli = jj(Cli::for_api(&api));
    cli.run(&["issue", "jj", "start", "ENG-7"]).success();
    let jj = calls(&cli, "jj");
    assert!(jj.contains(&vec!["new".to_owned()]), "{jj:?}");
    let describe = jj
        .iter()
        .find(|args| args[0] == "describe")
        .expect("jj describe ran");
    assert_eq!(describe[1], "-m");
    assert!(describe[2].starts_with("ENG-7 Repair the widget\n"));
    assert!(describe[2].contains("Linear-issue: Fixes ENG-7"));
    assert!(calls(&cli, "git").is_empty());
    assert_eq!(
        api.variables("UpdateIssueState"),
        json!({ "issueId": "ENG-7", "stateId": "state-progress" })
    );
}

#[test]
fn git_and_jj_start_prepare_the_working_copy_of_a_started_or_closed_issue_and_keep_its_status() {
    for (name, state_type, said) in [
        (
            "In Review",
            "started",
            "✓ Left Linear issue ENG-7 in \"In Review\" (already started)\n",
        ),
        (
            "Done",
            "completed",
            "✓ Left Linear issue ENG-7 in \"Done\" (closed; not reopened)\n",
        ),
        (
            "Canceled",
            "canceled",
            "✓ Left Linear issue ENG-7 in \"Canceled\" (closed; not reopened)\n",
        ),
        (
            "Duplicate",
            "duplicate",
            "✓ Left Linear issue ENG-7 in \"Duplicate\" (closed; not reopened)\n",
        ),
    ] {
        let mut issue = details();
        issue["issue"]["state"] = json!({ "name": name, "type": state_type });
        let api = MockLinear::start();
        api.on("GetIssueTransitionDetails", issue.clone())
            .on("GetIssueTransitionDetails", issue);
        // The retired vcs setting changes nothing: the command names its tool.
        let cli = git(Cli::for_api(&api).env("LINEAR_VCS", "jj"));
        let run = cli.run(&["issue", "git", "start", "ENG-7"]);
        run.success();
        assert_eq!(
            run.stdout,
            format!(
                "✓ Created and switched to git branch \"eng-7-repair-the-widget\" from \"HEAD\" (0123456789ab); previous branch: \"main\"\n{said}"
            )
        );
        let cli = jj(Cli::for_api(&api)).env("LINEAR_VCS", "git");
        let run = cli.run(&["issue", "jj", "start", "ENG-7"]);
        run.success();
        assert!(
            run.stdout
                .ends_with(&format!("{URL}\n{ON_DESCRIBED}{said}")),
            "{run}"
        );
        assert_eq!(
            api.operations(),
            ["GetIssueTransitionDetails", "GetIssueTransitionDetails"]
        );
    }
}

#[test]
fn jj_start_has_no_git_flags() {
    let api = MockLinear::start();
    let cli = jj(Cli::for_api(&api));
    for flag in [["--branch", "my-branch"], ["--from-ref", "main"]] {
        cli.run(&["issue", "jj", "start", "ENG-7", flag[0], flag[1]])
            .usage_error()
            .stderr_has("unexpected argument");
    }
    assert!(api.requests().is_empty());
    assert!(calls(&cli, "jj").is_empty());
}

#[test]
fn git_start_with_a_bare_number_and_no_team_fails_before_any_request() {
    let api = MockLinear::start();
    let cli = git(Cli::for_api(&api));
    cli.run(&["issue", "git", "start", "7"])
        .failure()
        .stderr_has("Issue number 7 needs a team");
    assert!(api.requests().is_empty());
    assert!(calls(&cli, "git").is_empty());
}

#[test]
fn git_start_rejects_an_empty_issue_id_without_a_picker() {
    let api = MockLinear::start();
    let cli = git(Cli::for_api(&api).env("LINEAR_TEAM_ID", "ENG"));
    cli.run(&["issue", "git", "start", ""])
        .usage_error()
        .stderr_has("a value is required");
    assert!(api.requests().is_empty());
    assert!(calls(&cli, "git").is_empty());
}

#[test]
fn start_without_an_issue_off_a_terminal_is_a_usage_error_before_any_request() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api);
    cli.run(&["issue", "start"])
        .usage_error()
        .stderr_has("No issue to start")
        .stderr_has("Pass an issue ID, or run in a terminal");
    cli.run(&["issue", "start", "--team", "ENG", "--no-input"])
        .usage_error()
        .stderr_has("No issue to start")
        .stderr_has("Pass an issue ID.");
    assert!(api.requests().is_empty());
}

#[test]
fn start_without_an_issue_or_team_on_a_terminal_fails_before_any_request() {
    let api = MockLinear::start();
    let run = Cli::for_api(&api).run_tty(&["issue", "start"], &[]);
    assert_eq!(run.code, 2, "{run}");
    assert!(
        run.stdout.contains("No team to pick an issue from"),
        "{run}"
    );
    assert!(api.requests().is_empty());
}

#[test]
fn git_start_rejects_a_reference_that_names_no_issue_instead_of_picking() {
    let api = MockLinear::start();
    let cli = git(Cli::for_api(&api).env("LINEAR_TEAM_ID", "ENG"));
    cli.run(&["issue", "git", "start", "not-an-issue"])
        .failure()
        .stderr_has("Not an issue ID: not-an-issue");
    assert!(api.requests().is_empty());
    assert!(calls(&cli, "git").is_empty());
}

fn gh(cli: Cli) -> Cli {
    cli.stub_bin("gh", "echo https://github.com/acme/widgets/pull/1")
}

#[test]
fn pull_request_passes_the_issue_title_and_url_to_gh() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details());
    let cli = git(gh(Cli::for_api(&api)));
    cli.run(&["issue", "gh", "pull-request", "ENG-7"]).success();
    assert_eq!(
        cli.calls("gh"),
        [[
            "pr",
            "create",
            "--title",
            "ENG-7 Repair the widget",
            "--body",
            &format!("Fixes ENG-7\n\n{URL}")
        ]]
    );
    assert_eq!(api.variables("GetIssueDetails"), json!({ "id": "ENG-7" }));
    // gh reads the repository and branch itself.
    assert!(calls(&cli, "git").is_empty(), "{:?}", cli.calls("git"));
}

#[test]
fn pull_request_passes_everything_after_the_separator_to_gh_after_its_own_flags() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details());
    let cli = git(gh(Cli::for_api(&api)));
    cli.run(&[
        "issue",
        "gh",
        "pr",
        "ENG-7",
        "--title",
        " Custom ",
        "--",
        "--draft",
        "--base",
        "main",
        "--reviewer",
        "alice",
        "--title",
        "From gh",
        "--body",
        "Replaced",
        "ENG-9",
        "--",
        "-r",
    ])
    .success();
    // Unchanged and last, so gh takes the later --title and --body.
    assert_eq!(
        cli.calls("gh"),
        [[
            "pr",
            "create",
            "--title",
            " Custom ",
            "--body",
            &format!("Fixes ENG-7\n\n{URL}"),
            "--draft",
            "--base",
            "main",
            "--reviewer",
            "alice",
            "--title",
            "From gh",
            "--body",
            "Replaced",
            "ENG-9",
            "--",
            "-r"
        ]]
    );
    assert_eq!(api.operations(), ["GetIssueDetails"]);
}

#[test]
fn pull_request_needs_no_git_checkout_and_leaves_gh_s_own_errors_as_they_are() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details())
        .on("GetIssueDetails", details());
    let cli = gh(Cli::for_api(&api)).stub_bin("git", NO_REPOSITORY);
    cli.run(&[
        "issue",
        "gh",
        "pr",
        "ENG-7",
        "--",
        "--repo",
        "upstream/project",
        "--head",
        "alice:topic",
    ])
    .success();
    assert_eq!(
        cli.calls("gh")[0][6..],
        ["--repo", "upstream/project", "--head", "alice:topic"]
    );
    assert!(calls(&cli, "git").is_empty(), "{:?}", cli.calls("git"));
    let run = cli
        .stub_bin(
            "gh",
            "echo 'could not determine the current branch: not on any branch' >&2; exit 1",
        )
        .run(&["issue", "gh", "pr", "ENG-7"]);
    run.failure().stderr_has("not on any branch");
}

#[test]
fn pull_request_says_where_the_flags_that_moved_to_gh_go_and_fetches_nothing() {
    let api = MockLinear::start();
    let cli = git(gh(Cli::for_api(&api)));
    for (args, said, hint) in [
        (
            &["--draft"][..],
            "--draft is now passed to gh",
            "Put it after --: linear issue gh pr <ISSUE> -- --draft",
        ),
        (
            &["--web"],
            "--web is now passed to gh",
            "Put it after --: linear issue gh pr <ISSUE> -- --web",
        ),
        (
            &["--base", "main"],
            "--base is now passed to gh",
            "Put it after --: linear issue gh pr <ISSUE> -- --base <BRANCH>",
        ),
        (
            &["--head", "topic"],
            "--head is now passed to gh",
            "Put it after --: linear issue gh pr <ISSUE> -- --head <BRANCH>",
        ),
        (
            &["--draft", "--base", "main"],
            "--base and --draft are now passed to gh",
            "Put them after --: linear issue gh pr <ISSUE> -- --base <BRANCH> --draft",
        ),
    ] {
        let mut argv = vec!["issue", "gh", "pr", "ENG-7"];
        argv.extend(args);
        let run = cli.run(&argv);
        run.usage_error().stderr_has(said).stderr_has(hint);
        assert_eq!(run.stdout, "", "{run}");
    }
    assert!(api.requests().is_empty());
    assert!(cli.calls("gh").is_empty());
    let help = cli.run(&["issue", "gh", "pr", "--help"]);
    for hidden in ["--draft ", "--web ", "--base <", "--head <"] {
        assert!(
            !help.stdout.contains(&format!("      {hidden}")),
            "{hidden}: {help}"
        );
    }
}

#[test]
fn pull_request_body_is_the_given_description_then_the_clause_and_links() {
    let api = MockLinear::start();
    for _ in 0..4 {
        api.on("GetIssueDetails", details());
    }
    let cli = git(gh(Cli::for_api(&api)))
        .file("cwd/body.md", "\u{feff}## Summary\n\nMine.\n\n")
        .stdin(b"From stdin\n");
    cli.run(&["issue", "gh", "pr", "ENG-7", "-b", "Inline\n \n"])
        .success();
    cli.run(&["issue", "gh", "pr", "ENG-7", "--body-file", "body.md"])
        .success();
    cli.run(&["issue", "gh", "pr", "ENG-7", "-F", "-", "-r"])
        .success();
    cli.run(&["issue", "gh", "pr", "ENG-7", "--body", "Mine", "--ref"])
        .success();
    let bodies: Vec<String> = cli
        .calls("gh")
        .into_iter()
        .map(|call| call[5].clone())
        .collect();
    assert_eq!(
        bodies,
        [
            format!("Inline\n\nFixes ENG-7\n\n{URL}"),
            format!("## Summary\n\nMine.\n\nFixes ENG-7\n\n{URL}"),
            format!("From stdin\n\nReferences ENG-7\n\n{URL}"),
            format!("Mine\n\nReferences ENG-7\n\n{URL}"),
        ]
    );
}

#[test]
fn pull_request_for_several_issues_puts_the_description_before_one_clause() {
    let api = MockLinear::start();
    issues(&api, &["DES-5"]);
    let cli = git(gh(Cli::for_api(&api))).file("cwd/body.md", "Both at once\n");
    cli.run(&[
        "issue", "gh", "pr", "ENG-7", "DES-5", "-t", "Both", "-F", "body.md",
    ])
    .success();
    assert_eq!(
        cli.calls("gh")[0][3..6],
        [
            "Both".to_owned(),
            "--body".to_owned(),
            format!(
                "Both at once\n\nFixes ENG-7 and DES-5\n\n{URL}\n{}",
                other_url("DES-5", "Design")
            )
        ]
    );
}

/// A git stub that answers as git does when it finds no repository.
const NO_REPOSITORY: &str =
    "echo 'fatal: not a git repository (or any of the parent directories): .git' >&2; exit 128";

#[test]
fn at_git_keeps_every_other_git_answer_as_an_error() {
    for (case, git, said) in [
        (
            "an unexpected failure, with no .git anywhere",
            Some("echo 'fatal: bad object HEAD' >&2; exit 128"),
            "fatal: bad object HEAD",
        ),
        (
            "git's own exit status for something else",
            Some("echo 'error: unknown' >&2; exit 2"),
            "error: unknown",
        ),
        ("a signal", Some("kill -9 $$"), "`git` failed (signal: 9"),
        (
            "more than one line",
            Some("printf 'notice\\nmain\\n'"),
            "Unexpected output from `git symbolic-ref`: \"notice\\nmain\\n\"",
        ),
        (
            "no branch name",
            Some("printf '\\n'"),
            "Unexpected output from `git symbolic-ref`",
        ),
        (
            "invalid UTF-8",
            Some("printf 'ma\\377in\\n'"),
            "Unexpected output from `git symbolic-ref`",
        ),
        (
            "padding around the name",
            Some("printf ' main\\n'"),
            "Unexpected output from `git symbolic-ref`",
        ),
        ("no git on PATH", None, "Could not find `git`"),
    ] {
        let mut cli = Cli::new();
        if let Some(git) = git {
            cli = cli.stub_bin("git", git);
        }
        let run = cli.run(&["issue", "id", "@git"]);
        run.failure()
            .stderr_has("Failed to read the current git branch for @git")
            .stderr_has(said);
        assert!(
            !run.stderr.contains("Not in a git repository"),
            "{case}: {run}"
        );
        assert_eq!(run.stdout, "", "{case}: {run}");
    }
}

/// Runs the real git, from `PATH=/usr/bin:/bin`, with its calls recorded.
fn real_git(cli: Cli) -> Cli {
    cli.stub_bin("git", "exec git \"$@\"")
}

/// Runs the real git in `dir` to set up a repository for a test.
fn git_setup(dir: &std::path::Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", dir)
        .stdout(std::process::Stdio::null())
        .status()
        .expect("run git");
    assert!(status.success(), "git {args:?} in {}", dir.display());
}

#[test]
fn at_git_reads_the_branch_wherever_real_git_finds_one() {
    // Unborn: a branch with no commit yet is still the current branch.
    let cli = real_git(Cli::new());
    git_setup(&cli.path("cwd"), &["init", "-q", "-b", "eng-7-unborn"]);
    assert_eq!(
        cli.run(&["issue", "id", "@git"]).success().stdout,
        "ENG-7\n"
    );
    // A linked worktree, whose .git is a file.
    let cli = real_git(Cli::new()).cwd("worktree");
    let main = cli.path("main");
    std::fs::create_dir_all(&main).expect("main repository");
    git_setup(&main, &["init", "-q", "-b", "main"]);
    git_setup(&main, &["commit", "-q", "--allow-empty", "-m", "first"]);
    git_setup(
        &main,
        &["worktree", "add", "-q", "-b", "eng-8-wt", "../worktree"],
    );
    assert!(cli.path("worktree/.git").is_file());
    assert_eq!(
        cli.run(&["issue", "id", "@git"]).success().stdout,
        "ENG-8\n"
    );
    // A bare repository named by GIT_DIR, outside any checkout.
    let cli = real_git(Cli::new());
    let bare = cli.path("bare.git");
    std::fs::create_dir_all(&bare).expect("bare repository");
    git_setup(&bare, &["init", "-q", "--bare", "-b", "eng-9-bare"]);
    let cli = cli.env("GIT_DIR", &bare.display().to_string());
    assert_eq!(
        cli.run(&["issue", "id", "@git"]).success().stdout,
        "ENG-9\n"
    );
}

#[test]
fn at_git_tells_real_git_s_no_repository_from_a_bad_git_dir() {
    let cli = real_git(Cli::new());
    let root = cli.path("").display().to_string();
    let cli = cli.env("GIT_CEILING_DIRECTORIES", &root);
    cli.run(&["issue", "id", "@git"])
        .failure()
        .stderr_has("Not in a git repository")
        .stderr_has("@git reads the current branch.");
    let run = cli
        .env("GIT_DIR", "/nonexistent/repo")
        .run(&["issue", "id", "@git"]);
    run.failure()
        .stderr_has("Failed to read the current git branch for @git")
        .stderr_has("not a git repository: '/nonexistent/repo'");
    assert!(!run.stderr.contains("Not in a git repository"), "{run}");
}

#[test]
fn pull_request_refuses_blank_or_doubled_text_before_anything_is_read() {
    let api = MockLinear::start();
    let cli = git(gh(Cli::for_api(&api))).file("cwd/empty.md", " \n\n");
    for (args, said) in [
        (
            &["issue", "gh", "pr", "ENG-7", "--title", ""][..],
            "--title",
        ),
        (&["issue", "gh", "pr", "ENG-7", "-t", " \t"], "--title"),
        (&["issue", "gh", "pr", "ENG-7", "-b", " "], "--body"),
        (
            &["issue", "gh", "pr", "ENG-7", "-b", "x", "-F", "empty.md"],
            "cannot be used with",
        ),
    ] {
        let run = cli.run(args);
        run.usage_error().stderr_has(said);
        assert_eq!(run.stdout, "", "{run}");
    }
    let run = cli.run(&["issue", "gh", "pr", "ENG-7", "-F", "empty.md"]);
    run.usage_error()
        .stderr_has("Body file is empty: empty.md")
        .stderr_has("leave out --body-file");
    let run = cli.run(&["issue", "gh", "pr", "ENG-7", "-F", "missing.md"]);
    run.failure()
        .stderr_has("Failed to read body file: missing.md");
    assert!(api.requests().is_empty());
    assert!(cli.calls("gh").is_empty());
}

#[test]
fn a_pr_template_setting_is_reported_and_no_longer_read() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details())
        .on("GetIssueDetails", details());
    let cli = git(gh(Cli::for_api(&api)))
        .file("cwd/pr.md", "## Summary\n")
        .file("cwd/.linear.toml", "pr_template = \"pr.md\"\n");
    let run = cli.run(&["issue", "gh", "pr", "ENG-7"]);
    run.success()
        .stderr_has("Warning: pr_template from project config ")
        .stderr_has("was removed in 4.0 and does nothing.")
        .stderr_has("  Remove pr_template. To start the body from a file, pass it to `linear issue gh pr <ISSUE> --body-file <FILE>`; the issue links follow it.");
    let cli = cli.env("LINEAR_PR_TEMPLATE", "missing.md");
    let run = cli.run(&["issue", "gh", "pr", "ENG-7"]);
    run.success()
        .stderr_has("Warning: LINEAR_PR_TEMPLATE from process environment was removed in 4.0");
    assert_eq!(run.stderr.matches("Warning:").count(), 2, "{run}");
    for call in cli.calls("gh") {
        assert_eq!(call[5], format!("Fixes ENG-7\n\n{URL}"));
    }
}

#[test]
fn the_template_flags_say_to_pass_the_file_as_the_body_and_fetch_nothing() {
    let api = MockLinear::start();
    let cli = git(gh(Cli::for_api(&api))).file("cwd/pr.md", "## Summary\n");
    let hint = "-F <FILE> gives the same body: the file's text, then the issue links. Add -- --editor to fill it in before creating.";
    for (args, said) in [
        (
            &["--template", "pr.md"][..],
            "--template was removed in 4.0, with pr_template",
        ),
        (
            &["-T", "pr.md"],
            "--template was removed in 4.0, with pr_template",
        ),
        (
            &["--no-template"],
            "--no-template was removed in 4.0, with pr_template",
        ),
    ] {
        let mut argv = vec!["issue", "gh", "pr", "ENG-7"];
        argv.extend(args);
        let run = cli.run(&argv);
        run.usage_error().stderr_has(said).stderr_has(hint);
        assert_eq!(run.stdout, "", "{run}");
    }
    assert!(api.requests().is_empty());
    assert!(cli.calls("gh").is_empty());
    let help = cli.run(&["issue", "gh", "pr", "--help"]);
    assert!(!help.stdout.contains("-T, --template"), "{help}");
    assert!(!help.stdout.contains("--no-template"), "{help}");
}

#[test]
fn pull_request_fails_with_status_1_when_gh_fails_and_130_when_it_is_cancelled() {
    for (script, code) in [
        ("echo 'To get started, run: gh auth login' >&2; exit 4", 1),
        ("exit 2", 130),
    ] {
        let api = MockLinear::start();
        api.on("GetIssueDetails", details());
        let run = git(Cli::for_api(&api))
            .stub_bin("gh", script)
            .run(&["issue", "gh", "pr", "ENG-7"]);
        assert_eq!(run.code, code, "{run}");
        if code == 1 {
            run.stderr_has("gh auth login");
        }
    }
}

#[test]
fn commits_was_removed_and_says_how_to_search_with_jj() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api)
        .file("cwd/.jj/repo/store", "")
        .stub_bin("jj", "echo 'Fixes ENG-7'")
        .stub_bin("git", "echo eng-7-repair-the-widget");
    for args in [
        &["issue", "commits"][..],
        &["issue", "commits", "ENG-7"],
        &["issue", "commits", "@jj"],
        &["issue", "commits", "--help"],
        &["i", "commits", "ENG-7", "--json"],
    ] {
        let run = cli.run(args);
        run.usage_error()
            .stderr_has("`issue commits` was removed in 4.0")
            .stderr_has(r#"jj log -p -r "description(regex:'(?mi)^Linear-issue:.*\bENG-123\b')""#);
        assert_eq!(run.stdout, "", "{run}");
    }
    assert!(api.requests().is_empty());
    assert!(calls(&cli, "jj").is_empty(), "{:?}", cli.calls("jj"));
    assert!(calls(&cli, "git").is_empty(), "{:?}", cli.calls("git"));
    let help = cli.run(&["issue", "--help"]);
    help.success();
    assert!(!help.stdout.contains("commits"), "{help}");
}

#[test]
fn git_start_outside_a_repository_says_so_before_changing_the_state() {
    let api = MockLinear::start();
    api.on("GetIssueTransitionDetails", details());
    let cli =
        Cli::for_api(&api).stub_bin("git", "echo 'fatal: not a git repository' >&2; exit 128");
    cli.run(&["issue", "git", "start", "ENG-7"])
        .failure()
        .stderr_has("Not in a git repository")
        .stderr_has("linear issue start <ISSUE>");
    assert_eq!(api.operations(), ["GetIssueTransitionDetails"]);
}

#[test]
fn jj_start_stops_when_the_change_probe_fails() {
    let api = MockLinear::start();
    api.on("GetIssueTransitionDetails", details());
    let cli = Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .file("cwd/.jj/repo/store", "")
        .stub_bin("jj", "echo 'Error: concurrent operation' >&2; exit 1");
    cli.run(&["issue", "jj", "start", "ENG-7"])
        .failure()
        .stderr_has("concurrent operation");
    assert_eq!(calls(&cli, "jj").len(), 1);
    assert_eq!(api.operations(), ["GetIssueTransitionDetails"]);
}

#[test]
fn git_start_creates_a_local_branch_when_only_another_revision_has_its_name() {
    for branch in ["release", "deadbeef", "origin/release"] {
        let api = MockLinear::start();
        api.on("GetIssueTransitionDetails", details())
            .on("GetWorkflowStates", states())
            .on(
                "UpdateIssueState",
                json!({"issueUpdate": {"success": true}}),
            );
        let cli = Cli::for_api(&api).stub_bin(
            "git",
            "case \"$1 $2\" in
 'rev-parse --verify') echo 0123456789ab ;;
 'show-ref --verify') exit 1 ;;
 'symbolic-ref --quiet') echo main ;;
 'checkout -b') exit 0 ;;
 *) exit 1 ;;
 esac",
        );
        cli.run(&["issue", "git", "start", "ENG-7", "--branch", branch])
            .success()
            .stdout_has(&format!(
                "✓ Created and switched to git branch \"{branch}\" from \"HEAD\""
            ));
        assert_eq!(
            calls(&cli, "git"),
            [
                vec![
                    "show-ref".to_owned(),
                    "--verify".to_owned(),
                    "--quiet".to_owned(),
                    format!("refs/heads/{branch}")
                ],
                vec![
                    "symbolic-ref".to_owned(),
                    "--quiet".to_owned(),
                    "--short".to_owned(),
                    "HEAD".to_owned()
                ],
                vec![
                    "rev-parse".to_owned(),
                    "--verify".to_owned(),
                    "--quiet".to_owned(),
                    "--short=12".to_owned(),
                    "--end-of-options".to_owned(),
                    "HEAD^{commit}".to_owned()
                ],
                vec![
                    "checkout".to_owned(),
                    "-b".to_owned(),
                    branch.to_owned(),
                    "HEAD".to_owned()
                ],
            ]
        );
        assert_eq!(
            api.operations(),
            [
                "GetIssueTransitionDetails",
                "GetWorkflowStates",
                "UpdateIssueState"
            ]
        );
        assert_eq!(
            api.variables("UpdateIssueState"),
            json!({"issueId": "ENG-7", "stateId": "state-progress"})
        );
    }
}

#[test]
fn git_start_still_requires_a_terminal_when_a_local_branch_exists() {
    let api = MockLinear::start();
    api.on("GetIssueTransitionDetails", details());
    let cli = Cli::for_api(&api).stub_bin(
        "git",
        "case \"$1 $2\" in
 'rev-parse --verify' | 'show-ref --verify') exit 0 ;;
 *) exit 1 ;;
 esac",
    );
    cli.run(&["issue", "git", "start", "ENG-7"])
        .usage_error()
        .stderr_has("--branch with a new name");
    assert_eq!(api.operations(), ["GetIssueTransitionDetails"]);
    assert!(
        !calls(&cli, "git")
            .iter()
            .any(|args| args.first().is_some_and(|arg| arg == "checkout"))
    );
}

#[test]
fn git_start_preserves_unexpected_git_probe_failures() {
    for status in [2, 128] {
        let api = MockLinear::start();
        api.on("GetIssueTransitionDetails", details());
        let cli = Cli::for_api(&api).file("cwd/.git/HEAD", "").stub_bin(
            "git",
            &format!(
                "case \"$1 $2\" in
 'rev-parse --verify' | 'show-ref --verify') echo 'fatal: corrupt refs' >&2; exit {status} ;;
 *) exit 1 ;;
 esac"
            ),
        );
        cli.run(&["issue", "git", "start", "ENG-7"])
            .failure()
            .stderr_has("Failed to check if branch exists")
            .stderr_has("fatal: corrupt refs");
        assert_eq!(api.operations(), ["GetIssueTransitionDetails"]);
        assert!(
            !calls(&cli, "git")
                .iter()
                .any(|args| args.first().is_some_and(|arg| arg == "checkout"))
        );
    }
}

#[test]
fn git_start_does_not_treat_a_revision_expression_as_a_branch() {
    let api = MockLinear::start();
    api.on("GetIssueTransitionDetails", details());
    let cli = Cli::for_api(&api).stub_bin(
        "git",
        "case \"$1 $2\" in
 'rev-parse --verify') echo 0123456789ab ;;
 'show-ref --verify') exit 1 ;;
 'symbolic-ref --quiet') echo main ;;
 'checkout -b') echo 'fatal: not a valid branch name' >&2; exit 128 ;;
 *) exit 1 ;;
 esac",
    );
    let run = cli.run(&["issue", "git", "start", "ENG-7", "--branch", "main~0"]);
    run.failure().stderr_has("not a valid branch name");
    assert_eq!(run.stdout, "", "a failed checkout reports nothing done");
    assert_eq!(api.operations(), ["GetIssueTransitionDetails"]);
}

#[test]
fn describe_preserves_exact_remote_text_when_piped() {
    for title in [
        "Plain café",
        "ok café\u{1b}]0;owned\u{7}\u{1b}[2J\u{9b}31mred\u{8}\u{7f}\r\n\tend",
    ] {
        for (flags, magic) in [(Vec::new(), "Fixes"), (vec!["--references"], "References")] {
            let api = MockLinear::start();
            let mut reply = details();
            let url = format!("{URL}\u{1b}[2J");
            reply["issue"]["title"] = json!(title);
            reply["issue"]["url"] = json!(url);
            api.on("GetIssueDetails", reply);
            let mut args = vec!["issue", "describe", "ENG-7"];
            args.extend(flags);
            let run = Cli::for_api(&api).run(&args);
            run.success();
            assert_eq!(
                run.stdout,
                format!("ENG-7 {title}\n\nLinear-issue: {magic} ENG-7\nLinear-issue-url: {url}\n")
            );
            assert_eq!(api.operations(), ["GetIssueDetails"]);
            assert_eq!(api.variables("GetIssueDetails"), json!({"id": "ENG-7"}));
        }
    }
}

#[test]
fn start_picks_from_the_team_given_with_team_and_says_when_it_has_nothing() {
    let api = MockLinear::start();
    api.on(
        "ResolveTeam",
        crate::team::resolved("team-ops-id", "OPS", "Operations"),
    )
    .on(
        "GetIssuesForState",
        json!({ "issues": { "nodes": [], "pageInfo": { "hasNextPage": false, "endCursor": null } } }),
    );
    Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .run_tty(&["issue", "start", "--team", "Operations"], &[])
        .failure()
        .stdout_has("OPS has no unstarted issues assigned to you")
        .stdout_has("Pass -A");
    assert_eq!(
        api.variables("GetIssuesForState")["filter"]["team"],
        json!({ "key": { "eq": "OPS" } })
    );
}

#[test]
fn start_rejects_all_assignees_with_unassigned() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .run(&["issue", "start", "-A", "-U"])
        .usage_error();
    assert!(api.requests().is_empty());
}

#[test]
fn the_old_pull_request_spellings_say_where_the_command_went() {
    let api = MockLinear::start();
    let cli = gh(Cli::for_api(&api));
    for args in [
        &["issue", "pr"][..],
        &["issue", "pull-request", "ENG-7", "--draft", "--title", "x"],
        &["i", "pr", "--help"],
    ] {
        cli.run(args)
            .usage_error()
            .stderr_has("`issue pull-request` moved to `issue gh pull-request`")
            .stderr_has("Run `linear issue gh pr <ISSUE>`; gh options such as --draft or --base now go after `--`. From a jj repository, push a bookmark first and pass `-- --head <BRANCH>`.");
    }
    assert!(api.requests().is_empty());
    assert!(cli.calls("gh").is_empty());
    let help = cli.run(&["issue", "--help"]);
    help.success().stdout_has("  gh ");
    assert!(!help.stdout.contains("pull-request"), "{help}");
}

/// A sandbox whose git branch and jj trailers both name ENG-99, with tools
/// that record their calls.
fn repo_naming_an_issue(api: &MockLinear) -> Cli {
    Cli::for_api(api)
        .env("LINEAR_TEAM_ID", "ENG")
        .file("cwd/.jj/repo/store", "")
        .stub_bin("git", "echo eng-99-other")
        .stub_bin("jj", "echo 'Fixes ENG-99'")
}

#[test]
fn no_command_takes_the_issue_from_the_branch_or_jj_change() {
    let api = MockLinear::start();
    let cli = repo_naming_an_issue(&api);
    for args in [
        &["issue", "view"][..],
        &["issue", "view", "--json"],
        &["issue", "title"],
        &["issue", "url"],
        &["issue", "describe"],
        &["issue", "update", "--state", "Done"],
        &["issue", "comment", "add", "--body", "hi"],
        &["issue", "comment", "list"],
        &["issue", "relation", "list"],
        &["issue", "agent-session", "list"],
        &["issue", "archive", "--yes"],
        &["issue", "delete", "--yes"],
        &["issue", "link"],
        &["issue", "view", " "],
    ] {
        let run = cli.run(args);
        assert_eq!(run.code, 2, "{run}");
        assert!(run.stderr.contains("ISSUE"), "{run}");
        assert_eq!(run.stdout, "", "{run}");
    }
    assert!(calls(&cli, "git").is_empty(), "{:?}", cli.calls("git"));
    assert!(calls(&cli, "jj").is_empty(), "{:?}", cli.calls("jj"));
    assert!(api.requests().is_empty());
}

#[test]
fn a_named_issue_is_used_without_asking_the_repository() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details());
    let cli = repo_naming_an_issue(&api);
    cli.run(&["issue", "title", "ENG-7"])
        .success()
        .stdout_has("Repair the widget");
    assert_eq!(api.variables("GetIssueDetails"), json!({ "id": "ENG-7" }));
    assert!(calls(&cli, "git").is_empty(), "{:?}", cli.calls("git"));
    assert!(calls(&cli, "jj").is_empty());
    cli.run(&["issue", "title", "not-an-issue"])
        .usage_error()
        .stderr_has("Not an issue ID: not-an-issue")
        .stderr_has("Pass an issue ID like ENG-123");
}

#[test]
fn pull_requests_without_an_issue_say_how_to_name_one_and_read_nothing() {
    let api = MockLinear::start();
    let cli = gh(repo_naming_an_issue(&api));
    for args in [
        &["issue", "gh", "pr", "-r"][..],
        &["issue", "gh", "pull-request", "-F", "missing.md"],
    ] {
        let run = cli.run(args);
        run.usage_error()
            .stderr_has("No issue to open a pull request for")
            .stderr_has("linear issue gh pr ENG-123")
            .stderr_has("`@git`")
            .stderr_has("`@jj`");
        assert_eq!(run.stdout, "", "{run}");
    }
    assert!(calls(&cli, "git").is_empty(), "{:?}", cli.calls("git"));
    assert!(calls(&cli, "jj").is_empty(), "{:?}", cli.calls("jj"));
    assert!(cli.calls("gh").is_empty());
    cli.run(&["issue", "gh", "pr", "--help"])
        .success()
        .stdout_has("Usage: linear issue gh pull-request [OPTIONS] <ISSUE>")
        .stdout_has("@git");
    assert!(api.requests().is_empty());
}

#[test]
fn pull_request_help_states_the_rule_and_how_to_push_from_jj() {
    let cli = Cli::new();
    let gh = cli.run(&["issue", "gh", "pr", "--help"]);
    gh.success()
        .stdout_has("Usage: linear issue gh pull-request [OPTIONS] <ISSUE>... [-- <GH_ARGS>...]")
        .stdout_has("Before `--` is what linear builds from the issues: the title and the\nbody. Everything after `--` is passed to `gh pr create` as it is,")
        .stdout_has("A --title or --body after `--` replaces linear's, issue links\nincluded.")
        .stdout_has("gh's own --template cannot be used")
        .stdout_has("`gh pr create --help`")
        .stdout_has("  linear issue gh pr ENG-7 -- --draft --base main\n")
        .stdout_has("  linear issue gh pr ENG-7 -- --editor ")
        .stdout_has("linear itself never creates, moves or pushes a branch or bookmark.")
        .stdout_has("  jj git push --named \"$branch=@-\"\n")
        .stdout_has("  linear issue gh pr ENG-7 -- --head \"$branch\"\n")
        .stdout_has("right after `issue jj start`, @ is\nthe described change, so run `jj new` first (or push @)")
        .stdout_has("Later, `jj git push --bookmark \"$branch\"` updates the pull request.")
        .stdout_has("first run `jj bookmark move \"$branch\" --to @-`.");
    assert!(!gh.stdout.contains("vcs"), "{gh}");
    assert!(!gh.stdout.contains("jj pr"), "{gh}");
    let jj = cli.run(&["issue", "jj", "--help"]);
    jj.success().stdout_has("  start ");
    assert!(!jj.stdout.contains("pull-request"), "{jj}");
    cli.run(&["issue", "jj", "pr", "ENG-7"]).usage_error();
    cli.run(&["issue", "jj", "pull-request", "ENG-7"])
        .usage_error();
}

#[test]
fn issue_id_prints_what_an_argument_names_without_credentials_or_requests() {
    let cli = Cli::new().env("LINEAR_TEAM_ID", "ENG").stub_bin(
        "git",
        "case \"$1\" in symbolic-ref) echo sam/eng-7-repair-the-widget ;; *) exit 1 ;; esac",
    );
    let cli = jj_refs(
        cli,
        false,
        &[("bbb", &["Fixes OPS-2"]), ("ccc", &["Fixes ENG-7"])],
    );
    for (input, printed) in [
        ("eng-7", "ENG-7\n"),
        ("7", "ENG-7\n"),
        ("@git", "ENG-7\n"),
        ("@jj", "ENG-7\nOPS-2\n"),
    ] {
        let run = cli.run(&["issue", "id", input]);
        run.success();
        assert_eq!(run.stdout, printed, "{run}");
        assert_eq!(run.stderr, "", "{run}");
    }
    cli.run(&["issue", "id"]).usage_error().stderr_has("<REF>");
    cli.run(&["issue", "id", "not-an-issue"])
        .usage_error()
        .stderr_has("Not an issue ID: not-an-issue")
        .stderr_has("@git, or @jj");
    let help = cli.run(&["issue", "--help"]);
    help.success()
        .stdout_has("  id ")
        .stdout_has("Issue references:");
}

#[test]
fn link_takes_the_issue_and_the_url_and_checks_the_url_before_any_request() {
    let api = MockLinear::start();
    let cli = repo_naming_an_issue(&api);
    // The old one-argument form named only the URL.
    cli.run(&["issue", "link", "https://example.com/doc"])
        .usage_error()
        .stderr_has("<URL>");
    cli.run(&["issue", "link", "ENG-7", "example.com/doc"])
        .usage_error()
        .stderr_has("Invalid URL: 'example.com/doc'")
        .stderr_has("http:// or https://");
    assert!(api.requests().is_empty());
    assert!(calls(&cli, "git").is_empty());
}

#[test]
fn archive_and_delete_need_an_issue_or_bulk_ids() {
    let api = MockLinear::start();
    let cli = repo_naming_an_issue(&api);
    for command in ["archive", "delete"] {
        cli.run(&["issue", command, "--yes", "--bulk"])
            .usage_error()
            .stderr_has("Issue ID required")
            .stderr_has("after --bulk");
    }
    assert!(api.requests().is_empty());
}

/// A git on the branch `branch` (detached when it is empty), and a gh.
pub(crate) fn git_on(cli: Cli, branch: &str) -> Cli {
    let answer = if branch.is_empty() {
        "exit 1".to_owned()
    } else {
        format!("echo '{branch}'")
    };
    gh(cli).stub_bin(
        "git",
        &format!("case \"$1\" in symbolic-ref) {answer} ;; *) exit 1 ;; esac"),
    )
}

/// One `[change, [[key, value]...]]` line of the `@jj` trailer listing.
fn jj_change(change: &str, linear_issue: &[&str]) -> String {
    let trailers: Vec<Value> = linear_issue
        .iter()
        .map(|value| json!(["Linear-issue", value]))
        .collect();
    json!([change, trailers]).to_string()
}

/// A jj repository for `@jj`: `trunk()` is the root commit when
/// `trunk_is_root`, and `trunk()..@` holds `changes` with their
/// `Linear-issue` trailer values. Any other jj call fails.
pub(crate) fn jj_refs(cli: Cli, trunk_is_root: bool, changes: &[(&str, &[&str])]) -> Cli {
    let lines: Vec<String> = changes
        .iter()
        .map(|(change, trailers)| format!("'{}'", jj_change(change, trailers)))
        .collect();
    cli.file("cwd/.jj/repo/store", "").stub_bin(
        "jj",
        &format!(
            "case \"$*\" in\n\
             *'log -r trunk() '*) echo {trunk_is_root} ;;\n\
             *'log -r trunk()..@ '*) printf '%s\\n' {} ;;\n\
             *) echo \"unexpected jj $*\" >&2; exit 1 ;;\n\
             esac",
            if lines.is_empty() {
                "''".to_owned()
            } else {
                lines.join(" ")
            }
        ),
    )
}

/// The revsets of the jj calls, asserting each reads without a snapshot.
fn jj_reads(cli: &Cli) -> Vec<String> {
    calls(cli, "jj")
        .into_iter()
        .map(|args| {
            assert_eq!(
                args[..5],
                [
                    "--ignore-working-copy",
                    "--no-pager",
                    "--color",
                    "never",
                    "log"
                ],
                "{args:?}"
            );
            assert_eq!(args[5], "-r", "{args:?}");
            args[6].clone()
        })
        .collect()
}

#[test]
fn at_git_reads_the_issue_from_the_branch_name_and_says_so_before_any_request() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details());
    let cli = git_on(Cli::for_api(&api), "sam/eng-7-add-utf-8-support");
    let run = cli.run(&["issue", "title", "@git"]);
    run.success()
        .stderr_has("Using issue ENG-7 from the git branch name \"sam/eng-7-add-utf-8-support\"\n");
    assert_eq!(run.stdout, "Repair the widget\n");
    assert_eq!(api.variables("GetIssueDetails"), json!({ "id": "ENG-7" }));
    assert_eq!(
        calls(&cli, "git"),
        [["symbolic-ref", "--quiet", "--short", "HEAD"]]
    );
    assert!(cli.calls("jj").is_empty());
}

#[test]
fn at_git_fails_before_any_request_when_the_branch_names_no_issue() {
    let api = MockLinear::start();
    for (branch, said, hint) in [
        (
            "main",
            "No issue ID in the git branch name \"main\"",
            "Name the issue instead, like ENG-123",
        ),
        (
            "",
            "Cannot read @git: git HEAD is detached",
            "Name the issue instead, like ENG-123",
        ),
    ] {
        git_on(Cli::for_api(&api), branch)
            .run(&["issue", "view", "@git"])
            .usage_error()
            .stderr_has(said)
            .stderr_has(hint);
    }
    Cli::for_api(&api)
        .stub_bin("git", NO_REPOSITORY)
        .run(&["issue", "url", "@git"])
        .failure()
        .stderr_has("Not in a git repository")
        .stderr_has("or name the issue instead");
    assert!(api.requests().is_empty());
}

#[test]
fn at_jj_reads_every_trailer_in_trunk_to_working_copy_without_a_snapshot() {
    let api = MockLinear::start();
    let mut ops = details();
    ops["issue"]["identifier"] = json!("OPS-2");
    api.on_variables("GetIssueDetails", json!({ "id": "ENG-7" }), details())
        .on_variables("GetIssueDetails", json!({ "id": "OPS-2" }), ops);
    let cli = jj_refs(
        Cli::for_api(&api),
        false,
        &[
            ("aaa", &[]),
            ("bbb", &["Fixes OPS-2"]),
            ("ccc", &["Fixes ENG-7", "References ops-2"]),
        ],
    )
    .stub_bin("git", "exit 1");
    let run = cli.run(&["issue", "view", "@jj", "--json", "--no-comments"]);
    run.success()
        .stderr_has("Using issues ENG-7, OPS-2 from Linear-issue trailers in trunk()..@\n");
    let shown: Vec<Value> = run
        .json()
        .as_array()
        .expect("an array for two issues")
        .iter()
        .map(|issue| issue["identifier"].clone())
        .collect();
    assert_eq!(shown, [json!("ENG-7"), json!("OPS-2")]);
    assert_eq!(jj_reads(&cli), ["trunk()", "trunk()..@"]);
    assert!(calls(&cli, "git").is_empty());
}

#[test]
fn at_jj_naming_one_issue_is_viewed_as_one() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details());
    let cli = jj_refs(Cli::for_api(&api), false, &[("bbb", &["Fixes ENG-7"])]);
    let run = cli.run(&["issue", "view", "@jj", "--json", "--no-comments"]);
    run.success()
        .stderr_has("Using issue ENG-7 from Linear-issue trailers in trunk()..@\n");
    assert_eq!(run.json()["identifier"], "ENG-7");
}

#[test]
fn a_command_for_one_issue_refuses_a_reference_that_names_several() {
    let api = MockLinear::start();
    let cli = jj_refs(
        Cli::for_api(&api),
        false,
        &[("bbb", &["Fixes OPS-2"]), ("ccc", &["Fixes ENG-7"])],
    );
    for args in [
        &["issue", "title", "@jj"][..],
        &["issue", "complete", "@jj"],
        &["issue", "start", "@jj"],
        &["issue", "archive", "@jj", "--yes"],
        &["issue", "relation", "add", "ENG-1", "blocks", "@jj"],
        &["issue", "update", "ENG-1", "--parent", "@jj"],
        &["document", "list", "--issue", "@jj"],
    ] {
        let run = cli.run(args);
        run.usage_error()
            .stderr_has("@jj names several issues: ENG-7, OPS-2")
            .stderr_has("Name the one you mean, like ENG-7.");
        assert_eq!(run.stdout, "", "{run}");
        assert!(!run.stderr.contains("Using issue"), "{run}");
    }
    assert!(api.requests().is_empty());
    assert!(cli.calls("gh").is_empty());
}

#[test]
fn at_jj_is_refused_when_trunk_is_the_root_commit_and_never_searches_history() {
    let api = MockLinear::start();
    let cli = jj_refs(Cli::for_api(&api), true, &[("bbb", &["Fixes ENG-7"])]);
    cli.run(&["issue", "view", "@jj"])
        .usage_error()
        .stderr_has("Cannot read @jj: trunk() is the root commit")
        .stderr_has("revset-aliases");
    assert_eq!(jj_reads(&cli), ["trunk()"]);
    assert!(api.requests().is_empty());
}

#[test]
fn at_jj_fails_before_any_request_when_the_trailers_name_no_issue() {
    let api = MockLinear::start();
    for (changes, said) in [
        (
            &[("aaa", &[][..])][..],
            "Cannot read @jj: no Linear-issue trailer in trunk()..@ names an issue",
        ),
        (
            &[("aaa", &["Fixes ENG-7"][..]), ("bbb", &["Fixes it"])],
            "Cannot read @jj: the Linear-issue trailer \"Fixes it\" of change bbb names no issue",
        ),
    ] {
        jj_refs(Cli::for_api(&api), false, changes)
            .run(&["issue", "title", "@jj"])
            .usage_error()
            .stderr_has(said)
            .stderr_has("Name the issue instead, like ENG-123");
    }
    Cli::for_api(&api)
        .stub_bin(
            "jj",
            "echo 'Error: There is no jj repo in \".\"' >&2; exit 1",
        )
        .run(&["issue", "title", "@jj"])
        .failure()
        .stderr_has("Not in a jj repository");
    assert!(api.requests().is_empty());
}

#[test]
fn an_unknown_reference_is_a_usage_error_before_anything_is_read() {
    let api = MockLinear::start();
    let cli = repo_naming_an_issue(&api);
    for args in [
        &["issue", "view", "@me"][..],
        &["issue", "title", "@GIT"],
        &["issue", "id", "@branch"],
        &["issue", "update", "ENG-1", "--parent", "@head"],
        &["issue", "gh", "pr", "@pr"],
    ] {
        cli.run(args)
            .usage_error()
            .stderr_has("unknown issue reference")
            .stderr_has("use @git, @jj, or an issue ID like ENG-123");
    }
    assert!(calls(&cli, "git").is_empty());
    assert!(calls(&cli, "jj").is_empty());
    assert!(api.requests().is_empty());
}

#[test]
fn bulk_input_takes_no_references() {
    let api = MockLinear::start();
    for command in ["archive", "delete"] {
        for (args, stdin) in [
            (&["--bulk", "ENG-1", "@git"][..], &b""[..]),
            (&["--bulk-file", "ids.txt"], b""),
            (&["--bulk-stdin"], b"ENG-1\n@jj\n"),
        ] {
            let cli = repo_naming_an_issue(&api)
                .file("cwd/ids.txt", "ENG-1\n@jj\n")
                .stdin(stdin);
            let mut argv = vec!["issue", command, "--yes"];
            argv.extend(args);
            cli.run(&argv)
                .usage_error()
                .stderr_has("Bulk input takes issue IDs, not references like @")
                .stderr_has("List the issue IDs");
            assert!(calls(&cli, "git").is_empty());
            assert!(calls(&cli, "jj").is_empty());
        }
    }
    assert!(api.requests().is_empty());
}

#[test]
fn a_parent_can_be_a_reference() {
    let api = MockLinear::start();
    api.on_raw("GetIssueId", 200, r#"{"errors":[{"message":"Entity not found: Issue","extensions":{"userPresentableMessage":"Could not find referenced Issue."}}]}"#);
    git_on(Cli::for_api(&api), "eng-404-parent")
        .run(&["issue", "update", "ENG-1", "--parent", "@git"])
        .not_found()
        .stderr_has("Using issue ENG-404 from the git branch name \"eng-404-parent\"\n")
        .stderr_has("Parent issue not found: ENG-404");
    assert_eq!(api.operations(), ["GetIssueId"]);
}

#[test]
fn gh_pull_request_takes_the_issue_from_at_git_and_never_from_head() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details());
    let cli = git_on(Cli::for_api(&api), "sam/eng-7-add-utf-8-support");
    let run = cli.run(&[
        "issue",
        "gh",
        "pr",
        "@git",
        "--",
        "--head",
        "someone:ops-1-other",
        "--draft",
    ]);
    run.success()
        .stderr_has("Using issue ENG-7 from the git branch name \"sam/eng-7-add-utf-8-support\"\n");
    assert_eq!(run.stdout, "https://github.com/acme/widgets/pull/1\n");
    assert_eq!(
        cli.calls("gh")[0],
        [
            "pr",
            "create",
            "--title",
            "ENG-7 Repair the widget",
            "--body",
            &format!("Fixes ENG-7\n\n{URL}"),
            "--head",
            "someone:ops-1-other",
            "--draft"
        ]
    );
}

#[test]
fn gh_pull_request_reads_at_jj_without_a_snapshot_and_whatever_head_names() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details());
    // A colocated jj repository (git's HEAD detached), with the retired
    // setting that used to select jj.
    let cli = git_on(
        jj_refs(
            Cli::for_api(&api),
            false,
            &[("aaa", &[]), ("bbb", &["Fixes ENG-7"])],
        ),
        "",
    )
    .env("LINEAR_VCS", "jj");
    let run = cli.run(&["issue", "gh", "pr", "@jj", "--", "--head", "elsewhere"]);
    run.success()
        .stderr_has("LINEAR_VCS from process environment was removed in 4.0")
        .stderr_has("Using issue ENG-7 from Linear-issue trailers in trunk()..@\n");
    assert_eq!(run.stdout, "https://github.com/acme/widgets/pull/1\n");
    assert_eq!(jj_reads(&cli), ["trunk()", "trunk()..@"]);
    assert_eq!(
        cli.calls("gh"),
        [[
            "pr",
            "create",
            "--title",
            "ENG-7 Repair the widget",
            "--body",
            &format!("Fixes ENG-7\n\n{URL}"),
            "--head",
            "elsewhere"
        ]]
    );
    // --head given: git is not asked for a branch.
    assert!(calls(&cli, "git").is_empty(), "{:?}", cli.calls("git"));
}

/// The details of another issue than ENG-7, for pull requests that cover
/// several.
fn other(identifier: &str, title: &str) -> Value {
    let mut other = details();
    let slug = title.to_lowercase().replace(' ', "-");
    other["issue"]["identifier"] = json!(identifier);
    other["issue"]["title"] = json!(title);
    other["issue"]["url"] = json!(other_url(identifier, title));
    other["issue"]["branchName"] = json!(format!("{}-{slug}", identifier.to_lowercase()));
    other
}

fn other_url(identifier: &str, title: &str) -> String {
    let slug = title.to_lowercase().replace(' ', "-");
    format!("https://linear.app/acme/issue/{identifier}/{slug}")
}

/// A mock that answers ENG-7 and these of DES-5 ("Design") and ENG-256
/// ("Followup"), whatever order they are asked in.
fn issues(api: &MockLinear, others: &[&str]) {
    api.on_variables("GetIssueDetails", json!({ "id": "ENG-7" }), details());
    for identifier in others {
        let title = match *identifier {
            "DES-5" => "Design",
            "ENG-256" => "Followup",
            unexpected => panic!("no fixture for {unexpected}"),
        };
        api.on_variables(
            "GetIssueDetails",
            json!({ "id": identifier }),
            other(identifier, title),
        );
    }
}

/// ENG-7, DES-5 and ENG-256: see [`issues`].
fn three_issues(api: &MockLinear) {
    issues(api, &["DES-5", "ENG-256"]);
}

#[test]
fn gh_pull_request_for_several_issues_takes_the_title_and_closes_them_all() {
    let api = MockLinear::start();
    three_issues(&api);
    let cli = git(gh(Cli::for_api(&api))).file("cwd/pr.md", "## Summary\nDescribe the change.\n\n");
    let run = cli.run(&[
        "issue",
        "gh",
        "pr",
        "ENG-7",
        "DES-5",
        "ENG-256",
        "-F",
        "pr.md",
        "--title",
        " Widget and design ",
        "--",
        "--draft",
    ]);
    run.success();
    assert_eq!(run.stdout, "https://github.com/acme/widgets/pull/1\n");
    let body = format!(
        "## Summary\nDescribe the change.\n\nFixes ENG-7, DES-5, and ENG-256\n\n{URL}\n{}\n{}",
        other_url("DES-5", "Design"),
        other_url("ENG-256", "Followup")
    );
    assert_eq!(
        cli.calls("gh"),
        [[
            "pr",
            "create",
            "--title",
            " Widget and design ",
            "--body",
            &body,
            "--draft"
        ]]
    );
    let mut fetched: Vec<Value> = api
        .requests()
        .into_iter()
        .map(|request| request.variables["id"].clone())
        .collect();
    fetched.sort_by_key(Value::to_string);
    assert_eq!(fetched, [json!("DES-5"), json!("ENG-256"), json!("ENG-7")]);
}

#[test]
fn several_issue_bodies_say_and_for_two_and_references_with_the_flag() {
    for (flags, clause) in [
        (&[][..], "Fixes ENG-7 and DES-5"),
        (&["--references"], "References ENG-7 and DES-5"),
        (&["--ref"], "References ENG-7 and DES-5"),
        (&["-r"], "References ENG-7 and DES-5"),
    ] {
        let api = MockLinear::start();
        issues(&api, &["DES-5"]);
        let cli = git(gh(Cli::for_api(&api)));
        let mut args = vec!["issue", "gh", "pr", "ENG-7", "DES-5", "-t", "Both"];
        args.extend(flags);
        cli.run(&args).success();
        assert_eq!(
            cli.calls("gh")[0][3..6],
            [
                "Both".to_owned(),
                "--body".to_owned(),
                format!("{clause}\n\n{URL}\n{}", other_url("DES-5", "Design"))
            ],
            "{flags:?}"
        );
    }
}

#[test]
fn one_issue_with_references_keeps_its_title_and_writes_that_clause() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details())
        .on("GetIssueDetails", details());
    let cli = git(gh(Cli::for_api(&api)));
    cli.run(&["issue", "gh", "pr", "ENG-7", "--references"])
        .success();
    cli.run(&["issue", "gh", "pr", "ENG-7", "--ref"]).success();
    for call in cli.calls("gh") {
        assert_eq!(
            call,
            [
                "pr",
                "create",
                "--title",
                "ENG-7 Repair the widget",
                "--body",
                &format!("References ENG-7\n\n{URL}")
            ]
        );
    }
}

#[test]
fn repeated_issues_are_one_issue_and_keep_the_one_issue_pull_request() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details());
    let cli = git(gh(Cli::for_api(&api))).env("LINEAR_TEAM_ID", "ENG");
    cli.run(&["issue", "gh", "pr", "ENG-7", "eng-7", URL, "7"])
        .success();
    assert_eq!(
        cli.calls("gh"),
        [[
            "pr",
            "create",
            "--title",
            "ENG-7 Repair the widget",
            "--body",
            &format!("Fixes ENG-7\n\n{URL}")
        ]]
    );
    assert_eq!(api.operations(), ["GetIssueDetails"]);
}

#[test]
fn several_issues_without_a_title_fail_before_any_request() {
    let api = MockLinear::start();
    let cli = git(gh(jj_refs(
        Cli::for_api(&api),
        false,
        &[("bbb", &["Fixes ENG-7", "Fixes DES-5"])],
    )));
    for args in [
        &["issue", "gh", "pr", "ENG-7", "DES-5"][..],
        &["issue", "gh", "pr", "@jj", "ENG-7"],
    ] {
        let run = cli.run(args);
        run.usage_error()
            .stderr_has("A title is required for a pull request covering several issues: ")
            .stderr_has(
                "Pass --title \"<summary covering all issues>\". The title is used exactly as supplied.",
            );
        assert_eq!(run.stdout, "", "{run}");
    }
    let run = cli.run(&["issue", "gh", "pr", "@jj", "ENG-7"]);
    run.usage_error()
        .stderr_has("covering several issues: DES-5, ENG-7\n");
    assert!(cli.calls("gh").is_empty());
    assert_eq!(
        jj_reads(&cli),
        ["trunk()", "trunk()..@", "trunk()", "trunk()..@"]
    );
    assert!(api.requests().is_empty());
}

#[test]
fn a_reference_naming_several_issues_covers_them_all_with_a_title() {
    let api = MockLinear::start();
    three_issues(&api);
    let cli = git(gh(jj_refs(
        Cli::for_api(&api),
        false,
        &[("bbb", &["Fixes ENG-7", "Fixes DES-5"])],
    )));
    let run = cli.run(&[
        "issue",
        "gh",
        "pr",
        "ENG-256",
        "@jj",
        "--title",
        "All three",
    ]);
    run.success()
        .stderr_has("Using issues DES-5, ENG-7 from Linear-issue trailers in trunk()..@\n");
    assert_eq!(run.stdout, "https://github.com/acme/widgets/pull/1\n");
    assert_eq!(
        cli.calls("gh")[0][3..6],
        [
            "All three".to_owned(),
            "--body".to_owned(),
            format!(
                "Fixes ENG-256, DES-5, and ENG-7\n\n{}\n{}\n{URL}",
                other_url("ENG-256", "Followup"),
                other_url("DES-5", "Design"),
            )
        ]
    );
    assert_eq!(jj_reads(&cli), ["trunk()", "trunk()..@"]);
}

#[test]
fn a_missing_issue_among_several_opens_nothing() {
    let api = MockLinear::start();
    api.on_variables("GetIssueDetails", json!({ "id": "ENG-7" }), details())
        .on_variables_error(
            "GetIssueDetails",
            json!({ "id": "DES-5" }),
            "Entity not found: Issue",
        );
    let cli = git(gh(Cli::for_api(&api)));
    let run = cli.run(&["issue", "gh", "pr", "ENG-7", "DES-5", "--title", "Both"]);
    assert_eq!(run.code, 3, "{run}");
    run.stderr_has("Failed to create pull request: Failed to fetch issue DES-5: ")
        .stderr_has("Issue not found: DES-5")
        .stderr_has("No pull request was opened. Check the issue ID and access, then try again.");
    assert!(!run.stderr.contains("bookmark"), "{run}");
    assert!(cli.calls("gh").is_empty());
}

/// A mock for one successful `issue git start` or `issue jj start` of ENG-7,
/// from "Todo" to "In Progress".
fn started(api: &MockLinear) {
    api.on("GetIssueTransitionDetails", details())
        .on("GetWorkflowStates", states())
        .on(
            "UpdateIssueState",
            json!({ "issueUpdate": { "success": true } }),
        );
}

const MOVED: &str = "✓ Moved Linear issue ENG-7 from \"Todo\" to \"In Progress\"\n";

/// The line saying `@` is left on the change `issue jj start` described.
const ON_DESCRIBED: &str =
    "✓ @ is now the described change; run jj new to edit in an empty change on top\n";

/// What `issue jj start` reports writing for ENG-7 to `change`.
fn described(change: &str) -> String {
    format!(
        "✓ Wrote jj change {change} description: \"ENG-7 Repair the widget\"\n\
         ✓ Wrote trailers (keep Linear-issue when rewording to use @jj):\n\
         \x20 Linear-issue: Fixes ENG-7\n\
         \x20 Linear-issue-url: {URL}\n\
         {ON_DESCRIBED}"
    )
}

/// The `jj describe` calls, as jj received them.
fn describes(cli: &Cli) -> Vec<Vec<String>> {
    calls(cli, "jj")
        .into_iter()
        .filter(|args| args[0] == "describe")
        .collect()
}

#[test]
fn jj_start_says_which_change_it_created_and_what_it_wrote() {
    let api = MockLinear::start();
    started(&api);
    let cli = jj(Cli::for_api(&api));
    let run = cli.run(&["issue", "jj", "start", "ENG-7"]);
    run.success();
    assert_eq!(
        run.stdout,
        format!(
            "✓ Created and switched to jj change {NEW_CHANGE} from {CHANGE}\n{}{MOVED}",
            described(NEW_CHANGE)
        )
    );
    assert!(!run.stderr.contains('✓'), "{run}");
    assert_eq!(
        describes(&cli),
        [[
            "describe".to_owned(),
            "-m".to_owned(),
            format!(
                "ENG-7 Repair the widget\n\nLinear-issue: Fixes ENG-7\nLinear-issue-url: {URL}"
            ),
            "-r".to_owned(),
            "@".to_owned(),
        ]]
    );
    assert!(calls(&cli, "git").is_empty());
}

#[test]
fn jj_start_reuses_only_an_empty_undescribed_change_and_says_so() {
    let api = MockLinear::start();
    started(&api);
    let cli = jj_starting_on(Cli::for_api(&api), &jj_at(CHANGE, "", true), "");
    let run = cli.run(&["issue", "jj", "start", "ENG-7"]);
    run.success();
    assert_eq!(
        run.stdout,
        format!(
            "✓ Reused empty, undescribed jj change {CHANGE}\n{}{MOVED}",
            described(CHANGE)
        )
    );
    assert!(!calls(&cli, "jj").iter().any(|args| args[..] == ["new"]));
    assert_eq!(describes(&cli)[0][4], "@");
    // A described empty change and an undescribed change with files get a
    // new change on top.
    for at in [jj_at(CHANGE, "notes\n", true), jj_at(CHANGE, "", false)] {
        let api = MockLinear::start();
        started(&api);
        let cli = jj_starting_on(Cli::for_api(&api), &at, "");
        cli.run(&["issue", "jj", "start", "ENG-7"])
            .success()
            .stdout_has(&format!(
                "✓ Created and switched to jj change {NEW_CHANGE} from {CHANGE}\n"
            ));
        assert_eq!(describes(&cli)[0][4], "@");
    }
}

#[test]
fn jj_start_keeps_reporting_the_change_when_describing_it_fails() {
    for (at, done, change) in [
        (jj_at(CHANGE, "work\n", false), "created", NEW_CHANGE),
        (jj_at(CHANGE, "", true), "reused", CHANGE),
    ] {
        let api = MockLinear::start();
        api.on("GetIssueTransitionDetails", details());
        let cli = jj_starting_on(
            Cli::for_api(&api),
            &at,
            "echo 'Error: concurrent operation' >&2; exit 1",
        );
        let run = cli.run(&["issue", "jj", "start", "ENG-7"]);
        run.failure()
            .stderr_has("Failed to set jj description")
            .stderr_has("concurrent operation")
            .stderr_has(&format!(
                "The jj change {change} was {done}, but setting its description failed. Inspect it with jj log -r {change} before retrying."
            ));
        let prepared = match done {
            "created" => {
                format!("✓ Created and switched to jj change {NEW_CHANGE} from {CHANGE}\n")
            }
            _ => format!("✓ Reused empty, undescribed jj change {CHANGE}\n"),
        };
        assert_eq!(run.stdout, prepared);
        assert_eq!(api.operations(), ["GetIssueTransitionDetails"]);
    }
}

#[test]
fn jj_start_says_a_change_was_created_when_its_id_cannot_be_read() {
    let api = MockLinear::start();
    api.on("GetIssueTransitionDetails", details());
    let at = jj_at(CHANGE, "work\n", false);
    let cli = Cli::for_api(&api)
        .file("cwd/.jj/repo/store", "")
        .stub_bin(
            "jj",
            &format!(
                "case \"$*\" in\n\
                 'log -r @ --no-graph '*) if [ -e made-new ]; then echo 'Error: concurrent operation' >&2; exit 1; else printf '%s\\n' '{at}'; fi ;;\n\
                 new) touch made-new ;;\n\
                 *) echo \"unexpected jj $*\" >&2; exit 1 ;;\n\
                 esac"
            ),
        );
    let run = cli.run(&["issue", "jj", "start", "ENG-7"]);
    run.failure()
        .stderr_has("Failed to inspect the new jj change")
        .stderr_has("A new jj change was created, but its ID could not be read. Inspect it with jj log -r @ before retrying.");
    assert_eq!(
        run.stdout,
        format!("✓ Created and switched to a new jj change from {CHANGE}\n")
    );
    assert!(describes(&cli).is_empty());
    assert_eq!(api.operations(), ["GetIssueTransitionDetails"]);
}

#[test]
fn jj_start_refuses_a_probe_it_cannot_read_before_changing_anything() {
    let one = jj_at(CHANGE, "", true);
    for probe in [
        format!("'{one}' '{one}'"),
        "'occupied'".to_owned(),
        "''".to_owned(),
        format!(
            "'{}'",
            json!([{ "change_id": " ", "description": "" }, true])
        ),
    ] {
        let api = MockLinear::start();
        api.on("GetIssueTransitionDetails", details());
        let cli = Cli::for_api(&api).file("cwd/.jj/repo/store", "").stub_bin(
            "jj",
            &format!(
                "case \"$*\" in\n\
                     'log -r @ --no-graph '*) printf '%s\\n' {probe} ;;\n\
                     *) echo \"unexpected jj $*\" >&2; exit 1 ;;\n\
                     esac"
            ),
        );
        let run = cli.run(&["issue", "jj", "start", "ENG-7"]);
        run.failure().stderr_has("Unexpected output from `jj log`");
        assert_eq!(run.stdout, "", "{probe}");
        assert_eq!(calls(&cli, "jj").len(), 1, "{probe}");
        assert_eq!(api.operations(), ["GetIssueTransitionDetails"]);
    }
}

#[test]
fn jj_start_shows_a_hostile_title_escaped_and_writes_it_as_it_is() {
    let title = "Say \"hi\"\n\u{1b}[2Jnow \u{9b}31m café";
    let mut issue = details();
    issue["issue"]["title"] = json!(title);
    let api = MockLinear::start();
    api.on("GetIssueTransitionDetails", issue)
        .on("GetWorkflowStates", states())
        .on(
            "UpdateIssueState",
            json!({ "issueUpdate": { "success": true } }),
        );
    let cli = jj(Cli::for_api(&api));
    let run = cli.run(&["issue", "jj", "start", "ENG-7"]);
    run.success().stdout_has(&format!(
        "✓ Wrote jj change {NEW_CHANGE} description: \"ENG-7 Say \\\"hi\\\"\\n\\u001b[2Jnow \u{fffd}31m café\"\n"
    ));
    assert!(!run.stdout.contains('\u{1b}'), "{run}");
    assert!(!run.stdout.contains('\u{9b}'), "{run}");
    assert_eq!(
        describes(&cli)[0][2],
        format!("ENG-7 {title}\n\nLinear-issue: Fixes ENG-7\nLinear-issue-url: {URL}")
    );
}

#[test]
fn jj_start_keeps_its_lines_when_starting_the_issue_fails() {
    let api = MockLinear::start();
    api.on("GetIssueTransitionDetails", details())
        .on_error("GetWorkflowStates", "Team not found");
    let cli = jj(Cli::for_api(&api));
    let run = cli.run(&["issue", "jj", "start", "ENG-7"]);
    run.failure()
        .stderr_has("Could not move the issue to a started status")
        .stderr_has("The jj change is ready");
    assert_eq!(
        run.stdout,
        format!(
            "✓ Created and switched to jj change {NEW_CHANGE} from {CHANGE}\n{}",
            described(NEW_CHANGE)
        )
    );
}

/// A git checked out on `current` (detached at abcdef012345 when empty),
/// where the branches `existing` exist; `HEAD` is 0123456789ab.
fn git_repo(cli: Cli, current: &str, existing: &[&str]) -> Cli {
    let current = if current.is_empty() {
        "exit 1".to_owned()
    } else {
        format!("echo '{current}'")
    };
    let existing: Vec<String> = existing
        .iter()
        .map(|branch| format!("'refs/heads/{branch}'"))
        .collect();
    let existing = if existing.is_empty() {
        "'-'".to_owned()
    } else {
        existing.join(" | ")
    };
    cli.stub_bin(
        "git",
        &format!(
            "case \"$1 $2\" in\n\
             'show-ref --verify') case \"$4\" in {existing}) exit 0 ;; *) exit 1 ;; esac ;;\n\
             'symbolic-ref --quiet') {current} ;;\n\
             'rev-parse --verify') case \"$6\" in\n\
               'HEAD^{{commit}}') echo 0123456789ab ;;\n\
               *) exit 1 ;;\n\
               esac ;;\n\
             'checkout -b' | 'checkout '*) ;;\n\
             *) exit 1 ;;\n\
             esac"
        ),
    )
}

#[test]
fn git_start_from_a_detached_head_names_the_commit_it_left() {
    let api = MockLinear::start();
    started(&api);
    let cli = git_repo(Cli::for_api(&api), "", &[]);
    let run = cli.run(&["issue", "git", "start", "ENG-7"]);
    run.success();
    assert_eq!(
        run.stdout,
        format!(
            "✓ Created and switched to git branch \"eng-7-repair-the-widget\" from \"HEAD\" (0123456789ab); previous HEAD: detached at 0123456789ab\n{MOVED}"
        )
    );
}

#[test]
fn git_start_with_a_base_that_names_no_commit_fails_before_checking_out() {
    let api = MockLinear::start();
    api.on("GetIssueTransitionDetails", details());
    let cli = git(Cli::for_api(&api));
    let run = cli.run(&["issue", "git", "start", "ENG-7", "--from-ref", "nope"]);
    run.usage_error()
        .stderr_has(
            "Cannot create branch 'eng-7-repair-the-widget' from \"nope\": it names no commit",
        )
        .stderr_has("Pass --from-ref a branch, tag or commit that exists");
    assert_eq!(run.stdout, "");
    assert!(!calls(&cli, "git").iter().any(|args| args[0] == "checkout"));
    assert_eq!(api.operations(), ["GetIssueTransitionDetails"]);
}

#[test]
fn git_start_on_an_existing_branch_says_what_it_switched_from() {
    let branch = "eng-7-repair-the-widget";
    for (current, from_ref, said) in [
        (
            "main",
            None,
            format!("✓ Switched from git branch \"main\" to \"{branch}\"\n"),
        ),
        (
            "",
            None,
            format!("✓ Switched from detached git HEAD at 0123456789ab to branch \"{branch}\"\n"),
        ),
        (
            branch,
            None,
            format!("✓ Kept git branch \"{branch}\" checked out\n"),
        ),
        (
            "main",
            Some("origin/main"),
            format!(
                "✓ Switched from git branch \"main\" to \"{branch}\"\n\
                 ✓ Left existing branch history unchanged; --from-ref \"origin/main\" was not used\n"
            ),
        ),
    ] {
        let api = MockLinear::start();
        started(&api);
        let cli = git_repo(Cli::for_api(&api), current, &[branch]);
        let mut args = vec!["issue", "git", "start", "ENG-7"];
        if let Some(from_ref) = from_ref {
            args.extend(["--from-ref", from_ref]);
        }
        let run = cli.run_tty(&args, &[("Existing branch:", "\r")]);
        run.success()
            .stdout_has(&format!("{said}{}", MOVED.trim_end()));
        assert!(
            calls(&cli, "git").contains(&vec!["checkout".to_owned(), branch.to_owned()]),
            "{:?}",
            cli.calls("git")
        );
        // An unused base is never resolved.
        assert!(
            !calls(&cli, "git")
                .iter()
                .any(|args| args.last().is_some_and(|arg| arg == "origin/main^{commit}")),
            "{:?}",
            cli.calls("git")
        );
    }
}

#[test]
fn git_start_names_the_suffixed_branch_it_created() {
    let api = MockLinear::start();
    started(&api);
    let cli = git_repo(Cli::for_api(&api), "main", &["eng-7-repair-the-widget"]);
    let run = cli.run_tty(
        &["issue", "git", "start", "ENG-7"],
        &[("Existing branch:", "\x1b[B\r")],
    );
    run.success().stdout_has(
        "✓ Created and switched to git branch \"eng-7-repair-the-widget-1\" from \"HEAD\" (0123456789ab); previous branch: \"main\"",
    );
    assert!(
        calls(&cli, "git").contains(&vec![
            "checkout".to_owned(),
            "-b".to_owned(),
            "eng-7-repair-the-widget-1".to_owned(),
            "HEAD".to_owned()
        ]),
        "{:?}",
        cli.calls("git")
    );
}

#[test]
fn start_help_lists_the_steps_the_output_reports() {
    let cli = Cli::new();
    cli.run(&["issue", "git", "start", "--help"])
        .success()
        .stdout_has(
            "Create or switch to a git branch, then start the issue and report both results",
        )
        .stdout_has("1. Creates the branch Linear suggests")
        .stdout_has("2. Moves the issue to its team's first started status")
        .stdout_has("does not use\n   --from-ref")
        .stdout_has("unused when switching to an existing branch")
        .stdout_has("\"HEAD\" (0123456789ab); previous branch: \"main\"\n");
    cli.run(&["issue", "jj", "start", "--help"])
        .success()
        .stdout_has(
            "Describe a new or reused jj change, then start the issue and report both results",
        )
        .stdout_has("1. Creates a new change on the current one (`jj new`)")
        .stdout_has("2. Writes \"<ID> <issue title>\"")
        .stdout_has(
            "Keep the Linear-issue trailer when rewording if you want @jj to\n   find the issue.",
        )
        .stdout_has(
            "✓ @ is now the described change; run jj new to edit in an empty change on top\n",
        )
        .stdout_has("3. Moves the issue")
        .stdout_has("No bookmark is created and nothing is pushed.");
}

#[test]
fn describe_and_jj_start_share_the_configured_commit_text_and_real_issue_description() {
    let api = MockLinear::start();
    let mut reply = details();
    let description = "Details {{ inert }} {% include '/etc/passwd' %}";
    reply["issue"]["description"] = json!(description);
    api.on("GetIssueDetails", reply.clone())
        .on("GetIssueDetails", reply.clone())
        .on("GetIssueTransitionDetails", reply)
        .on("GetWorkflowStates", states())
        .on(
            "UpdateIssueState",
            json!({"issueUpdate": {"success": true}}),
        );
    let cli = jj(Cli::for_api(&api)).file("cwd/linear.toml", r#"[templates]
commit_description = "{{ issue.identifier }}: {{ issue.title }}\n\n{{ issue.description }}\n{{ linkage }} {{ issue.url }}\n"
"#);
    let expected = format!("ENG-7: Repair the widget\n\n{description}\nFixes {URL}\n");
    let run = cli.run(&["issue", "describe", "ENG-7"]);
    run.success();
    assert_eq!(run.stdout, expected);
    let run = cli.run(&["issue", "describe", "ENG-7", "--references"]);
    run.success();
    assert_eq!(run.stdout, expected.replace("Fixes", "References"));
    let run = cli.run(&["issue", "jj", "start", "ENG-7"]);
    run.success().stdout_has("Details {{ inert }}");
    assert_eq!(describes(&cli)[0][2], expected.trim_end_matches('\n'));
    assert!(!run.stdout.contains("Wrote trailers"), "{run}");
    assert!(
        api.requests()
            .iter()
            .filter(|request| matches!(
                request.operation.as_deref(),
                Some("GetIssueDetails" | "GetIssueTransitionDetails")
            ))
            .all(|request| request.query.contains("description"))
    );
}

#[test]
fn jj_start_reports_the_custom_trailers_it_actually_wrote() {
    let api = MockLinear::start();
    started(&api);
    let cli = jj(Cli::for_api(&api)).file("cwd/linear.toml", r#"[templates]
commit_description = "Custom {{ issue.identifier }}\n\nLinear-issue: References OPS-4\nLinear-issue-url: https://linear.app/acme/issue/OPS-4/other\n"
"#);
    let run = cli.run(&["issue", "jj", "start", "ENG-7"]);
    run.success()
        .stdout_has("description: \"Custom ENG-7\"")
        .stdout_has("  Linear-issue: References OPS-4\n")
        .stdout_has("  Linear-issue-url: https://linear.app/acme/issue/OPS-4/other\n");
    assert!(!run.stdout.contains("Linear-issue: Fixes ENG-7"), "{run}");
    assert_eq!(
        describes(&cli)[0][2],
        "Custom ENG-7\n\nLinear-issue: References OPS-4\nLinear-issue-url: https://linear.app/acme/issue/OPS-4/other"
    );
}

#[test]
fn blank_or_runtime_failed_commit_templates_print_nothing_and_prepare_no_jj_change() {
    for configuration in [
        "[templates]\ncommit_description = '   '",
        "[templates]\ncommit_description = 'prefix {% include \"broken\" %}'\n[template-aliases]\nbroken = '{{ issue[\"missing\"] }}'",
    ] {
        let api = MockLinear::start();
        api.on("GetIssueDetails", details())
            .on("GetIssueTransitionDetails", details());
        let cli = jj(Cli::for_api(&api)).file("cwd/linear.toml", configuration);
        for command in [
            &["issue", "describe", "ENG-7"][..],
            &["issue", "jj", "start", "ENG-7"],
        ] {
            let run = cli.run(command);
            run.failure();
            assert_eq!(run.stdout, "", "{run}");
        }
        assert!(calls(&cli, "jj").is_empty());
        assert_eq!(
            api.operations(),
            ["GetIssueDetails", "GetIssueTransitionDetails"]
        );
    }
}

#[test]
fn invalid_commit_configuration_fails_before_fetching_or_preparing_anything() {
    for body in ["{% if %}", "{{ issue.missing }}"] {
        let api = MockLinear::start();
        let cli = jj(Cli::for_api(&api));
        let configuration = format!("templates.commit_description={body:?}");
        let run = cli.run(&["--config", &configuration, "issue", "jj", "start", "ENG-7"]);
        run.failure().stderr_has("templates.commit_description");
        assert_eq!(run.stdout, "", "{run}");
        assert!(calls(&cli, "jj").is_empty());
        assert!(api.requests().is_empty());
    }
}

#[test]
fn configured_pull_request_templates_use_all_distinct_issues_and_description_data() {
    let api = MockLinear::start();
    let mut first = details();
    first["issue"]["description"] = json!("remote {{ inert }}");
    let mut second = other("DES-5", "Design");
    second["issue"]["description"] = json!("Design details");
    api.on_variables("GetIssueDetails", json!({"id": "ENG-7"}), first)
        .on_variables("GetIssueDetails", json!({"id": "DES-5"}), second);
    let cli = gh(Cli::for_api(&api)).file("cwd/linear.toml", r#"[templates]
pr_title = '{% for issue in issues %}{{ issue.identifier }}{% if not loop.last %} + {% endif %}{% endfor %}'
pr_body = "  {{ description }}\n{% for issue in issues %}{{ issue.title }}: {{ issue.description }}\n{% endfor %}{% if references %}Nonclosing\n{% endif %}{% include 'pr_issue_links' %}\n  "
"#);
    let run = cli.run(&[
        "issue",
        "gh",
        "pr",
        "ENG-7",
        "DES-5",
        "ENG-7",
        "--body",
        "Given {{ literal }}\n \n",
        "--references",
    ]);
    run.success();
    assert_eq!(
        cli.calls("gh"),
        [vec![
            "pr".to_owned(),
            "create".to_owned(),
            "--title".to_owned(),
            "ENG-7 + DES-5".to_owned(),
            "--body".to_owned(),
            format!(
                "  Given {{{{ literal }}}}\nRepair the widget: remote {{{{ inert }}}}\nDesign: Design details\nNonclosing\nReferences ENG-7 and DES-5\n\n{URL}\n{}\n  ",
                other_url("DES-5", "Design")
            ),
        ]]
    );
    assert_eq!(api.requests().len(), 2);
}

#[test]
fn an_explicit_title_bypasses_the_title_template_and_gh_passthrough_remains_last() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details());
    let cli = gh(Cli::for_api(&api)).file(
        "cwd/linear.toml",
        r#"[templates]
pr_title = '{{ issues[0]["missing"] }}'
pr_body = 'Rendered {{ description }}'
"#,
    );
    cli.run(&[
        "issue",
        "gh",
        "pr",
        "ENG-7",
        "--title",
        " Manual {{ inert }} ",
        "--body",
        "input",
        "--",
        "--title",
        "gh title",
        "--body",
        "gh body",
    ])
    .success();
    assert_eq!(
        cli.calls("gh"),
        [[
            "pr",
            "create",
            "--title",
            " Manual {{ inert }} ",
            "--body",
            "Rendered input",
            "--title",
            "gh title",
            "--body",
            "gh body",
        ]]
    );
}

#[test]
fn configured_pr_body_receives_file_input_and_can_replace_the_whole_body() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details());
    let cli = gh(Cli::for_api(&api)).file("cwd/body.md", "From file\n\n");
    let run = cli.run(&[
        "--config",
        "templates.pr_body='Only {{ description }}'",
        "issue",
        "gh",
        "pr",
        "ENG-7",
        "--body-file",
        "body.md",
    ]);
    run.success();
    assert_eq!(cli.calls("gh")[0][5], "Only From file");
}

#[test]
fn blank_or_runtime_failed_pr_text_never_runs_gh() {
    for configuration in [
        "[templates]\npr_title = ' '",
        "[templates]\npr_body = ' '",
        "[templates]\npr_title = 'prefix {{ issues[0][\"missing\"] }}'",
        "[templates]\npr_body = 'prefix {% include \"broken\" %}'\n[template-aliases]\nbroken = '{{ issues[0][\"missing\"] }}'",
    ] {
        let api = MockLinear::start();
        api.on("GetIssueDetails", details());
        let cli = gh(Cli::for_api(&api)).file("cwd/linear.toml", configuration);
        let run = cli.run(&["issue", "gh", "pr", "ENG-7"]);
        run.failure();
        assert_eq!(run.stdout, "", "{run}");
        assert!(cli.calls("gh").is_empty());
        assert_eq!(api.operations(), ["GetIssueDetails"]);
    }
}

#[test]
fn describe_sanitizes_remote_fields_for_a_terminal_and_keeps_description_lines() {
    let api = MockLinear::start();
    let mut issue = details();
    issue["issue"]["title"] = json!("first\nsecond");
    issue["issue"]["description"] = json!("one\n\u{1b}[2Jtwo");
    issue["issue"]["url"] = json!(format!("{URL}\u{1b}[2J"));
    api.on("GetIssueDetails", issue);
    let cli = Cli::for_api(&api).file(
        "cwd/linear.toml",
        r#"[templates]
commit_description = "{{ issue.title }}\n{{ issue.description }}\n{{ issue.url }}\n"
"#,
    );
    let run = cli.run_tty(&["issue", "describe", "ENG-7"], &[]);
    run.success().stdout_has(&format!(
        "first second\none\n\u{fffd}[2Jtwo\n{URL}\u{fffd}[2J\n"
    ));
}
