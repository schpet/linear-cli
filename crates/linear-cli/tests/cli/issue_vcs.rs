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

/// A git where no branch exists yet.
fn git(cli: Cli) -> Cli {
    cli.stub_bin(
        "git",
        "case \"$1 $2\" in\n\
         'show-ref --verify') exit 1 ;;\n\
         'checkout -b') echo \"Switched to a new branch '$3'\" >&2 ;;\n\
         *) exit 1 ;;\n\
         esac",
    )
}

/// A jj whose `@` already has a description, so `issue jj start` must make a new change.
fn jj(cli: Cli) -> Cli {
    cli.env("LINEAR_VCS", "jj").stub_bin(
        "jj",
        "case \"$*\" in\n\
         'log -r @ --no-graph '*) printf 'occupied\\n' ;;\n\
         esac",
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
    cli.run(&["issue", "git", "start", "ENG-7"])
        .success()
        .stdout_has("eng-7-repair-the-widget")
        .stdout_has("In Progress");
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
    cli.run(&[
        "issue",
        "git",
        "start",
        "7",
        "--branch",
        "my-branch",
        "--from-ref",
        "origin/main",
    ])
    .success();
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
    cli.run(&["issue", "git", "start", "ENG-7"])
        .failure()
        .stdout_has("Created and switched to branch")
        .stderr_has("Could not move the issue to a started status")
        .stderr_has("Team not found")
        .stderr_has("The branch is ready");
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
        ("In Review", "started", "is already started (In Review)"),
        (
            "Done",
            "completed",
            "is closed (Done), so it was not started",
        ),
    ] {
        let mut issue = details();
        issue["issue"]["state"] = json!({ "name": name, "type": state_type });
        let api = MockLinear::start();
        api.on("GetIssueTransitionDetails", issue.clone())
            .on("GetIssueTransitionDetails", issue);
        // The explicit namespace wins over the configured VCS.
        let cli = git(Cli::for_api(&api).env("LINEAR_VCS", "jj"));
        cli.run(&["issue", "git", "start", "ENG-7"])
            .success()
            .stdout_has("Created and switched to branch 'eng-7-repair-the-widget'")
            .stdout_has(said);
        let cli = jj(Cli::for_api(&api)).env("LINEAR_VCS", "git");
        cli.run(&["issue", "jj", "start", "ENG-7"])
            .success()
            .stdout_has("Prepared jj change for issue ENG-7")
            .stdout_has(said);
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
        .env("LINEAR_VCS", "jj")
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
        .env("LINEAR_VCS", "jj")
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
 'rev-parse --verify') exit 0 ;;
 'show-ref --verify') exit 1 ;;
 'checkout -b') exit 0 ;;
 *) exit 1 ;;
 esac",
        );
        cli.run(&["issue", "git", "start", "ENG-7", "--branch", branch])
            .success()
            .stdout_has("Created and switched");
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
 'rev-parse --verify') exit 0 ;;
 'show-ref --verify') exit 1 ;;
 'checkout -b') echo 'fatal: not a valid branch name' >&2; exit 128 ;;
 *) exit 1 ;;
 esac",
    );
    cli.run(&["issue", "git", "start", "ENG-7", "--branch", "main~0"])
        .failure()
        .stderr_has("not a valid branch name");
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
/// whether it is empty and immutable.
fn jj_commit(id: &str, parent: &str, description: &str, empty: bool, immutable: bool) -> String {
    json!([
        { "commit_id": id, "parents": [parent], "description": description },
        empty, immutable, false
    ])
    .to_string()
}

/// A jj repository for `issue gh pr`: an empty scratch `@` (`aaa`) on the
/// described change `bbb`, itself on the immutable `ccc`, with the remote
/// `origin`. Local bookmarks are the lines of `cwd/bookmarks`, which
/// `bookmark create` appends to. A push leaves `cwd/pushed` behind, and the
/// gh stub refuses to run before it exists.
fn jj_repo(cli: Cli, bookmarks: &str) -> Cli {
    let scratch = jj_commit("aaa", "bbb", "", true, false);
    let work = jj_commit("bbb", "ccc", "ENG-7 Repair the widget\n", false, false);
    let trunk = jj_commit("ccc", "ddd", "trunk\n", false, true);
    let scratch_change = jj_change("aaa", &[]);
    let work_change = jj_change("bbb", &["Fixes ENG-7"]);
    cli.env("LINEAR_VCS", "jj")
        .file("cwd/bookmarks", bookmarks)
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
fn gh_pull_request_with_jj_creates_and_pushes_a_bookmark_for_the_parent_of_a_scratch_change() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details());
    let cli = jj_repo(Cli::for_api(&api), "");
    let run = cli.run(&["issue", "gh", "pr", "ENG-7", "--draft"]);
    run.success()
        .stderr_has("Creating jj bookmark eng-7-repair-the-widget\n")
        .stderr_has("Pushing jj bookmark eng-7-repair-the-widget to origin\n")
        .stderr_has("Changes to push to origin:");
    assert_eq!(run.stdout, "https://github.com/acme/widgets/pull/1\n");
    let jj = jj_calls(&cli);
    assert!(jj[0].starts_with("log -r @ --no-graph"), "{jj:?}");
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
fn gh_pull_request_with_jj_reuses_a_bookmark_on_the_head_and_pushes_it() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details());
    let cli = jj_repo(
        Cli::for_api(&api),
        "{\"name\":\"mine\",\"target\":[\"bbb\"]}\n\
         {\"name\":\"mine\",\"remote\":\"origin\",\"target\":[\"bbb\"]}\n\
         {\"name\":\"elsewhere\",\"target\":[\"ccc\"]}\n",
    );
    cli.run(&["issue", "gh", "pull-request", "ENG-7", "--base", "main"])
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
fn gh_pull_request_with_jj_takes_head_as_a_bookmark_existing_or_new() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details());
    // An existing bookmark decides the revision, here one that is immutable.
    let cli = jj_repo(
        Cli::for_api(&api),
        "{\"name\":\"release\",\"target\":[\"ccc\"]}\n",
    );
    cli.run(&["issue", "gh", "pr", "ENG-7", "--head", "release"])
        .usage_error()
        .stderr_has("The pull request revision ccc is immutable");
    assert!(gh_calls(&cli).is_empty());
    // A new name is created on the automatic head.
    cli.run(&["issue", "gh", "pr", "ENG-7", "--head", "topic"])
        .success();
    assert!(
        jj_calls(&cli).contains(&"bookmark create -r bbb -- topic".to_owned()),
        "{:?}",
        jj_calls(&cli)
    );
    assert_eq!(gh_calls(&cli)[0][6..], ["--repo", REPO, "--head", "topic"]);
    cli.run(&["issue", "gh", "pr", "ENG-7", "--head", "someone:topic"])
        .usage_error()
        .stderr_has("Cannot open a pull request from someone:topic with jj");
}

#[test]
fn gh_pull_request_with_jj_stops_before_publishing_when_the_bookmark_is_unclear() {
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
        cli.run(&["issue", "gh", "pr", "ENG-7"])
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
fn gh_pull_request_with_jj_refuses_what_jj_would_not_push() {
    let api = MockLinear::start();
    // An undescribed change under the head stops it before a bookmark exists.
    let cli = jj_repo(Cli::for_api(&api), "").file("cwd/unpushable", "0123456789ab\n");
    cli.run(&["issue", "gh", "pr", "ENG-7"])
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
fn gh_pull_request_with_jj_needs_gh_before_it_pushes_anything() {
    let api = MockLinear::start();
    let cli = jj_repo(Cli::for_api(&api), "").stub_bin("gh", "exit 127");
    std::fs::remove_file(cli.path("bin/gh")).expect("remove the gh stub");
    cli.run(&["issue", "gh", "pr", "ENG-7"])
        .failure()
        .stderr_has("Could not find `gh`");
    assert!(calls(&cli, "jj").is_empty());
    assert!(api.requests().is_empty());
}

#[test]
fn gh_pull_request_with_jj_does_not_run_gh_when_the_push_fails() {
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
            jj_commit("aaa", "bbb", "ENG-7 work\n", false, false)
        ),
    );
    let run = cli.run(&["issue", "gh", "pr", "ENG-7"]);
    assert_eq!(run.code, 1, "{run}");
    run.stderr_has("Failed to push the jj bookmark mine to origin")
        .stderr_has("remote rejected")
        .stderr_has("No pull request was opened");
    assert!(gh_calls(&cli).is_empty());
}

#[test]
fn gh_pull_request_with_jj_needs_a_remote_and_a_repository_before_any_request() {
    let api = MockLinear::start();
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
                jj_commit("aaa", "bbb", "ENG-7 work\n", false, false)
            ),
        );
        cli.run(&["issue", "gh", "pr", "ENG-7"])
            .failure()
            .stderr_has(said);
        assert!(gh_calls(&cli).is_empty());
    }
    gh(Cli::for_api(&api))
        .env("LINEAR_VCS", "jj")
        .stub_bin(
            "jj",
            "echo 'Error: There is no jj repo in \".\"' >&2; exit 1",
        )
        .run(&["issue", "gh", "pr", "ENG-7"])
        .failure()
        .stderr_has("Not in a jj repository");
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
            .stderr_has("linear issue gh pr");
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
    for vcs in ["git", "jj"] {
        let cli = repo_naming_an_issue(&api).env("LINEAR_VCS", vcs);
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
    }
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
fn gh_pull_request_without_an_issue_says_how_to_name_one_and_reads_nothing() {
    let api = MockLinear::start();
    for vcs in ["git", "jj"] {
        let cli = gh(repo_naming_an_issue(&api).env("LINEAR_VCS", vcs));
        for args in [
            &["issue", "gh", "pr", "--draft"][..],
            &["issue", "gh", "pull-request", "--template", "missing.md"],
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
        let help = cli.run(&["issue", "gh", "pr", "--help"]);
        help.success()
            .stdout_has("Usage: linear issue gh pull-request [OPTIONS] <ISSUE>")
            .stdout_has("@git");
    }
    assert!(api.requests().is_empty());
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
        &["issue", "gh", "pr", "@jj"],
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
fn gh_pull_request_with_jj_takes_the_issue_from_at_jj_and_snapshots_once() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details());
    let cli = jj_repo(Cli::for_api(&api), "");
    let run = cli.run(&["issue", "gh", "pr", "@jj"]);
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
