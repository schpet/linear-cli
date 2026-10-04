//! Issue commands that read the current branch or drive git, jj and gh.
use serde_json::{Value, json};

use crate::support::{Cli, MockLinear};

const URL: &str = "https://linear.app/acme/issue/ENG-7/repair-the-widget";

fn details() -> Value {
    let last = json!({ "hasNextPage": false, "endCursor": null });
    json!({ "issue": {
        "identifier": "ENG-7", "title": "Repair the widget", "description": null,
        "url": URL, "branchName": "eng-7-repair-the-widget",
        "state": { "name": "Todo", "color": "#123456" }, "assignee": null, "priority": 2,
        "project": null, "projectMilestone": null, "cycle": null,
        "team": { "key": "ENG", "activeCycle": null }, "labels": { "nodes": [], "pageInfo": last }, "parent": null,
        "children": { "nodes": [], "pageInfo": last },
        "attachments": { "nodes": [], "pageInfo": last },
        "documents": { "nodes": [], "pageInfo": last }
    } })
}

fn states() -> Value {
    json!({ "team": { "states": { "nodes": [
        { "id": "state-todo", "name": "Todo", "type": "unstarted", "position": 1 },
        { "id": "state-review", "name": "In Review", "type": "started", "position": 3 },
        { "id": "state-progress", "name": "In Progress", "type": "started", "position": 2 },
    ], "pageInfo": { "hasNextPage": false, "endCursor": null } } } })
}

/// A git whose current branch is `branch`, where no other branch exists yet.
fn git(cli: Cli, branch: &str) -> Cli {
    cli.stub_bin(
        "git",
        &format!(
            "case \"$1 $2\" in\n\
             'symbolic-ref --quiet') echo '{branch}' ;;\n\
             'show-ref --verify') exit 1 ;;\n\
             'checkout -b') echo \"Switched to a new branch '$3'\" >&2 ;;\n\
             *) exit 1 ;;\n\
             esac"
        ),
    )
}

/// A jj whose working-copy ancestry carries a `Linear-issue: Fixes ENG-7` trailer and whose
/// `@` already has a description, so `issue start` must make a new change.
fn jj(cli: Cli) -> Cli {
    cli.env("LINEAR_VCS", "jj").stub_bin(
        "jj",
        "case \"$*\" in\n\
         'log -r ::@ '*) printf 'Fixes ENG-7\\n' ;;\n\
         'log -r @ --no-graph '*) printf 'occupied\\n' ;;\n\
         *'-T commit_id'*) printf 'abc123\\n' ;;\n\
         *builtin_log_compact_full_description*) printf 'commit abc123\\n+patched line\\n' ;;\n\
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
fn id_comes_from_the_git_branch() {
    let cli = git(Cli::new(), "feature/eng-7-repair");
    cli.run(&["issue", "id"]).success().stdout_has("ENG-7");
    assert_eq!(
        calls(&cli, "git"),
        [["symbolic-ref", "--quiet", "--short", "HEAD"]]
    );
}

#[test]
fn id_fails_when_the_branch_names_no_issue() {
    git(Cli::new(), "main").run(&["issue", "id"]).failure();
}

#[test]
fn id_comes_from_jj_trailers() {
    let cli = jj(Cli::new());
    cli.run(&["issue", "id"]).success().stdout_has("ENG-7");
    assert!(calls(&cli, "git").is_empty());
    assert_eq!(calls(&cli, "jj")[0][..3], ["log", "-r", "::@"]);
}

#[test]
fn vcs_is_read_from_project_config() {
    let cli = jj(Cli::new())
        .env_remove("LINEAR_VCS")
        .file("cwd/.linear.toml", "vcs = \"jj\"\n");
    cli.run(&["issue", "id"]).success().stdout_has("ENG-7");
}

#[test]
fn title_uses_the_issue_from_the_branch() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details());
    git(Cli::for_api(&api), "eng-7-repair")
        .run(&["issue", "title"])
        .success()
        .stdout_has("Repair the widget");
    assert_eq!(api.variables("GetIssueDetails"), json!({ "id": "ENG-7" }));
}

#[test]
fn view_json_uses_the_issue_from_the_branch() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details());
    let json = git(Cli::for_api(&api), "eng-7-repair")
        .run(&["issue", "view", "--json", "--no-comments"])
        .success()
        .json();
    assert_eq!(json["identifier"], "ENG-7");
    assert_eq!(api.variables("GetIssueDetails"), json!({ "id": "ENG-7" }));
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
fn describe_infers_the_issue_from_the_branch() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details());
    git(Cli::for_api(&api), "eng-7-repair")
        .run(&["issue", "describe"])
        .success()
        .stdout_has("Linear-issue: Fixes ENG-7");
}

#[test]
fn start_creates_the_issue_branch_and_marks_it_started() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details())
        .on("GetWorkflowStates", states())
        .on(
            "UpdateIssueState",
            json!({ "issueUpdate": { "success": true } }),
        );
    let cli = git(Cli::for_api(&api), "main");
    cli.run(&["issue", "start", "ENG-7"])
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
    assert_eq!(api.variables("GetIssueDetails"), json!({ "id": "ENG-7" }));
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
fn start_accepts_a_custom_branch_and_base_ref() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details())
        .on("GetWorkflowStates", states())
        .on(
            "UpdateIssueState",
            json!({ "issueUpdate": { "success": true } }),
        );
    let cli = git(Cli::for_api(&api).env("LINEAR_TEAM_ID", "ENG"), "main");
    cli.run(&[
        "issue",
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
fn start_fails_after_preparing_the_branch_when_the_state_update_fails() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details())
        .on_error("GetWorkflowStates", "Team not found");
    let cli = git(Cli::for_api(&api), "main");
    cli.run(&["issue", "start", "ENG-7"])
        .failure()
        .stdout_has("Created and switched to branch")
        .stderr_has("Could not move the issue to a started state")
        .stderr_has("Team not found")
        .stderr_has("The branch is ready");
    assert!(calls(&cli, "git").iter().any(|args| args[0] == "checkout"));
}

#[test]
fn start_fails_when_linear_does_not_update_the_state() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details())
        .on("GetWorkflowStates", states())
        .on(
            "UpdateIssueState",
            json!({ "issueUpdate": { "success": false } }),
        );
    let cli = jj(Cli::for_api(&api));
    cli.run(&["issue", "start", "ENG-7"])
        .failure()
        .stderr_has("Linear did not update the issue")
        .stderr_has("The jj change is ready");
}

#[test]
fn start_uses_the_workflow_states_of_the_issue_team() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details())
        .on("GetWorkflowStates", states())
        .on(
            "UpdateIssueState",
            json!({ "issueUpdate": { "success": true } }),
        );
    let cli = git(Cli::for_api(&api).env("LINEAR_TEAM_ID", "OPS"), "main");
    cli.run(&["issue", "start", "ENG-7"]).success();
    assert_eq!(
        api.variables("GetWorkflowStates"),
        json!({ "teamKey": "ENG", "first": 100 })
    );
}

#[test]
fn start_with_jj_makes_a_new_described_change() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details())
        .on("GetWorkflowStates", states())
        .on(
            "UpdateIssueState",
            json!({ "issueUpdate": { "success": true } }),
        );
    let cli = jj(Cli::for_api(&api));
    cli.run(&["issue", "start", "ENG-7"]).success();
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
fn start_with_a_bare_number_and_no_team_fails_before_any_request() {
    let api = MockLinear::start();
    let cli = git(Cli::for_api(&api), "main");
    cli.run(&["issue", "start", "7"])
        .failure()
        .stderr_has("Issue number 7 needs a team");
    assert!(api.requests().is_empty());
    assert!(calls(&cli, "git").is_empty());
}

#[test]
fn start_rejects_an_empty_issue_id_without_a_picker() {
    let api = MockLinear::start();
    let cli = git(Cli::for_api(&api).env("LINEAR_TEAM_ID", "ENG"), "main");
    cli.run(&["issue", "start", ""])
        .usage_error()
        .stderr_has("a value is required");
    assert!(api.requests().is_empty());
    assert!(calls(&cli, "git").is_empty());
}

#[test]
fn start_without_an_issue_or_team_fails_before_any_request() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&["issue", "start"])
        .usage_error()
        .stderr_has("No team to pick an issue from");
    assert!(api.requests().is_empty());
}

#[test]
fn start_rejects_a_reference_that_names_no_issue_instead_of_picking() {
    let api = MockLinear::start();
    let cli = git(Cli::for_api(&api).env("LINEAR_TEAM_ID", "ENG"), "main");
    cli.run(&["issue", "start", "not-an-issue"])
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
    cli.run(&["issue", "pull-request", "ENG-7"]).success();
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
    let cli = git(gh(Cli::for_api(&api)), "eng-7-repair");
    cli.run(&[
        "issue", "pr", "--title", "Custom", "--base", "main", "--head", "topic", "--draft",
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
    cli.run(&["issue", "pr", "ENG-7"]).success();
    cli.run(&["issue", "pr", "ENG-7", "--no-template"])
        .success();
    let calls = cli.calls("gh");
    assert_eq!(calls[0][5], format!("## Summary\n\n{URL}"));
    assert_eq!(calls[1][5], URL);
}

#[test]
fn pull_request_with_a_missing_template_fails_before_any_request() {
    let api = MockLinear::start();
    let cli = gh(Cli::for_api(&api));
    cli.run(&["issue", "pr", "ENG-7", "--template", "missing.md"])
        .failure()
        .stderr_has("missing.md");
    assert!(api.requests().is_empty());
    assert!(cli.calls("gh").is_empty());
}

#[test]
fn pull_request_fails_when_gh_fails() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details());
    let run = Cli::for_api(&api)
        .stub_bin("gh", "exit 5")
        .run(&["issue", "pr", "ENG-7"]);
    assert_eq!(run.code, 5, "{run}");
}

#[test]
fn commits_shows_the_jj_log_for_the_issue() {
    let api = MockLinear::start();
    api.on("GetIssueId", json!({ "issue": { "id": "issue-7" } }));
    let cli = jj(Cli::for_api(&api));
    cli.run(&["issue", "commits", "eng-7"])
        .success()
        .stdout_has("+patched line");
    assert_eq!(api.variables("GetIssueId"), json!({ "id": "ENG-7" }));
    let log = calls(&cli, "jj")
        .into_iter()
        .find(|args| args.iter().any(|arg| arg == "-p"))
        .expect("jj log -p ran");
    assert!(
        log[2].contains("Linear-issue") && log[2].contains("ENG-7"),
        "{log:?}"
    );
}

#[test]
fn commits_infers_the_issue_from_jj_trailers() {
    let api = MockLinear::start();
    api.on("GetIssueId", json!({ "issue": { "id": "issue-7" } }));
    jj(Cli::for_api(&api))
        .run(&["issue", "commits"])
        .success()
        .stdout_has("+patched line");
    assert_eq!(api.variables("GetIssueId"), json!({ "id": "ENG-7" }));
}

#[test]
fn commits_requires_jj() {
    let api = MockLinear::start();
    let cli = git(Cli::for_api(&api), "eng-7-repair").stub_bin("jj", "exit 1");
    cli.run(&["issue", "commits", "ENG-7"])
        .failure()
        .stderr_has("jj");
    assert!(api.requests().is_empty());
    assert!(cli.calls("jj").is_empty());
}

#[test]
fn commits_drains_large_probe_output_and_passes_the_log_exit_status_through() {
    let api = MockLinear::start();
    api.on("GetIssueId", json!({ "issue": { "id": "issue-7" } }));
    // Children get a null stdin, and the probe output is larger than a pipe buffer on both
    // streams.
    let cli = Cli::for_api(&api)
        .env("LINEAR_VCS", "jj")
        .stdin(b"not for children\n")
        .stub_bin(
            "jj",
            "if IFS= read -r line; then exit 8; fi\n\
             case \"$*\" in\n\
             *'-T commit_id'*) head -c 131072 /dev/zero | tr '\\0' x; head -c 131072 /dev/zero >&2 ;;\n\
             *builtin_log_compact_full_description*) printf '+patched line\\n'; exit 7 ;;\n\
             esac",
        );
    let run = cli.run(&["issue", "commits", "ENG-7"]);
    assert_eq!(run.code, 7, "{run}");
    assert_eq!(run.stdout, "+patched line\n");
    assert_eq!(calls(&cli, "jj").len(), 2);
}

#[test]
fn commits_fails_when_no_commit_names_the_issue() {
    let api = MockLinear::start();
    api.on("GetIssueId", json!({ "issue": { "id": "issue-7" } }));
    let cli = Cli::for_api(&api)
        .env("LINEAR_VCS", "jj")
        .stub_bin("jj", "printf '\\357\\273\\277 \\n'");
    cli.run(&["issue", "commits", "ENG-7"])
        .failure()
        .stderr_has("Commits not found: ENG-7");
    assert_eq!(calls(&cli, "jj").len(), 1);
}

#[test]
fn commits_fails_when_the_jj_probe_fails() {
    let api = MockLinear::start();
    api.on("GetIssueId", json!({ "issue": { "id": "issue-7" } }));
    let cli = Cli::for_api(&api).env("LINEAR_VCS", "jj").stub_bin(
        "jj",
        "printf 'abc123\\n'; echo 'Error: The working copy is stale' >&2; exit 1",
    );
    cli.run(&["issue", "commits", "ENG-7"])
        .failure()
        .stderr_has("The working copy is stale");
    assert_eq!(calls(&cli, "jj").len(), 1);
}

#[test]
fn id_outside_a_jj_repository_says_so() {
    let cli = Cli::new().env("LINEAR_VCS", "jj").stub_bin(
        "jj",
        "echo 'Error: There is no jj repo in \".\"' >&2; exit 1",
    );
    cli.run(&["issue", "id"])
        .failure()
        .stderr_has("Not in a jj repository")
        .stderr_has("Pass an issue ID like ENG-123");
}

#[test]
fn id_reports_a_failed_jj_log_inside_a_repository() {
    let cli = Cli::new()
        .env("LINEAR_VCS", "jj")
        .file("cwd/.jj/repo/store", "")
        .stub_bin("jj", "echo 'Error: concurrent operation' >&2; exit 1");
    cli.run(&["issue", "id"])
        .failure()
        .stderr_has("concurrent operation");
}

#[test]
fn id_on_a_detached_git_head_names_no_issue() {
    Cli::new()
        .stub_bin("git", "exit 1")
        .run(&["issue", "id"])
        .failure()
        .stderr_has("Could not determine issue ID");
}

#[test]
fn id_outside_a_git_repository_says_so() {
    Cli::new()
        .stub_bin("git", "echo 'fatal: not a git repository' >&2; exit 128")
        .run(&["issue", "id"])
        .failure()
        .stderr_has("Not in a git repository")
        .stderr_has("Pass an issue ID like ENG-123");
}

#[test]
fn id_reports_a_failed_git_branch_lookup_inside_a_repository() {
    Cli::new()
        .file("cwd/.git/HEAD", "ref: refs/heads/main\n")
        .stub_bin("git", "echo 'fatal: bad object HEAD' >&2; exit 128")
        .run(&["issue", "id"])
        .failure()
        .stderr_has("fatal: bad object HEAD");
}

#[test]
fn start_outside_a_repository_says_so_before_changing_the_state() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details());
    let cli =
        Cli::for_api(&api).stub_bin("git", "echo 'fatal: not a git repository' >&2; exit 128");
    cli.run(&["issue", "start", "ENG-7"])
        .failure()
        .stderr_has("Not in a git repository")
        .stderr_has("linear issue update <ID> --state <state>");
    assert_eq!(api.operations(), ["GetIssueDetails"]);
}

#[test]
fn start_with_jj_stops_when_the_change_probe_fails() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details());
    let cli = Cli::for_api(&api)
        .env("LINEAR_VCS", "jj")
        .env("LINEAR_TEAM_ID", "ENG")
        .file("cwd/.jj/repo/store", "")
        .stub_bin("jj", "echo 'Error: concurrent operation' >&2; exit 1");
    cli.run(&["issue", "start", "ENG-7"])
        .failure()
        .stderr_has("concurrent operation");
    assert_eq!(calls(&cli, "jj").len(), 1);
    assert_eq!(api.operations(), ["GetIssueDetails"]);
}

#[test]
fn pull_request_with_an_empty_title_uses_the_issue_title() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details());
    let cli = gh(Cli::for_api(&api));
    cli.run(&["issue", "pr", "ENG-7", "--title", ""]).success();
    assert_eq!(cli.calls("gh")[0][3], "ENG-7 Repair the widget");
}

#[test]
fn start_creates_a_local_branch_when_only_another_revision_has_its_name() {
    for branch in ["release", "deadbeef", "origin/release"] {
        let api = MockLinear::start();
        api.on("GetIssueDetails", details())
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
        cli.run(&["issue", "start", "ENG-7", "--branch", branch])
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
            ["GetIssueDetails", "GetWorkflowStates", "UpdateIssueState"]
        );
        assert_eq!(
            api.variables("UpdateIssueState"),
            json!({"issueId": "ENG-7", "stateId": "state-progress"})
        );
    }
}

#[test]
fn start_still_requires_a_terminal_when_a_local_branch_exists() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details());
    let cli = Cli::for_api(&api).stub_bin(
        "git",
        "case \"$1 $2\" in
 'rev-parse --verify' | 'show-ref --verify') exit 0 ;;
 *) exit 1 ;;
 esac",
    );
    cli.run(&["issue", "start", "ENG-7"])
        .failure()
        .stderr_has("--branch with a new name");
    assert_eq!(api.operations(), ["GetIssueDetails"]);
    assert!(
        !calls(&cli, "git")
            .iter()
            .any(|args| args.first().is_some_and(|arg| arg == "checkout"))
    );
}

#[test]
fn start_preserves_unexpected_git_probe_failures() {
    for status in [2, 128] {
        let api = MockLinear::start();
        api.on("GetIssueDetails", details());
        let cli = Cli::for_api(&api).file("cwd/.git/HEAD", "").stub_bin(
            "git",
            &format!(
                "case \"$1 $2\" in
 'rev-parse --verify' | 'show-ref --verify') echo 'fatal: corrupt refs' >&2; exit {status} ;;
 *) exit 1 ;;
 esac"
            ),
        );
        cli.run(&["issue", "start", "ENG-7"])
            .failure()
            .stderr_has("Failed to check if branch exists")
            .stderr_has("fatal: corrupt refs");
        assert_eq!(api.operations(), ["GetIssueDetails"]);
        assert!(
            !calls(&cli, "git")
                .iter()
                .any(|args| args.first().is_some_and(|arg| arg == "checkout"))
        );
    }
}

#[test]
fn start_does_not_treat_a_revision_expression_as_a_branch() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details());
    let cli = Cli::for_api(&api).stub_bin(
        "git",
        "case \"$1 $2\" in
 'rev-parse --verify') exit 0 ;;
 'show-ref --verify') exit 1 ;;
 'checkout -b') echo 'fatal: not a valid branch name' >&2; exit 128 ;;
 *) exit 1 ;;
 esac",
    );
    cli.run(&["issue", "start", "ENG-7", "--branch", "main~0"])
        .failure()
        .stderr_has("not a valid branch name");
    assert_eq!(api.operations(), ["GetIssueDetails"]);
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
