//! `issue create`, `issue update`, `issue archive` and `issue delete`.
use serde_json::{Value, json};

use crate::support::{Cli, MockLinear, Run};
use crate::team::{resolve_vars, resolved};

const ENG_ID: &str = "team-eng-id";
const PARENT_ID: &str = "issue-parent-id";
const PROJECT_ID: &str = "project-roadmap-id";

fn created(identifier: &str) -> Value {
    json!({
        "issueCreate": {
            "success": true,
            "issue": {
                "id": "issue-new-id", "identifier": identifier, "title": "New issue",
                "url": format!("https://linear.app/acme/issue/{identifier}/new"),
                "team": { "key": "ENG" }
            }
        }
    })
}

fn updated(identifier: &str, title: &str) -> Value {
    json!({
        "issueUpdate": {
            "success": true,
            "issue": {
                "id": "issue-1-id", "identifier": identifier, "title": title,
                "url": format!("https://linear.app/acme/issue/{identifier}/x")
            }
        }
    })
}

fn states() -> Value {
    json!({ "team": { "states": { "nodes": [
        { "id": "state-todo", "name": "Todo", "type": "unstarted", "position": 1 },
        { "id": "state-progress", "name": "In Progress", "type": "started", "position": 2 },
        { "id": "state-done", "name": "Done", "type": "completed", "position": 3 }
    ], "pageInfo": { "hasNextPage": false, "endCursor": null } } } })
}

fn label(id: &str, name: &str) -> Value {
    json!({ "issueLabels": { "nodes": [{ "id": id, "name": name }] } })
}

fn cycles() -> Value {
    json!({ "team": {
        "key": "ENG", "cyclesEnabled": true,
        "cycles": {
            "nodes": [{
                "id": "cycle-next", "number": 6, "name": "Next", "startsAt": "2999-01-01T00:00:00.000Z",
                "isNext": true, "isPrevious": false
            }],
            "pageInfo": { "hasNextPage": false, "endCursor": null }
        },
        "activeCycle": { "id": "cycle-active", "number": 5, "name": "Current" }
    } })
}

fn milestones() -> Value {
    json!({ "project": { "projectMilestones": { "nodes": [{ "id": "milestone-1", "name": "Beta" }] } } })
}

fn template(id: &str, name: &str, kind: &str) -> Value {
    json!({
        "id": id, "name": name, "description": null, "type": kind, "icon": null, "color": null,
        "hasFormFields": false, "lastAppliedAt": null, "sortOrder": 0,
        "createdAt": "2025-01-01T00:00:00.000Z", "updatedAt": "2025-01-01T00:00:00.000Z",
        "team": null, "inheritedFrom": null, "creator": null, "templateData": "{}"
    })
}

/// The mutation `input`, with an empty `labelIds` list dropped (it means "no labels" either way)
/// and label ids sorted (resolution order is not part of the contract).
fn input(api: &MockLinear, operation: &str) -> Value {
    let mut input = api.variables(operation)["input"].clone();
    let object = input.as_object_mut().expect("input is an object");
    if let Some(Value::Array(ids)) = object.get_mut("labelIds") {
        if ids.is_empty() {
            object.remove("labelIds");
        } else {
            ids.sort_by_key(|id| id.to_string());
        }
    }
    input
}

#[test]
fn create_with_every_field_sends_resolved_ids() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
        .on("GetWorkflowStates", states())
        .on("GetViewerId", json!({ "viewer": { "id": "user-me" } }))
        .on("GetIssueLabelIdByNameForTeam", label("label-bug", "Bug"))
        .on("GetIssueLabelIdByNameForTeam", label("label-ui", "UI"))
        .on("GetProjectIdByName", json!({ "projects": { "nodes": [{ "id": PROJECT_ID }] } }))
        .on("GetProjectMilestonesForLookup", milestones())
        .on("GetTeamCyclesForLookup", cycles())
        .on("GetIssueId", json!({ "issue": { "id": PARENT_ID } }))
        .on(
            "GetParentIssueData",
            json!({ "issue": { "title": "Parent", "identifier": "ENG-9", "project": { "id": PROJECT_ID } } }),
        )
        .on("CreateIssue", created("ENG-42"));
    Cli::for_api(&api)
        .run(&[
            "issue",
            "create",
            "--no-interactive",
            "--no-use-default-template",
            "-t",
            "Fix the login",
            "--team",
            "eng",
            "-a",
            "self",
            "-l",
            "Bug",
            "-l",
            "UI",
            "--project",
            "Roadmap",
            "--milestone",
            "Beta",
            "--cycle",
            "active",
            "-s",
            "started",
            "-p",
            "2",
            "--estimate",
            "3",
            "--due-date",
            "2026-01-31",
            "--parent",
            "ENG-9",
            "-d",
            "Steps to reproduce",
        ])
        .success()
        .stdout_has("ENG-42");
    assert_eq!(
        input(&api, "CreateIssue"),
        json!({
            "title": "Fix the login",
            "assigneeId": "user-me",
            "dueDate": "2026-01-31",
            "parentId": PARENT_ID,
            "priority": 2,
            "estimate": 3,
            "labelIds": ["label-bug", "label-ui"],
            "teamId": ENG_ID,
            "projectId": PROJECT_ID,
            "projectMilestoneId": "milestone-1",
            "cycleId": "cycle-active",
            "stateId": "state-progress",
            "useDefaultTemplate": false,
            "description": "Steps to reproduce",
        })
    );
    assert_eq!(api.variables("ResolveTeam")["reference"], "eng");
    assert_eq!(api.variables("GetIssueId"), json!({ "id": "ENG-9" }));
    assert_eq!(api.variables("GetProjectIdByName")["name"], "Roadmap");
    let mut label_names: Vec<Value> = api
        .requests()
        .into_iter()
        .filter(|r| r.operation.as_deref() == Some("GetIssueLabelIdByNameForTeam"))
        .map(|r| r.variables["name"].clone())
        .collect();
    label_names.sort_by_key(|name| name.to_string());
    assert_eq!(label_names, [json!("Bug"), json!("UI")]);
}

#[test]
fn create_uses_the_configured_team_and_default_template() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
        .on("CreateIssue", created("ENG-7"));
    Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .run(&["issue", "create", "--no-interactive", "-t", "Plain"])
        .success()
        .stdout_has("✓ Created issue ENG-7: New issue\nhttps://linear.app/acme/issue/ENG-7/new\n");
    assert_eq!(api.variables("ResolveTeam"), resolve_vars("ENG"));
    assert_eq!(
        input(&api, "CreateIssue"),
        json!({ "title": "Plain", "teamId": ENG_ID, "useDefaultTemplate": true })
    );
}

#[test]
fn create_prints_its_progress_on_stderr_and_only_the_result_on_stdout() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
        .on("CreateIssue", created("ENG-7"));
    let run = Cli::for_api(&api).env("LINEAR_TEAM_ID", "ENG").run(&[
        "issue",
        "create",
        "--no-interactive",
        "-t",
        "Plain",
    ]);
    run.success().stderr_has("Creating issue in ENG\n");
    assert_eq!(
        run.stdout,
        "✓ Created issue ENG-7: New issue\nhttps://linear.app/acme/issue/ENG-7/new\n"
    );
}

#[test]
fn create_reads_the_description_file_and_looks_up_assignees() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
        .on(
            "LookupUser",
            json!({ "users": { "nodes": [{
                "id": "user-ada", "email": "ada@example.com", "displayName": "ada", "name": "Ada Lovelace"
            }] } }),
        )
        .on("CreateIssue", created("ENG-8"));
    let cli = Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .file("cwd/body.md", "# Heading\n\nBody text\n");
    cli.run(&[
        "issue",
        "create",
        "--no-interactive",
        "-t",
        "With body",
        "--description-file",
        "body.md",
        "-a",
        "ada",
    ])
    .success();
    assert_eq!(api.variables("LookupUser"), json!({ "input": "ada" }));
    let input = input(&api, "CreateIssue");
    assert_eq!(input["assigneeId"], "user-ada");
    assert_eq!(
        input["description"].as_str().map(str::trim_end),
        Some("# Heading\n\nBody text")
    );
}

#[test]
fn create_from_a_named_template_needs_no_title() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
        .on(
            "GetTemplates",
            json!({ "templates": [template("template-bug", "Bug report", "issue")] }),
        )
        .on("CreateIssue", created("ENG-9"));
    Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .run(&[
            "issue",
            "create",
            "--no-interactive",
            "--template",
            "Bug report",
        ])
        .success();
    assert_eq!(
        input(&api, "CreateIssue"),
        json!({ "teamId": ENG_ID, "templateId": "template-bug" })
    );
}

#[test]
fn create_rejects_a_non_issue_template() {
    let api = MockLinear::start();
    let id = "00000000-0000-4000-8000-000000000010";
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
        .on(
            "GetTemplate",
            json!({ "template": template(id, "Launch", "project") }),
        );
    Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .run(&["issue", "create", "--no-interactive", "--template", id])
        .failure()
        .stderr_has("project template");
    assert_eq!(api.variables("GetTemplate"), json!({ "id": id }));
}

#[test]
fn create_reports_an_unknown_template_id_as_not_found() {
    let api = MockLinear::start();
    let id = "00000000-0000-4000-8000-000000000010";
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
        .on_error("GetTemplate", "Entity not found")
        .on("GetTemplates", json!({ "templates": [] }));
    Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .run(&["issue", "create", "--no-interactive", "--template", id])
        .failure()
        .stderr_has(&format!("Template not found: {id}"));
}

#[test]
fn create_fails_on_an_unknown_label_without_creating() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
        .on(
            "GetIssueLabelIdByNameForTeam",
            json!({ "issueLabels": { "nodes": [] } }),
        );
    Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .run(&[
            "issue",
            "create",
            "--no-interactive",
            "-t",
            "x",
            "-l",
            "missing",
        ])
        .failure()
        .stderr_has("missing");
    assert_eq!(
        api.variables("GetIssueLabelIdByNameForTeam"),
        json!({ "name": "missing", "teamKey": "ENG" })
    );
}

#[test]
fn create_reports_an_unsuccessful_mutation() {
    let api = MockLinear::start();
    let mut rejected = created("ENG-1");
    rejected["issueCreate"]["success"] = json!(false);
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
        .on("CreateIssue", rejected);
    Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .run(&["issue", "create", "--no-interactive", "-t", "x"])
        .failure()
        .stderr_has("Linear did not create the issue");
}

#[test]
fn create_without_title_or_team_fails_before_any_request() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api);
    cli.run(&["issue", "create", "--no-interactive", "-t", "No team"])
        .usage_error();
    cli.env("LINEAR_TEAM_ID", "ENG")
        .run(&["issue", "create", "--no-interactive"])
        .usage_error();
    assert!(api.requests().is_empty(), "{:?}", api.operations());
}

#[test]
fn create_rejects_a_non_numeric_estimate_before_any_request() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .run(&[
            "issue",
            "create",
            "--no-interactive",
            "-t",
            "x",
            "--estimate",
            "lots",
        ])
        .usage_error();
    assert!(api.requests().is_empty(), "{:?}", api.operations());
}

#[test]
fn create_rejects_invalid_typed_values_before_any_request() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api).env("LINEAR_TEAM_ID", "ENG");
    for flags in [
        ["-p", "9"],
        ["-p", "2.5"],
        ["--estimate", "-1"],
        ["--estimate", "2.5"],
        ["--due-date", "tomorrow"],
        ["--due-date", "2026-02-30"],
    ] {
        let mut args = vec!["issue", "create", "--no-interactive", "-t", "x"];
        args.extend(flags);
        cli.run(&args).usage_error().stderr_has(flags[0]);
    }
    assert!(api.requests().is_empty(), "{:?}", api.operations());
}

/// The `UpdateIssue` input, which never moves the issue without `--team`.
fn update_input(api: &MockLinear) -> Value {
    let input = input(api, "UpdateIssue");
    assert!(input.get("teamId").is_none(), "{input}");
    input
}

#[test]
fn update_with_every_field_sends_resolved_ids() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved("team-app-id", "APP", "Apps"))
        .on("GetWorkflowStates", states())
        .on("GetViewerId", json!({ "viewer": { "id": "user-me" } }))
        .on("GetIssueLabelIdByNameForTeam", label("label-bug", "Bug"))
        .on(
            "GetProjectIdByName",
            json!({ "projects": { "nodes": [{ "id": PROJECT_ID }] } }),
        )
        .on("GetProjectMilestonesForLookup", milestones())
        .on("GetTeamCyclesForLookup", cycles())
        .on("GetIssueId", json!({ "issue": { "id": PARENT_ID } }))
        .on("UpdateIssue", updated("APP-3", "Renamed"));
    Cli::for_api(&api)
        .run(&[
            "issue",
            "update",
            "ENG-1",
            "--team",
            "APP",
            "-s",
            "Done",
            "-a",
            "self",
            "-l",
            "Bug",
            "--project",
            "Roadmap",
            "--milestone",
            "Beta",
            "--cycle",
            "next",
            "--parent",
            "ENG-9",
            "--due-date",
            "2026-02-01",
            "--estimate",
            "5",
            "-p",
            "Urgent",
            "-t",
            "Renamed",
            "-d",
            "New body",
        ])
        .success()
        .stdout_has("✓ Updated issue APP-3: Renamed\nhttps://linear.app/acme/issue/APP-3/x\n");
    let request = api.variables("UpdateIssue");
    assert_eq!(request["id"], "ENG-1");
    assert_eq!(
        input(&api, "UpdateIssue"),
        json!({
            "title": "Renamed",
            "assigneeId": "user-me",
            "dueDate": "2026-02-01",
            "parentId": PARENT_ID,
            "priority": 1,
            "estimate": 5,
            "labelIds": ["label-bug"],
            "teamId": "team-app-id",
            "projectId": PROJECT_ID,
            "projectMilestoneId": "milestone-1",
            "cycleId": "cycle-next",
            "stateId": "state-done",
            "description": "New body",
        })
    );
    assert_eq!(api.variables("ResolveTeam")["reference"], "APP");
    assert_eq!(api.variables("GetWorkflowStates")["teamKey"], "APP");
}

#[test]
fn update_clears_fields_with_nulls() {
    let api = MockLinear::start();
    api.on("UpdateIssue", updated("ENG-1", "Same"));
    Cli::for_api(&api)
        .run(&[
            "issue",
            "update",
            "ENG-1",
            "--unassign",
            "--clear-due-date",
            "--clear-parent",
            "--clear-estimate",
            "--clear-project",
            "--clear-cycle",
        ])
        .success();
    assert_eq!(
        update_input(&api),
        json!({
            "assigneeId": null, "dueDate": null, "parentId": null, "estimate": null,
            "projectId": null, "cycleId": null,
        })
    );
}

#[test]
fn update_milestone_uses_the_issue_project() {
    let api = MockLinear::start();
    api.on(
        "GetIssueProjectId",
        json!({ "issue": { "project": { "id": PROJECT_ID } } }),
    )
    .on("GetProjectMilestonesForLookup", milestones())
    .on("UpdateIssue", updated("ENG-1", "Same"));
    Cli::for_api(&api)
        .run(&["issue", "update", "ENG-1", "--milestone", "Beta"])
        .success();
    assert_eq!(api.variables("GetIssueProjectId"), json!({ "id": "ENG-1" }));
    assert_eq!(
        api.variables("GetProjectMilestonesForLookup"),
        json!({ "projectId": PROJECT_ID, "name": "Beta" })
    );
    assert_eq!(
        update_input(&api),
        json!({ "projectMilestoneId": "milestone-1" })
    );
}

#[test]
fn update_conflicting_flags_fail_before_any_request() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api);
    for args in [
        ["--assignee", "self", "--unassign"],
        ["--due-date", "2026-01-01", "--clear-due-date"],
        ["--project", "Roadmap", "--clear-project"],
        ["--estimate", "2", "--clear-estimate"],
    ] {
        let mut argv = vec!["issue", "update", "ENG-1"];
        argv.extend(args);
        let run = cli.run(&argv);
        run.failure();
    }
    assert!(api.requests().is_empty(), "{:?}", api.operations());
}

#[test]
fn update_resolves_the_issue_team_for_states_without_moving_the_issue() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
        .on("GetWorkflowStates", states())
        .on("UpdateIssue", updated("ENG-1", "Same"));
    Cli::for_api(&api)
        .run(&["issue", "update", "ENG-1", "-s", "Done"])
        .success();
    assert_eq!(api.variables("ResolveTeam")["reference"], "ENG");
    assert_eq!(update_input(&api), json!({ "stateId": "state-done" }));
}

#[test]
fn update_without_changes_fails_before_any_request() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&["issue", "update", "ENG-1"])
        .usage_error()
        .stderr_has("No changes given");
    assert!(api.requests().is_empty(), "{:?}", api.operations());
}

#[test]
fn update_reports_an_unsuccessful_mutation() {
    let api = MockLinear::start();
    let mut rejected = updated("ENG-1", "x");
    rejected["issueUpdate"]["success"] = json!(false);
    api.on("UpdateIssue", rejected);
    Cli::for_api(&api)
        .run(&["issue", "update", "ENG-1", "-t", "x"])
        .failure();
}

#[test]
fn update_of_a_missing_issue_names_it_and_prints_nothing_first() {
    let api = MockLinear::start();
    api.on_raw(
        "UpdateIssue",
        200,
        r#"{"errors":[{"message":"Entity not found: Issue","extensions":{"userPresentableMessage":"Could not find referenced Issue."}}]}"#,
    );
    let run = Cli::for_api(&api).run(&["issue", "update", "ENG-9999", "-t", "x"]);
    run.failure().stderr_has("Issue not found: ENG-9999");
    assert_eq!(run.stdout, "", "{run}");
}

fn archive_details(archived_at: Value) -> Value {
    json!({ "issue": { "identifier": "ENG-1", "title": "Old work", "archivedAt": archived_at } })
}

#[test]
fn archive_by_url_with_confirm() {
    let api = MockLinear::start();
    api.on("GetIssueSummary", archive_details(Value::Null)).on(
        "ArchiveIssue",
        json!({ "issueArchive": { "success": true } }),
    );
    Cli::for_api(&api)
        .run(&[
            "issue",
            "archive",
            "https://linear.app/acme/issue/eng-1/old-work",
            "-y",
        ])
        .success()
        .stdout_has("✓ Archived issue ENG-1: Old work\n");
    assert_eq!(api.variables("GetIssueSummary"), json!({ "id": "ENG-1" }));
    assert_eq!(api.variables("ArchiveIssue"), json!({ "id": "ENG-1" }));
}

#[test]
fn archive_of_an_archived_issue_is_a_no_op() {
    let api = MockLinear::start();
    api.on(
        "GetIssueSummary",
        archive_details(json!("2026-01-01T00:00:00.000Z")),
    );
    Cli::for_api(&api)
        .run(&["issue", "archive", "ENG-1", "--yes"])
        .success()
        .stdout_has("already archived");
}

#[test]
fn archive_without_confirmation_or_terminal_fails_before_any_request() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&["issue", "archive", "ENG-1"])
        .usage_error()
        .stderr_has("--yes");
    assert!(api.requests().is_empty());
}

#[test]
fn archive_bulk_archives_each_issue() {
    let api = MockLinear::start();
    for id in ["ENG-1", "ENG-2"] {
        api.on(
            "GetIssueSummary",
            json!({ "issue": { "identifier": id, "title": "t", "archivedAt": null } }),
        )
        .on(
            "ArchiveIssue",
            json!({ "issueArchive": { "success": true } }),
        );
    }
    Cli::for_api(&api)
        .run(&["issue", "archive", "--yes", "--bulk", "ENG-1", "ENG-2"])
        .success();
    let mut archived: Vec<Value> = api
        .requests()
        .into_iter()
        .filter(|r| r.operation.as_deref() == Some("ArchiveIssue"))
        .map(|r| r.variables["id"].clone())
        .collect();
    archived.sort_by_key(|id| id.to_string());
    assert_eq!(archived, [json!("ENG-1"), json!("ENG-2")]);
}

#[test]
fn delete_with_confirm() {
    let api = MockLinear::start();
    api.on(
        "GetIssueSummary",
        json!({ "issue": { "identifier": "ENG-3", "title": "Mistake", "archivedAt": null } }),
    )
    .on(
        "DeleteIssue",
        json!({ "issueDelete": { "success": true, "entity": null } }),
    );
    Cli::for_api(&api)
        .run(&["issue", "delete", "eng-3", "--yes"])
        .success()
        .stdout_has("✓ Deleted issue ENG-3: Mistake\n");
    assert_eq!(api.variables("GetIssueSummary"), json!({ "id": "ENG-3" }));
    assert_eq!(api.variables("DeleteIssue"), json!({ "id": "ENG-3" }));
}

#[test]
fn delete_without_confirmation_or_terminal_fails_before_any_request() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&["issue", "delete", "ENG-3"])
        .usage_error()
        .stderr_has("--yes");
    assert!(api.requests().is_empty());
}

#[test]
fn delete_rejects_a_positional_issue_with_bulk() {
    Cli::new()
        .run(&["issue", "delete", "ENG-9", "--yes", "--bulk", "ENG-6"])
        .usage_error()
        .stderr_has("--bulk");
}

#[test]
fn delete_reports_a_missing_issue() {
    let api = MockLinear::start();
    api.on_raw(
        "GetIssueSummary",
        200,
        r#"{"errors":[{"message":"Entity not found: Issue","extensions":{"userPresentableMessage":"Could not find referenced Issue."}}]}"#,
    );
    Cli::for_api(&api)
        .run(&["issue", "delete", "ENG-404", "-y"])
        .failure()
        .stderr_has("Issue not found: ENG-404");
}

#[test]
fn delete_bulk_reads_identifiers_from_a_file() {
    let api = MockLinear::start();
    for id in ["ENG-5", "ENG-6"] {
        api.on(
            "GetIssueSummary",
            json!({ "issue": { "identifier": id, "title": "t", "archivedAt": null } }),
        )
        .on("DeleteIssue", json!({ "issueDelete": { "success": true } }));
    }
    Cli::for_api(&api)
        .file("cwd/ids.txt", "ENG-5\nENG-6\n")
        .run(&["issue", "delete", "--yes", "--bulk-file", "ids.txt"])
        .success()
        .stdout_has("2");
    let mut deleted: Vec<Value> = api
        .requests()
        .into_iter()
        .filter(|r| r.operation.as_deref() == Some("DeleteIssue"))
        .map(|r| r.variables["id"].clone())
        .collect();
    deleted.sort_by_key(|id| id.to_string());
    assert_eq!(deleted, [json!("ENG-5"), json!("ENG-6")]);
}

#[test]
fn archive_bulk_reports_unusable_references_and_archives_the_rest() {
    let api = MockLinear::start();
    api.on(
        "GetIssueSummary",
        json!({ "issue": { "identifier": "ENG-1", "title": "t", "archivedAt": null } }),
    )
    .on(
        "ArchiveIssue",
        json!({ "issueArchive": { "success": true } }),
    );
    let run = Cli::for_api(&api).run(&[
        "issue",
        "archive",
        "--yes",
        "--bulk",
        "3",
        "https://linear.app/acme/settings/x",
        "eng-1",
    ]);
    run.failure()
        .stdout_has("Issue number 3 needs a team")
        .stdout_has("is not an entity this command can use");
    let ids: Vec<Value> = api
        .requests()
        .into_iter()
        .map(|r| r.variables["id"].clone())
        .collect();
    assert_eq!(ids, [json!("ENG-1"), json!("ENG-1")]);
}

#[test]
fn update_refuses_to_add_and_remove_the_same_label() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"));
    api.on("GetIssueLabelIdByNameForTeam", label("label-bug", "Bug"))
        .on("GetIssueLabelIdByNameForTeam", label("label-bug", "Bug"));
    Cli::for_api(&api)
        .run(&[
            "issue",
            "update",
            "ENG-1",
            "--add-label",
            "Bug",
            "--remove-label",
            "bug",
        ])
        .failure()
        .stderr_has("Cannot add and remove the same label in one update");
    assert!(!api.operations().contains(&"UpdateIssue".to_owned()));
}

#[test]
fn create_fails_when_the_parent_metadata_request_fails() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
        .on("GetIssueId", json!({ "issue": { "id": PARENT_ID } }))
        .on_error("GetParentIssueData", "Rate limit exceeded");
    Cli::for_api(&api)
        .run(&[
            "issue",
            "create",
            "--no-interactive",
            "--no-use-default-template",
            "-t",
            "Child",
            "--team",
            "eng",
            "--parent",
            "ENG-9",
        ])
        .failure()
        .stderr_has("Rate limit exceeded");
    assert!(!api.operations().contains(&"CreateIssue".to_owned()));
}

#[test]
fn delete_bulk_skips_issues_whose_lookup_fails() {
    let api = MockLinear::start();
    api.on_error("GetIssueSummary", "Rate limit exceeded");
    let run = Cli::for_api(&api).run(&["issue", "delete", "--confirm", "--bulk", "ENG-5"]);
    run.failure().stderr_has("Rate limit exceeded");
    assert_eq!(api.operations(), ["GetIssueSummary"]);
}

#[test]
fn missing_state_hints_quote_names_before_issue_mutations() {
    for command in ["create", "update"] {
        let api = MockLinear::start();
        let reply = json!({ "team": { "states": { "nodes": [
            { "id": "say", "name": "Say \"hi\"", "type": "started", "position": 2 },
            { "id": "bell", "name": "Bell\u{7}", "type": "unstarted", "position": 1 },
        ], "pageInfo": { "hasNextPage": false, "endCursor": null } } } });
        api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
            .on("GetWorkflowStates", reply);
        let argv = if command == "create" {
            vec![
                "issue",
                "create",
                "--no-interactive",
                "-t",
                "Title",
                "--team",
                "ENG",
                "--state",
                "Absent",
            ]
        } else {
            vec!["issue", "update", "ENG-1", "--state", "Absent"]
        };
        let run = Cli::for_api(&api).run(&argv);
        run.failure()
            .stderr_has("Workflow state not found: 'Absent' in team ENG")
            .stderr_has(r#"Valid states: "Bell\u0007" (unstarted), "Say \"hi\"" (started)."#);
        assert!(!run.stderr.contains('\u{7}'));
        assert_eq!(api.operations(), ["ResolveTeam", "GetWorkflowStates"]);
    }
}

#[test]
fn state_type_lookup_chooses_the_lowest_position_and_first_tie_after_name_lookup() {
    for (reference, expected) in [("STARTED", "lowest-first"), ("Unstarted", "named-type")] {
        let api = MockLinear::start();
        api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
            .on(
                "GetWorkflowStates",
                json!({ "team": { "states": { "nodes": [
                { "id": "higher", "name": "Later", "type": "started", "position": 9 },
                { "id": "lowest-first", "name": "First", "type": "started", "position": 1 },
                { "id": "lowest-tied", "name": "Tied", "type": "started", "position": 1 },
                { "id": "named-type", "name": "Unstarted", "type": "completed", "position": 4 },
                { "id": "unstarted", "name": "Todo", "type": "unstarted", "position": 1 },
            ], "pageInfo": { "hasNextPage": false, "endCursor": null } } } }),
            )
            .on("UpdateIssue", updated("ENG-1", "Same"));
        Cli::for_api(&api)
            .run(&["issue", "update", "ENG-1", "--state", reference])
            .success();
        assert_eq!(update_input(&api), json!({ "stateId": expected }));
    }
}

#[test]
fn state_url_rejection_keeps_workflow_lookup_before_validation() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
        .on("GetWorkflowStates", states());
    Cli::for_api(&api)
        .run(&[
            "issue",
            "update",
            "ENG-1",
            "--state",
            "https://linear.app/acme/issue/ENG-2/other",
        ])
        .failure()
        .stderr_has("workflow state name or type");
    assert_eq!(api.operations(), ["ResolveTeam", "GetWorkflowStates"]);
}

#[test]
fn create_reports_uncertain_outcomes_without_retrying() {
    for uncertain in [false, true] {
        let api = MockLinear::start();
        api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"));
        if uncertain {
            api.on_raw("CreateIssue", 200, "not json");
        } else {
            api.on_error("CreateIssue", "Title is required");
        }
        let run = Cli::for_api(&api).env("LINEAR_TEAM_ID", "ENG").run(&[
            "issue",
            "create",
            "--no-interactive",
            "-t",
            "Plain",
        ]);
        run.failure().stderr_has(if uncertain {
            "issue may already exist"
        } else {
            "Title is required"
        });
        assert_eq!(run.stderr.contains("may already exist"), uncertain);
        assert_eq!(api.operations(), ["ResolveTeam", "CreateIssue"]);
        assert_eq!(
            input(&api, "CreateIssue"),
            json!({"title": "Plain", "teamId": ENG_ID, "useDefaultTemplate": true})
        );
    }
}

#[test]
fn create_reports_a_missing_parent_without_mutating() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"));
    api.on_raw("GetIssueId", 200, r#"{"errors":[{"message":"Entity not found: Issue","extensions":{"userPresentableMessage":"Could not find referenced Issue."}}]}"#);
    Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .run(&[
            "issue",
            "create",
            "--no-interactive",
            "--no-use-default-template",
            "--title",
            "Child",
            "--parent",
            "ENG-404",
        ])
        .failure()
        .stderr_has("Parent issue not found: ENG-404");
    assert_eq!(api.operations(), ["ResolveTeam", "GetIssueId"]);
    assert_eq!(api.variables("GetIssueId"), json!({"id": "ENG-404"}));
}

#[test]
fn update_reports_a_missing_parent_without_mutating() {
    let api = MockLinear::start();

    api.on_raw("GetIssueId", 200, r#"{"errors":[{"message":"Entity not found: Issue","extensions":{"userPresentableMessage":"Could not find referenced Issue."}}]}"#);
    Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .run(&["issue", "update", "ENG-1", "--parent", "ENG-404"])
        .failure()
        .stderr_has("Parent issue not found: ENG-404");
    assert_eq!(api.operations(), ["GetIssueId"]);
    assert_eq!(api.variables("GetIssueId"), json!({"id": "ENG-404"}));
}

#[test]
fn parent_lookup_preserves_unrelated_errors_without_mutating() {
    for create in [false, true] {
        let api = MockLinear::start();
        if create {
            api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"));
        }
        api.on_error("GetIssueId", "Rate limit exceeded");
        let args = if create {
            vec![
                "issue",
                "create",
                "--no-interactive",
                "--no-use-default-template",
                "--title",
                "Child",
                "--parent",
                "ENG-404",
            ]
        } else {
            vec!["issue", "update", "ENG-1", "--parent", "ENG-404"]
        };
        let run = Cli::for_api(&api).env("LINEAR_TEAM_ID", "ENG").run(&args);
        run.failure().stderr_has("Rate limit exceeded");
        assert!(!run.stderr.contains("Parent issue not found"));
        let expected = if create {
            vec!["ResolveTeam", "GetIssueId"]
        } else {
            vec!["GetIssueId"]
        };
        assert_eq!(api.operations(), expected);
        assert_eq!(api.variables("GetIssueId"), json!({"id": "ENG-404"}));
    }
}

fn all_teams() -> Value {
    json!({ "teams": {
        "nodes": [
            { "id": "team-ops-id", "key": "OPS", "name": "Operations" },
            { "id": ENG_ID, "key": "ENG", "name": "Engineering" }
        ],
        "pageInfo": { "hasNextPage": false, "endCursor": null }
    } })
}

#[test]
fn create_on_a_terminal_picks_a_team_before_asking_for_the_title() {
    let api = MockLinear::start();
    api.on("GetAllTeams", all_teams())
        .on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
        .on("CreateIssue", created("ENG-7"));
    Cli::for_api(&api)
        .run_tty(
            &["issue", "create"],
            &[
                ("Engineering (ENG)", "\r"),
                ("Title:", "Fix it\r"),
                ("Create issue \"Fix it\" in ENG? (y/N)", "y\r"),
            ],
        )
        .success()
        .stdout_has("✓ Created issue ENG-7");
    assert_eq!(api.variables("ResolveTeam"), resolve_vars("ENG"));
    assert_eq!(input(&api, "CreateIssue")["title"], "Fix it");
}

#[test]
fn create_on_a_terminal_creates_nothing_unless_confirmed() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"));
    Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .run_tty(
            &["issue", "create"],
            &[("Title:", "Fix it\r"), ("(y/N)", "\r")],
        )
        .success()
        .stdout_has("Canceled.");
    assert_eq!(api.operations(), ["ResolveTeam"]);
}

#[test]
fn create_with_yes_skips_the_question_after_a_typed_title() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
        .on("CreateIssue", created("ENG-7"));
    let run = Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .run_tty(&["issue", "create", "--yes"], &[("Title:", "Fix it\r")]);
    run.success();
    assert!(!run.stdout.contains("(y/N)"), "{run}");
    assert_eq!(api.operations(), ["ResolveTeam", "CreateIssue"]);
}

fn label_page(labels: Value, next: Option<&str>) -> Value {
    json!({ "issueLabels": {
        "nodes": labels,
        "pageInfo": { "hasNextPage": next.is_some(), "endCursor": next }
    } })
}

/// The lookups of the `--interactive` wizard for an issue in ENG, with one
/// page of labels per entry of `labels`.
fn wizard_api(api: &MockLinear, labels: &[Value]) {
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
        .on(
            "GetWorkflowStates",
            json!({ "team": { "states": {
                "nodes": [{ "id": "state-todo", "name": "Todo", "type": "unstarted", "position": 1 }],
                "pageInfo": { "hasNextPage": false, "endCursor": null }
            } } }),
        );
    for (index, page) in labels.iter().enumerate() {
        let next = (index + 1 < labels.len()).then(|| format!("cursor-{index}"));
        api.on("GetIssueLabels", label_page(page.clone(), next.as_deref()));
    }
}

/// The `--interactive` wizard for an issue in ENG, answered up to its final
/// question, which gets `last`.
fn run_wizard(api: &MockLinear, last: &str) -> Run {
    wizard_api(api, &[json!([])]);
    Cli::for_api(api)
        .env("LINEAR_TEAM_ID", "ENG")
        .env("LINEAR_ISSUE_CREATE_ASSIGN_SELF", "never")
        .run_tty(
            &["issue", "create", "-i"],
            &[
                ("Title:", "Fix it\r"),
                ("Description (optional", "\r"),
                ("Next:", "\r"),
                ("Start working on this issue now?", "\r"),
                ("Create issue \"Fix it\" in ENG? (y/N)", last),
            ],
        )
}

#[test]
fn the_create_wizard_offers_every_team_and_workspace_label() {
    let api = MockLinear::start();
    let label = |id: &str, name: &str, team: Value| json!({ "id": id, "name": name, "description": null, "color": "#000000", "team": team });
    wizard_api(
        &api,
        &[
            json!([label(
                "label-bug",
                "Bug",
                json!({ "key": "ENG", "name": "Engineering" })
            )]),
            json!([label("label-customer", "Customer", Value::Null)]),
        ],
    );
    api.on("CreateIssue", created("ENG-7"));
    let down = "\u{1b}[B";
    Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .env("LINEAR_ISSUE_CREATE_ASSIGN_SELF", "never")
        .run_tty(
            &["issue", "create", "-i"],
            &[
                ("Title:", "Fix it\r"),
                ("Description (optional", "\r"),
                ("Next:", &format!("{down}\r")),
                ("More fields:", &format!("{down}{down}{down} \r")),
                ("Customer", &format!("{down} \r")),
                ("Start working on this issue now?", "\r"),
                ("Create issue \"Fix it\" in ENG? (y/N)", "y\r"),
            ],
        )
        .success();
    assert_eq!(
        input(&api, "CreateIssue")["labelIds"],
        json!(["label-customer"])
    );
    let filters: Vec<Value> = api
        .requests()
        .into_iter()
        .filter(|request| request.operation.as_deref() == Some("GetIssueLabels"))
        .map(|request| request.variables)
        .collect();
    assert_eq!(filters.len(), 2);
    assert_eq!(
        filters[0]["filter"],
        json!({ "or": [
            { "team": { "key": { "eq": "ENG" } } },
            { "team": { "null": true } }
        ] })
    );
    assert_eq!(filters[1]["after"], "cursor-0");
}

#[test]
fn the_create_wizard_creates_nothing_unless_confirmed() {
    let api = MockLinear::start();
    run_wizard(&api, "\r").success().stdout_has("Canceled.");
    // States and labels load together, in either order.
    let mut operations = api.operations();
    operations.sort();
    assert_eq!(
        operations,
        ["GetIssueLabels", "GetWorkflowStates", "ResolveTeam"]
    );
}

#[test]
fn the_create_wizard_creates_the_issue_once_confirmed() {
    let api = MockLinear::start();
    api.on("CreateIssue", created("ENG-7"));
    // The wizard reports progress like the flag form does.
    run_wizard(&api, "y\r")
        .success()
        .stdout_has("Creating issue in ENG\n✓ Created issue ENG-7");
    assert_eq!(input(&api, "CreateIssue")["title"], "Fix it");
}

#[test]
fn empty_field_values_are_usage_errors_before_any_request() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api).env("LINEAR_TEAM_ID", "ENG");
    for args in [
        &["issue", "update", "ENG-1", "--title", ""][..],
        &["issue", "update", "ENG-1", "--project="],
        &["issue", "update", "ENG-1", "--state", ""],
        &["issue", "update", "ENG-1", "--add-label", ""],
        &["issue", "create", "--title", ""],
        &["issue", "create", "-t", "x", "--team", ""],
        &["issue", "create", "-t", "x", "--label", ""],
    ] {
        cli.run(args)
            .usage_error()
            .stderr_has("a value is required");
    }
    assert!(api.requests().is_empty(), "{:?}", api.operations());
}

#[test]
fn cycle_flags_accept_a_negative_offset_without_an_equals_sign() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api);
    for args in [
        &["issue", "update", "ENG-1", "--cycle", "-1", "--help"][..],
        &["issue", "create", "--cycle", "-2", "--help"],
        &["issue", "query", "--cycle", "-1", "--help"],
        &["document", "list", "--cycle", "-1", "--help"],
    ] {
        cli.run(args).success();
    }
    cli.run(&["issue", "update", "ENG-1", "--cycle", "--title", "x"])
        .usage_error();
    assert!(api.requests().is_empty());
}

#[test]
fn create_reads_the_description_from_stdin_with_a_dash() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
        .on("CreateIssue", created("ENG-8"));
    Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .stdin(b"Piped description\n")
        .run(&["issue", "create", "-t", "Piped", "--description-file", "-"])
        .success();
    assert_eq!(
        input(&api, "CreateIssue")["description"],
        "Piped description\n"
    );
}

#[test]
fn whitespace_only_flag_values_are_usage_errors() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api);
    for args in [
        &["issue", "create", "--title", "   ", "--team", "ENG"][..],
        &["issue", "update", "ENG-1", "--title", " \t"],
        &["issue", "create", "--title", "Real", "--team", "  "],
        &["project", "create", "--name", " "],
    ] {
        cli.run(args)
            .usage_error()
            .stderr_has("the value is only whitespace");
    }
    assert!(api.requests().is_empty());
}

const MISSING_ISSUE: &str = r#"{"errors":[{"message":"Entity not found: Issue","extensions":{"userPresentableMessage":"Could not find referenced Issue."}}]}"#;

#[test]
fn bulk_delete_lists_what_it_found_and_skips_before_asking() {
    let api = MockLinear::start();
    api.on(
        "GetIssueSummary",
        json!({ "issue": { "identifier": "ENG-1", "title": "Fix login", "archivedAt": null } }),
    )
    .on("DeleteIssue", json!({ "issueDelete": { "success": true } }));
    let run = Cli::for_api(&api).run_tty(
        &["issue", "delete", "--bulk", "ENG-1", "3"],
        &[("Delete 1 issue? (y/N)", "y\r")],
    );
    assert_eq!(run.code, 1, "{run}");
    let listed = "1 issue to delete:\n  ENG-1: Fix login\n\
                  Skipping 1 issue that could not be found:\n  3: Issue number 3 needs a team";
    assert!(run.stdout.contains(listed), "{run}");
    assert!(
        run.stdout.contains("Completed: 1/2 issues deleted"),
        "{run}"
    );
    assert_eq!(api.operations(), ["GetIssueSummary", "DeleteIssue"]);
}

#[test]
fn bulk_archive_with_nothing_found_asks_nothing() {
    let api = MockLinear::start();
    api.on_raw("GetIssueSummary", 200, MISSING_ISSUE);
    let run = Cli::for_api(&api).run_tty(&["issue", "archive", "--bulk", "ENG-404"], &[]);
    assert_eq!(run.code, 1, "{run}");
    assert!(run.stdout.contains("ENG-404: Issue not found"), "{run}");
    assert!(
        run.stdout
            .contains("None of the listed issues could be found"),
        "{run}"
    );
    assert!(!run.stdout.contains("(y/N)"), "{run}");
    assert_eq!(api.operations(), ["GetIssueSummary"]);
}
