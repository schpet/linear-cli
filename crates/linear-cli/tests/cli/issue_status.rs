//! `issue start`, `issue complete` and `issue cancel`: status moves that
//! leave the repository alone.
use serde_json::{Value, json};

use crate::support::{Cli, MockLinear};

fn issue(state: &str, state_type: &str) -> Value {
    json!({ "issue": {
        "identifier": "ENG-7", "title": "Repair the widget",
        "url": "https://linear.app/acme/issue/ENG-7/repair-the-widget",
        "branchName": "eng-7-repair-the-widget",
        "team": { "key": "ENG" },
        "state": { "name": state, "type": state_type }
    } })
}

fn states(nodes: Value) -> Value {
    json!({ "team": { "states": {
        "nodes": nodes, "pageInfo": { "hasNextPage": false, "endCursor": null }
    } } })
}

fn workflow() -> Value {
    states(json!([
        { "id": "state-todo", "name": "Todo", "type": "unstarted", "position": 1 },
        { "id": "state-review", "name": "In Review", "type": "started", "position": 3 },
        { "id": "state-progress", "name": "In Progress", "type": "started", "position": 2 },
        { "id": "state-done", "name": "Done", "type": "completed", "position": 4 },
        { "id": "state-canceled", "name": "Canceled", "type": "canceled", "position": 5 },
    ]))
}

fn moved() -> Value {
    json!({ "issueUpdate": { "success": true } })
}

/// A sandbox where git and jj record their calls and fail, in a directory
/// that is both kinds of repository and whose branch names another issue.
fn repo(api: &MockLinear) -> Cli {
    Cli::for_api(api)
        .file("cwd/.jj/repo/store", "")
        .stub_bin(
            "git",
            "case \"$1\" in symbolic-ref) echo eng-99-other ;; *) exit 1 ;; esac",
        )
        .stub_bin("jj", "exit 1")
}

/// Calls of `tool` other than the startup repository-root probe.
fn calls(cli: &Cli, tool: &str) -> Vec<Vec<String>> {
    cli.calls(tool)
        .into_iter()
        .filter(|args| args[..] != ["rev-parse", "--show-toplevel"])
        .collect()
}

#[test]
fn start_moves_the_issue_to_the_first_started_status_and_leaves_the_repository_alone() {
    let api = MockLinear::start();
    api.on("GetIssueTransitionDetails", issue("Todo", "unstarted"))
        .on("GetWorkflowStates", workflow())
        .on("UpdateIssueState", moved());
    let cli = repo(&api);
    let run = cli.run(&["issue", "start", "ENG-7"]);
    run.success();
    assert_eq!(run.stdout, "✓ Moved issue ENG-7 to In Progress\n");
    assert_eq!(
        api.variables("UpdateIssueState"),
        json!({ "issueId": "ENG-7", "stateId": "state-progress" })
    );
    assert!(calls(&cli, "git").is_empty(), "{:?}", cli.calls("git"));
    assert!(calls(&cli, "jj").is_empty(), "{:?}", cli.calls("jj"));
}

#[test]
fn start_works_outside_a_repository_and_uses_the_issue_team() {
    let api = MockLinear::start();
    api.on("GetIssueTransitionDetails", issue("Backlog", "backlog"))
        .on("GetWorkflowStates", workflow())
        .on("UpdateIssueState", moved());
    Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "OPS")
        .run(&["issue", "start", "https://linear.app/acme/issue/ENG-7/x"])
        .success();
    assert_eq!(
        api.variables("GetWorkflowStates"),
        json!({ "teamKey": "ENG", "first": 100 })
    );
}

#[test]
fn start_leaves_started_and_closed_issues_as_they_are() {
    for (name, state_type, said) in [
        (
            "In Review",
            "started",
            "✓ Issue ENG-7 is already started (In Review)\n",
        ),
        (
            "Done",
            "completed",
            "Issue ENG-7 is closed (Done), so it was not started. To reopen it, run `linear issue update ENG-7 --state <state>`.\n",
        ),
        (
            "Canceled",
            "canceled",
            "Issue ENG-7 is closed (Canceled), so it was not started. To reopen it, run `linear issue update ENG-7 --state <state>`.\n",
        ),
        (
            "Duplicate",
            "duplicate",
            "Issue ENG-7 is closed (Duplicate), so it was not started. To reopen it, run `linear issue update ENG-7 --state <state>`.\n",
        ),
    ] {
        let api = MockLinear::start();
        api.on("GetIssueTransitionDetails", issue(name, state_type));
        let run = Cli::for_api(&api).run(&["issue", "start", "ENG-7"]);
        run.success();
        assert_eq!(run.stdout, said);
        assert_eq!(api.operations(), ["GetIssueTransitionDetails"]);
    }
}

#[test]
fn start_says_where_the_git_flags_went_before_any_request() {
    let api = MockLinear::start();
    let cli = repo(&api);
    for flags in [
        &["--branch", "topic"][..],
        &["-b", "topic"],
        &["--from-ref", "main"],
        &["-f", "main"],
    ] {
        let mut args = vec!["issue", "start", "ENG-7"];
        args.extend_from_slice(flags);
        cli.run(&args)
            .usage_error()
            .stderr_has("--branch and --from-ref moved to `issue git start`")
            .stderr_has("linear issue git start <ISSUE> --branch <NAME> --from-ref <REF>");
    }
    assert!(api.requests().is_empty());
    assert!(calls(&cli, "git").is_empty());
}

#[test]
fn complete_and_cancel_move_the_issue_to_the_only_status_of_their_type() {
    for (verb, state, name) in [
        ("complete", "state-done", "Done"),
        ("cancel", "state-canceled", "Canceled"),
    ] {
        let api = MockLinear::start();
        api.on("GetIssueTransitionDetails", issue("In Review", "started"))
            .on("GetWorkflowStates", workflow())
            .on("UpdateIssueState", moved());
        let cli = repo(&api);
        let run = cli.run(&["issue", verb, "ENG-7"]);
        run.success();
        assert_eq!(run.stdout, format!("✓ Moved issue ENG-7 to {name}\n"));
        assert_eq!(
            api.variables("UpdateIssueState"),
            json!({ "issueId": "ENG-7", "stateId": state })
        );
        assert!(calls(&cli, "git").is_empty());
        assert!(calls(&cli, "jj").is_empty());
    }
}

#[test]
fn complete_and_cancel_leave_an_issue_already_there_and_move_one_closed_the_other_way() {
    let api = MockLinear::start();
    api.on("GetIssueTransitionDetails", issue("Shipped", "completed"));
    let run = Cli::for_api(&api).run(&["issue", "complete", "ENG-7"]);
    run.success();
    assert_eq!(run.stdout, "✓ Issue ENG-7 is already completed (Shipped)\n");
    assert_eq!(api.operations(), ["GetIssueTransitionDetails"]);

    let api = MockLinear::start();
    api.on("GetIssueTransitionDetails", issue("Done", "completed"))
        .on("GetWorkflowStates", workflow())
        .on("UpdateIssueState", moved());
    Cli::for_api(&api)
        .run(&["issue", "cancel", "ENG-7"])
        .success()
        .stdout_has("Moved issue ENG-7 to Canceled");
}

#[test]
fn cancel_does_not_guess_between_several_canceled_statuses() {
    let api = MockLinear::start();
    api.on("GetIssueTransitionDetails", issue("Todo", "unstarted"))
        .on(
            "GetWorkflowStates",
            states(json!([
                { "id": "state-dup", "name": "Duplicate", "type": "canceled", "position": 6 },
                { "id": "state-canceled", "name": "Canceled", "type": "canceled", "position": 5 },
            ])),
        );
    let run = Cli::for_api(&api).run(&["issue", "cancel", "ENG-7"]);
    assert_eq!(run.code, 1, "{run}");
    run.stderr_has("Team ENG has several canceled statuses: \"Canceled\", \"Duplicate\"")
        .stderr_has("linear issue update ENG-7 --state <name>");
    assert_eq!(
        api.operations(),
        ["GetIssueTransitionDetails", "GetWorkflowStates"]
    );
}

#[test]
fn a_team_without_a_status_of_the_type_is_reported() {
    let api = MockLinear::start();
    api.on("GetIssueTransitionDetails", issue("Todo", "unstarted"))
        .on(
            "GetWorkflowStates",
            states(json!([
                { "id": "state-todo", "name": "Todo", "type": "unstarted", "position": 1 },
            ])),
        );
    Cli::for_api(&api)
        .run(&["issue", "complete", "ENG-7"])
        .failure()
        .stderr_has("Failed to complete issue")
        .stderr_has("Team ENG has no completed status")
        .stderr_has("linear team states ENG");
}

#[test]
fn a_refused_update_and_an_unknown_status_type_are_errors() {
    let api = MockLinear::start();
    api.on("GetIssueTransitionDetails", issue("Todo", "unstarted"))
        .on("GetWorkflowStates", workflow())
        .on(
            "UpdateIssueState",
            json!({ "issueUpdate": { "success": false } }),
        );
    Cli::for_api(&api)
        .run(&["issue", "start", "ENG-7"])
        .failure()
        .stderr_has("Linear did not update the issue");

    let api = MockLinear::start();
    api.on("GetIssueTransitionDetails", issue("Parked", "parked"));
    Cli::for_api(&api)
        .run(&["issue", "cancel", "ENG-7"])
        .failure()
        .stderr_has("status of unknown type \"parked\"");
    assert_eq!(api.operations(), ["GetIssueTransitionDetails"]);
}

#[test]
fn complete_and_cancel_need_an_issue_and_never_take_it_from_the_branch() {
    let api = MockLinear::start();
    let cli = repo(&api).env("LINEAR_TEAM_ID", "ENG");
    for verb in ["complete", "cancel"] {
        cli.run(&["issue", verb]).usage_error();
        cli.run(&["issue", verb, "not-an-issue"])
            .usage_error()
            .stderr_has("Not an issue ID: not-an-issue");
    }
    cli.run(&["issue", "start"])
        .usage_error()
        .stderr_has("No issue to start");
    assert!(api.requests().is_empty());
    assert!(calls(&cli, "git").is_empty());
    assert!(calls(&cli, "jj").is_empty());
}

#[test]
fn a_missing_issue_is_not_found() {
    let api = MockLinear::start();
    api.on_error("GetIssueTransitionDetails", "Entity not found: Issue");
    Cli::for_api(&api)
        .run(&["issue", "complete", "ENG-404"])
        .not_found()
        .stderr_has("Issue not found: ENG-404");
}

#[test]
fn a_status_verb_takes_the_issue_from_at_git_only_when_asked() {
    let api = MockLinear::start();
    api.on("GetIssueTransitionDetails", issue("In Progress", "started"))
        .on("GetWorkflowStates", workflow())
        .on("UpdateIssueState", moved());
    let cli = repo(&api);
    let run = cli.run(&["issue", "complete", "@git"]);
    run.success()
        .stderr_has("Using issue ENG-99 from the git branch name \"eng-99-other\"\n");
    assert_eq!(
        api.variables("GetIssueTransitionDetails"),
        json!({ "id": "ENG-99" })
    );
    assert_eq!(
        calls(&cli, "git"),
        [["symbolic-ref", "--quiet", "--short", "HEAD"]]
    );
    assert!(calls(&cli, "jj").is_empty());
}
