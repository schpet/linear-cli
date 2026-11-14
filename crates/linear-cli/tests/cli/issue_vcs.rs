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
        assert!(run.stdout.ends_with(&format!("{URL}\n{said}")), "{run}");
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
    let cli = gh(Cli::for_api(&api));
    cli.run(&["issue", "gh", "pull-request", "ENG-7"]).success();
    assert_eq!(
        cli.calls("gh"),
        [[
            "pr",
            "create",
            "--title",
            "ENG-7 Repair the widget",
            "--body",
            URL
        ]]
    );
    assert_eq!(api.variables("GetIssueDetails"), json!({ "id": "ENG-7" }));
}

#[test]
fn pull_request_forwards_title_base_head_and_draft() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details());
    let cli = git(gh(Cli::for_api(&api)));
    cli.run(&[
        "issue", "gh", "pr", "ENG-7", "--title", "Custom", "--base", "main", "--head", "topic",
        "--draft",
    ])
    .success();
    let args = cli.calls("gh").remove(0);
    assert_eq!(
        args[..6],
        ["pr", "create", "--title", "ENG-7 Custom", "--body", URL]
    );
    let flags = &args[6..];
    for expected in [["--base", "main"], ["--head", "topic"]] {
        assert!(flags.windows(2).any(|pair| pair == expected), "{args:?}");
    }
    assert!(flags.contains(&"--draft".to_owned()), "{args:?}");
}

#[test]
fn pull_request_body_starts_with_the_template() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details())
        .on("GetIssueDetails", details());
    let cli = gh(Cli::for_api(&api))
        .file("cwd/pr.md", "## Summary\n")
        .file("cwd/.linear.toml", "pr_template = \"pr.md\"\n");
    cli.run(&["issue", "gh", "pr", "ENG-7"]).success();
    cli.run(&["issue", "gh", "pr", "ENG-7", "--no-template"])
        .success();
    let calls = cli.calls("gh");
    assert_eq!(calls[0][5], format!("## Summary\n\n{URL}"));
    assert_eq!(calls[1][5], URL);
}

#[test]
fn pull_request_with_a_missing_template_fails_before_any_request() {
    let api = MockLinear::start();
    let cli = gh(Cli::for_api(&api));
    cli.run(&["issue", "gh", "pr", "ENG-7", "--template", "missing.md"])
        .failure()
        .stderr_has("missing.md");
    assert!(api.requests().is_empty());
    assert!(cli.calls("gh").is_empty());
}

#[test]
fn pull_request_fails_with_status_1_when_gh_fails_and_130_when_it_is_cancelled() {
    for (script, code) in [
        ("echo 'To get started, run: gh auth login' >&2; exit 4", 1),
        ("exit 2", 130),
    ] {
        let api = MockLinear::start();
        api.on("GetIssueDetails", details());
        let run = Cli::for_api(&api)
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
fn pull_request_with_an_empty_title_uses_the_issue_title() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details());
    let cli = gh(Cli::for_api(&api));
    cli.run(&["issue", "gh", "pr", "ENG-7", "--title", ""])
        .success();
    assert_eq!(cli.calls("gh")[0][3], "ENG-7 Repair the widget");
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

/// The jj stub's answer for one commit: its ID, parents, description, and
/// whether it is empty.
fn jj_commit(id: &str, parent: &str, description: &str, empty: bool) -> String {
    json!([
        { "commit_id": id, "parents": [parent], "description": description },
        empty, false
    ])
    .to_string()
}

/// A jj repository for `issue jj pr`: an empty scratch `@` (`aaa`) on the
/// described change `bbb`, itself on the trunk change `ccc`, with the remote
/// `origin`. Local bookmarks are the lines of `cwd/bookmarks`, which
/// `bookmark create` appends to. A push leaves `cwd/pushed` behind, and the
/// gh stub refuses to run before it exists.
fn jj_repo(cli: Cli, bookmarks: &str) -> Cli {
    jj_repo_naming(cli, bookmarks, &["Fixes ENG-7"])
}

/// [`jj_repo`] whose described change `bbb` has these `Linear-issue`
/// trailer values.
fn jj_repo_naming(cli: Cli, bookmarks: &str, linear_issue: &[&str]) -> Cli {
    let scratch = jj_commit("aaa", "bbb", "", true);
    let work = jj_commit("bbb", "ccc", "ENG-7 Repair the widget\n", false);
    let trunk = jj_commit("ccc", "ddd", "trunk\n", false);
    let scratch_change = jj_change("aaa", &[]);
    let work_change = jj_change("bbb", linear_issue);
    cli.file("cwd/bookmarks", bookmarks)
        .stub_bin(
            "jj",
            &format!(
                "case \"$*\" in\n\
                 *'log -r @ '*) printf '%s\\n' '{scratch}' ;;\n\
                 *'log -r bbb '*) printf '%s\\n' '{work}' ;;\n\
                 *'log -r ccc '*) printf '%s\\n' '{trunk}' ;;\n\
                 *'log -r trunk() '*) echo false ;;\n\
                 *'log -r trunk()..@ '*) printf '%s\\n' '{scratch_change}' '{work_change}' ;;\n\
                 *'log -r (remote_bookmarks'*) cat unpushable 2>/dev/null || true ;;\n\
                 *'config list git.push '*) ;;\n\
                 *'git remote list') echo 'origin git@github.com:acme/widgets.git' ;;\n\
                 *'bookmark list '*) cat bookmarks ;;\n\
                 *'bookmark create -r '*) for name; do :; done\n\
                   printf '{{\"name\":\"%s\",\"target\":[\"%s\"]}}\\n' \"$name\" \"$8\" >> bookmarks ;;\n\
                 *'git push '*) echo 'Changes to push to origin:' >&2; touch pushed ;;\n\
                 *) echo \"unexpected jj $*\" >&2; exit 1 ;;\n\
                 esac"
            ),
        )
        .stub_bin(
            "gh",
            "test -e pushed || { echo 'gh ran before the push' >&2; exit 1; }\n\
             echo https://github.com/acme/widgets/pull/1",
        )
        .stub_bin("git", "exit 1")
}

/// What `calls` recorded for jj, without the global flags every call starts with.
fn jj_calls(cli: &Cli) -> Vec<String> {
    calls(cli, "jj")
        .into_iter()
        .map(|args| {
            let skip = if args[0] == "--ignore-working-copy" {
                4
            } else {
                3
            };
            args[skip..].join(" ")
        })
        .collect()
}

/// gh calls other than the `--version` check that precedes any push.
fn gh_calls(cli: &Cli) -> Vec<Vec<String>> {
    cli.calls("gh")
        .into_iter()
        .filter(|args| args[..] != ["--version"])
        .collect()
}

const REPO: &str = "git@github.com:acme/widgets.git";

#[test]
fn jj_pull_request_creates_and_pushes_a_bookmark_for_the_parent_of_a_scratch_change() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details());
    let cli = jj_repo(Cli::for_api(&api), "");
    let run = cli.run(&["issue", "jj", "pr", "ENG-7", "--draft"]);
    run.success()
        .stderr_has("Creating jj bookmark eng-7-repair-the-widget\n")
        .stderr_has("Pushing jj bookmark eng-7-repair-the-widget to origin\n")
        .stderr_has("Changes to push to origin:");
    assert_eq!(run.stdout, "https://github.com/acme/widgets/pull/1\n");
    let jj = jj_calls(&cli);
    assert!(jj[0].starts_with("log -r @ --no-graph"), "{jj:?}");
    assert!(
        !jj.iter().any(|call| call.contains("trunk()")),
        "a named issue needs no trunk: {jj:?}"
    );
    assert_eq!(
        calls(&cli, "jj")[0][..3],
        ["--no-pager", "--color", "never"],
        "only the first call snapshots the working copy"
    );
    assert_eq!(
        jj[jj.len() - 3..],
        [
            "bookmark create -r bbb -- eng-7-repair-the-widget",
            "bookmark list -T json(self) ++ \"\\n\"",
            "git push --remote origin --bookmark exact:eng-7-repair-the-widget",
        ]
    );
    assert_eq!(
        gh_calls(&cli),
        [[
            "pr",
            "create",
            "--title",
            "ENG-7 Repair the widget",
            "--body",
            URL,
            "--repo",
            REPO,
            "--head",
            "eng-7-repair-the-widget",
            "--draft"
        ]]
    );
    assert!(calls(&cli, "git").is_empty(), "gh needs no git checkout");
}

#[test]
fn jj_pull_request_reuses_a_bookmark_on_the_head_and_pushes_it() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details());
    let cli = jj_repo(
        Cli::for_api(&api),
        "{\"name\":\"mine\",\"target\":[\"bbb\"]}\n\
         {\"name\":\"mine\",\"remote\":\"origin\",\"target\":[\"bbb\"]}\n\
         {\"name\":\"elsewhere\",\"target\":[\"ccc\"]}\n",
    );
    cli.run(&["issue", "jj", "pull-request", "ENG-7", "--base", "main"])
        .success();
    let jj = jj_calls(&cli);
    assert!(
        !jj.iter().any(|call| call.starts_with("bookmark create")),
        "{jj:?}"
    );
    assert_eq!(
        jj.last().map(String::as_str),
        Some("git push --remote origin --bookmark exact:mine")
    );
    let gh = gh_calls(&cli).remove(0);
    assert_eq!(
        gh[6..],
        ["--repo", REPO, "--base", "main", "--head", "mine"]
    );
}

#[test]
fn jj_pull_request_takes_head_as_a_bookmark_existing_or_new() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details())
        .on("GetIssueDetails", details());
    // An existing bookmark decides the revision, even one on trunk that jj
    // treats as immutable: whether there is anything to merge is gh's call.
    let cli = jj_repo(
        Cli::for_api(&api),
        "{\"name\":\"release\",\"target\":[\"ccc\"]}\n",
    );
    cli.run(&["issue", "jj", "pr", "ENG-7", "--head", "release"])
        .success();
    let jj = jj_calls(&cli);
    assert!(
        !jj.iter().any(|call| call.starts_with("bookmark create")),
        "{jj:?}"
    );
    assert_eq!(
        jj.last().map(String::as_str),
        Some("git push --remote origin --bookmark exact:release")
    );
    assert_eq!(
        gh_calls(&cli)[0][6..],
        ["--repo", REPO, "--head", "release"]
    );
    // A new name is created on the automatic head.
    cli.run(&["issue", "jj", "pr", "ENG-7", "--head", "topic"])
        .success();
    assert!(
        jj_calls(&cli).contains(&"bookmark create -r bbb -- topic".to_owned()),
        "{:?}",
        jj_calls(&cli)
    );
    assert_eq!(gh_calls(&cli)[1][6..], ["--repo", REPO, "--head", "topic"]);
    cli.run(&["issue", "jj", "pr", "ENG-7", "--head", "someone:topic"])
        .usage_error()
        .stderr_has("Cannot open a pull request from someone:topic with jj");
    assert_eq!(api.operations().len(), 2, "refused before any request");
}

#[test]
fn jj_pull_request_stops_before_publishing_when_the_bookmark_is_unclear() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details())
        .on("GetIssueDetails", details());
    for (bookmarks, said) in [
        (
            "{\"name\":\"one\",\"target\":[\"bbb\"]}\n{\"name\":\"two\",\"target\":[\"bbb\"]}\n",
            "Several jj bookmarks point at the pull request revision: one, two",
        ),
        (
            "{\"name\":\"eng-7-repair-the-widget\",\"target\":[\"ccc\"]}\n",
            "The jj bookmark eng-7-repair-the-widget already exists at another revision",
        ),
    ] {
        let cli = jj_repo(Cli::for_api(&api), bookmarks);
        cli.run(&["issue", "jj", "pr", "ENG-7"])
            .usage_error()
            .stderr_has(said);
        let jj = jj_calls(&cli);
        assert!(
            !jj.iter()
                .any(|call| call.starts_with("bookmark create") || call.starts_with("git push")),
            "{jj:?}"
        );
        assert!(gh_calls(&cli).is_empty());
    }
}

#[test]
fn jj_pull_request_refuses_what_jj_would_not_push() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details());
    // An undescribed change under the head stops it before a bookmark exists.
    let cli = jj_repo(Cli::for_api(&api), "").file("cwd/unpushable", "0123456789ab\n");
    cli.run(&["issue", "jj", "pr", "ENG-7"])
        .usage_error()
        .stderr_has("jj will not push the pull request: 0123456789ab under it has no description");
    let jj = jj_calls(&cli);
    assert!(
        !jj.iter().any(|call| call.starts_with("bookmark create")),
        "{jj:?}"
    );
    assert!(cli.calls("gh").iter().all(|args| args[..] == ["--version"]));
}

#[test]
fn jj_pull_request_needs_gh_before_it_pushes_anything() {
    let api = MockLinear::start();
    let cli = jj_repo(Cli::for_api(&api), "").stub_bin("gh", "exit 127");
    std::fs::remove_file(cli.path("bin/gh")).expect("remove the gh stub");
    cli.run(&["issue", "jj", "pr", "ENG-7"])
        .failure()
        .stderr_has("Could not find `gh`");
    assert!(calls(&cli, "jj").is_empty());
    assert!(api.requests().is_empty());
}

#[test]
fn jj_pull_request_without_credentials_fails_before_touching_the_repository() {
    let cli = jj_repo(Cli::new(), "");
    cli.run(&["issue", "jj", "pr", "ENG-7"]).auth_failure();
    assert!(calls(&cli, "jj").is_empty(), "{:?}", cli.calls("jj"));
    assert!(cli.calls("gh").is_empty(), "{:?}", cli.calls("gh"));
    // Naming the issue with a reference still needs no credentials.
    cli.run(&["issue", "id", "@jj"])
        .success()
        .stdout_has("ENG-7");
}

#[test]
fn jj_pull_request_does_not_run_gh_when_the_push_fails() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details());
    let cli = jj_repo(Cli::for_api(&api), "").stub_bin(
        "jj",
        &format!(
            "case \"$*\" in\n\
             *'log -r @ '*) printf '%s\\n' '{}' ;;\n\
             *'config list '*) ;;\n\
             *'git remote list') echo 'origin https://github.com/acme/widgets.git' ;;\n\
             *'bookmark list '*) echo '{{\"name\":\"mine\",\"target\":[\"aaa\"]}}' ;;\n\
             *'log -r (remote_bookmarks'*) ;;\n\
             *) echo 'Error: remote rejected' >&2; exit 1 ;;\n\
             esac",
            jj_commit("aaa", "bbb", "ENG-7 work\n", false)
        ),
    );
    let run = cli.run(&["issue", "jj", "pr", "ENG-7"]);
    assert_eq!(run.code, 1, "{run}");
    run.stderr_has("Failed to push the jj bookmark mine to origin")
        .stderr_has("remote rejected")
        .stderr_has("No pull request was opened");
    assert!(gh_calls(&cli).is_empty());
}

#[test]
fn jj_pull_request_needs_a_remote_and_a_repository_before_anything_is_published() {
    let api = MockLinear::start();
    // Every issue is fetched before the repository is surveyed.
    for _ in 0..3 {
        api.on("GetIssueDetails", details());
    }
    for (remotes, said) in [
        ("", "This jj repository has no git remote to push to"),
        (
            "echo 'origin /srv/git/widgets.git'",
            "Cannot tell which GitHub repository the jj remote origin is",
        ),
    ] {
        let cli = jj_repo(Cli::for_api(&api), "").stub_bin(
            "jj",
            &format!(
                "case \"$*\" in\n\
                 *'log -r @ '*) printf '%s\\n' '{}' ;;\n\
                 *'git remote list') {remotes} ;;\n\
                 esac",
                jj_commit("aaa", "bbb", "ENG-7 work\n", false)
            ),
        );
        cli.run(&["issue", "jj", "pr", "ENG-7"])
            .failure()
            .stderr_has(said);
        assert!(gh_calls(&cli).is_empty());
    }
    gh(Cli::for_api(&api))
        .stub_bin(
            "jj",
            "echo 'Error: There is no jj repo in \".\"' >&2; exit 1",
        )
        .run(&["issue", "jj", "pr", "ENG-7"])
        .failure()
        .stderr_has("Not in a jj repository")
        .stderr_has("use `linear issue gh pr` instead");
    assert_eq!(api.operations(), ["GetIssueDetails"; 3]);
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
            .stderr_has("linear issue gh pr")
            .stderr_has("linear issue jj pr");
    }
    assert!(api.requests().is_empty());
    assert!(gh_calls(&cli).is_empty());
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
    for tool in ["gh", "jj"] {
        let cli = gh(repo_naming_an_issue(&api));
        for args in [
            &["issue", tool, "pr", "--draft"][..],
            &["issue", tool, "pull-request", "--template", "missing.md"],
        ] {
            let run = cli.run(args);
            run.usage_error()
                .stderr_has("No issue to open a pull request for")
                .stderr_has(&format!("linear issue {tool} pr ENG-123"))
                .stderr_has("`@git`")
                .stderr_has("`@jj`");
            assert_eq!(run.stdout, "", "{run}");
        }
        assert!(calls(&cli, "git").is_empty(), "{:?}", cli.calls("git"));
        assert!(calls(&cli, "jj").is_empty(), "{:?}", cli.calls("jj"));
        assert!(cli.calls("gh").is_empty());
        let help = cli.run(&["issue", tool, "pr", "--help"]);
        help.success()
            .stdout_has(&format!(
                "Usage: linear issue {tool} pull-request [OPTIONS] <ISSUE>"
            ))
            .stdout_has("@git");
    }
    assert!(api.requests().is_empty());
}

#[test]
fn the_pull_request_commands_describe_their_own_head() {
    let cli = Cli::new();
    let gh = cli.run(&["issue", "gh", "pr", "--help"]);
    gh.success()
        .stdout_has("--head <BRANCH>")
        .stdout_has("accepts owner:branch")
        .stdout_has("use `linear issue jj pr`");
    assert!(!gh.stdout.contains("vcs"), "{gh}");
    let jj = cli.run(&["issue", "jj", "pull-request", "--help"]);
    jj.success()
        .stdout_has("--head <BOOKMARK>")
        .stdout_has("jj git push")
        .stdout_has("trunk()..@ even when");
    assert!(!jj.stdout.contains("vcs"), "{jj}");
    cli.run(&["issue", "jj", "--help"])
        .success()
        .stdout_has("  start ")
        .stdout_has("  pull-request ");
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
        .stub_bin("git", "echo 'fatal: not a git repository' >&2; exit 128")
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
            URL,
            "--head",
            "someone:ops-1-other",
            "--draft"
        ]
    );
}

#[test]
fn jj_pull_request_takes_the_issue_from_at_jj_and_snapshots_once() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details());
    let cli = jj_repo(Cli::for_api(&api), "");
    let run = cli.run(&["issue", "jj", "pr", "@jj"]);
    run.success()
        .stderr_has("Using issue ENG-7 from Linear-issue trailers in trunk()..@\n");
    let snapshots = calls(&cli, "jj")
        .into_iter()
        .filter(|args| args[0] != "--ignore-working-copy")
        .count();
    assert_eq!(
        snapshots,
        1,
        "only the survey snapshots: {:?}",
        cli.calls("jj")
    );
    let jj = jj_calls(&cli);
    assert!(jj[0].starts_with("log -r trunk() "), "{jj:?}");
    assert!(jj[1].starts_with("log -r trunk()..@ "), "{jj:?}");
    assert_eq!(gh_calls(&cli).len(), 1);
}

#[test]
fn jj_pull_request_reads_at_jj_from_the_working_copy_even_when_head_is_elsewhere() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details());
    let cli = jj_repo(
        Cli::for_api(&api),
        "{\"name\":\"elsewhere\",\"target\":[\"ccc\"]}\n",
    );
    cli.run(&["issue", "jj", "pr", "@jj", "--head", "elsewhere"])
        .success()
        .stderr_has("Using issue ENG-7 from Linear-issue trailers in trunk()..@\n");
    assert_eq!(api.variables("GetIssueDetails"), json!({ "id": "ENG-7" }));
    let jj = jj_calls(&cli);
    assert!(jj[1].starts_with("log -r trunk()..@ "), "{jj:?}");
    assert_eq!(
        jj.last().map(String::as_str),
        Some("git push --remote origin --bookmark exact:elsewhere")
    );
    assert_eq!(
        gh_calls(&cli)[0][6..],
        ["--repo", REPO, "--head", "elsewhere"]
    );
}

#[test]
fn gh_pull_request_never_runs_the_jj_path_even_in_a_jj_repository() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details())
        .on("GetIssueDetails", details());
    // A jj repository, with the retired setting that used to select jj.
    let cli =
        gh(jj_repo(Cli::for_api(&api), "").file("cwd/.jj/repo/store", "")).env("LINEAR_VCS", "jj");
    let run = cli.run(&["issue", "gh", "pr", "ENG-7"]);
    run.success()
        .stderr_has("LINEAR_VCS from process environment was removed in 4.0");
    assert_eq!(run.stdout, "https://github.com/acme/widgets/pull/1\n");
    assert!(calls(&cli, "jj").is_empty(), "{:?}", cli.calls("jj"));
    // An explicit @jj reads the trailers, and still nothing is pushed: gh
    // gets no repository or head it was not given.
    let run = cli.run(&["issue", "gh", "pr", "@jj", "--draft"]);
    run.success()
        .stderr_has("Using issue ENG-7 from Linear-issue trailers in trunk()..@\n");
    let jj = jj_calls(&cli);
    assert_eq!(jj.len(), 2, "{jj:?}");
    assert!(jj[0].starts_with("log -r trunk() "), "{jj:?}");
    assert!(jj[1].starts_with("log -r trunk()..@ "), "{jj:?}");
    assert_eq!(
        cli.calls("gh"),
        [
            vec![
                "pr",
                "create",
                "--title",
                "ENG-7 Repair the widget",
                "--body",
                URL
            ],
            vec![
                "pr",
                "create",
                "--title",
                "ENG-7 Repair the widget",
                "--body",
                URL,
                "--draft"
            ],
        ]
    );
}

#[test]
fn gh_pull_request_leaves_a_detached_head_to_gh() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details());
    let cli = Cli::for_api(&api)
        .file("cwd/.jj/repo/store", "")
        .stub_bin("jj", "echo \"unexpected jj $*\" >&2; exit 1")
        .stub_bin(
            "gh",
            "echo 'could not determine the current branch: not on any branch' >&2; exit 1",
        );
    let run = cli.run(&["issue", "gh", "pr", "ENG-7"]);
    assert_eq!(run.code, 1, "{run}");
    run.stderr_has("not on any branch");
    assert!(calls(&cli, "jj").is_empty(), "{:?}", cli.calls("jj"));
    assert_eq!(cli.calls("gh").len(), 1);
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
    let cli = gh(Cli::for_api(&api))
        .file("cwd/pr.md", "## Summary\nDescribe the change.\n\n")
        .file("cwd/.linear.toml", "pr_template = \"pr.md\"\n");
    let run = cli.run(&[
        "issue",
        "gh",
        "pr",
        "ENG-7",
        "DES-5",
        "ENG-256",
        "--title",
        " Widget and design ",
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
        (&["-r", "--no-template"], "References ENG-7 and DES-5"),
    ] {
        let api = MockLinear::start();
        issues(&api, &["DES-5"]);
        let cli = gh(Cli::for_api(&api));
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
fn one_issue_with_references_keeps_its_title_and_adds_the_clause() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details())
        .on("GetIssueDetails", details());
    let cli = gh(Cli::for_api(&api));
    cli.run(&["issue", "gh", "pr", "ENG-7", "--references"])
        .success();
    cli.run(&["issue", "gh", "pr", "ENG-7", "-r", "--title", " "])
        .success();
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
    let cli = gh(Cli::for_api(&api)).env("LINEAR_TEAM_ID", "ENG");
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
            URL
        ]]
    );
    assert_eq!(api.operations(), ["GetIssueDetails"]);
}

#[test]
fn several_issues_without_a_title_fail_before_any_request() {
    let api = MockLinear::start();
    for tool in ["gh", "jj"] {
        let cli = jj_repo_naming(Cli::for_api(&api), "", &["Fixes ENG-7", "Fixes DES-5"]);
        for args in [
            &["issue", tool, "pr", "ENG-7", "DES-5"][..],
            &["issue", tool, "pr", "ENG-7", "DES-5", "--title", "  "],
            &["issue", tool, "pr", "@jj", "ENG-7"],
        ] {
            let run = cli.run(args);
            run.usage_error()
                .stderr_has(
                    "A title is required for a pull request covering several issues: ",
                )
                .stderr_has(
                    "Pass --title \"<summary covering all issues>\". The title is used exactly as supplied.",
                );
            assert_eq!(run.stdout, "", "{run}");
        }
        let run = cli.run(&["issue", tool, "pr", "@jj", "ENG-7"]);
        run.usage_error()
            .stderr_has("covering several issues: DES-5, ENG-7\n");
        assert!(gh_calls(&cli).is_empty());
        assert!(
            !jj_calls(&cli)
                .iter()
                .any(|call| !call.starts_with("log -r trunk()")),
            "only @jj was read: {:?}",
            jj_calls(&cli)
        );
    }
    assert!(api.requests().is_empty());
}

#[test]
fn a_reference_naming_several_issues_covers_them_all_with_a_title() {
    let api = MockLinear::start();
    three_issues(&api);
    let cli = gh(jj_repo_naming(
        Cli::for_api(&api),
        "",
        &["Fixes ENG-7", "Fixes DES-5"],
    ));
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
    let jj = jj_calls(&cli);
    assert_eq!(jj.len(), 2, "gh pr only reads @jj: {jj:?}");
}

#[test]
fn a_missing_issue_among_several_publishes_nothing() {
    for tool in ["gh", "jj"] {
        let api = MockLinear::start();
        api.on_variables("GetIssueDetails", json!({ "id": "ENG-7" }), details())
            .on_variables_error(
                "GetIssueDetails",
                json!({ "id": "DES-5" }),
                "Entity not found: Issue",
            );
        let cli = jj_repo(Cli::for_api(&api), "");
        let run = cli.run(&["issue", tool, "pr", "ENG-7", "DES-5", "--title", "Both"]);
        assert_eq!(run.code, 3, "{run}");
        run.stderr_has("Failed to create pull request: Failed to fetch issue DES-5: ")
            .stderr_has("Issue not found: DES-5")
            .stderr_has(
                "No pull request was opened and no bookmark was created or pushed. Check the issue ID and access, then try again.",
            );
        assert!(calls(&cli, "jj").is_empty(), "{:?}", cli.calls("jj"));
        assert!(gh_calls(&cli).is_empty());
        assert!(!cli.path("cwd/pushed").exists());
    }
}

#[test]
fn jj_pull_request_for_several_issues_names_a_new_bookmark_after_the_first() {
    let api = MockLinear::start();
    issues(&api, &["DES-5"]);
    let cli = jj_repo(Cli::for_api(&api), "");
    let run = cli.run(&[
        "issue",
        "jj",
        "pr",
        "DES-5",
        "ENG-7",
        "-t",
        "Design and widget",
    ]);
    run.success()
        .stderr_has("Creating jj bookmark des-5-design\n")
        .stderr_has("Pushing jj bookmark des-5-design to origin\n");
    let jj = jj_calls(&cli);
    assert!(jj[0].starts_with("log -r @ --no-graph"), "{jj:?}");
    assert_eq!(
        jj[jj.len() - 3..],
        [
            "bookmark create -r bbb -- des-5-design",
            "bookmark list -T json(self) ++ \"\\n\"",
            "git push --remote origin --bookmark exact:des-5-design",
        ]
    );
    assert_eq!(
        gh_calls(&cli),
        [[
            "pr",
            "create",
            "--title",
            "Design and widget",
            "--body",
            &format!(
                "Fixes DES-5 and ENG-7\n\n{}\n{URL}",
                other_url("DES-5", "Design")
            ),
            "--repo",
            REPO,
            "--head",
            "des-5-design",
        ]]
    );
    // A bookmark already on the head, or --head, still wins.
    let api = MockLinear::start();
    issues(&api, &["DES-5"]);
    let cli = jj_repo(
        Cli::for_api(&api),
        "{\"name\":\"mine\",\"target\":[\"bbb\"]}\n",
    );
    cli.run(&["issue", "jj", "pr", "DES-5", "ENG-7", "-t", "Both"])
        .success();
    assert_eq!(gh_calls(&cli)[0][8..], ["--head", "mine"]);
    let api = MockLinear::start();
    issues(&api, &["DES-5"]);
    let cli = jj_repo(Cli::for_api(&api), "");
    cli.run(&[
        "issue", "jj", "pr", "DES-5", "ENG-7", "-t", "Both", "--head", "topic",
    ])
    .success();
    assert!(
        jj_calls(&cli).contains(&"bookmark create -r bbb -- topic".to_owned()),
        "{:?}",
        jj_calls(&cli)
    );
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

/// What `issue jj start` reports writing for ENG-7 to `change`.
fn described(change: &str) -> String {
    format!(
        "✓ Wrote jj change {change} description: \"ENG-7 Repair the widget\"\n\
         ✓ Wrote trailers (keep both when rewording to use @jj):\n\
         \x20 Linear-issue: Fixes ENG-7\n\
         \x20 Linear-issue-url: {URL}\n"
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
        .stdout_has("Keep both trailers when rewording if you want @jj to find the issue.")
        .stdout_has("3. Moves the issue")
        .stdout_has("No bookmark is created and nothing is pushed.");
}
