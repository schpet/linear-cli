//! `issue create`, `issue update`, `issue archive` and `issue delete`.
use serde_json::{Value, json};

use crate::support::{Cli, MockLinear};
use crate::team::{resolve_vars, resolved};

const ENG_ID: &str = "team-eng-id";
const PARENT_ID: &str = "issue-parent-id";
const PROJECT_ID: &str = "project-roadmap-id";

fn created(identifier: &str) -> Value {
    json!({
        "issueCreate": {
            "success": true,
            "issue": {
                "id": "issue-new-id", "identifier": identifier,
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
    ] } } })
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
        .stdout_has("https://linear.app/acme/issue/ENG-7/new");
    assert_eq!(api.variables("ResolveTeam"), resolve_vars("ENG"));
    assert_eq!(
        input(&api, "CreateIssue"),
        json!({ "title": "Plain", "teamId": ENG_ID, "useDefaultTemplate": true })
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
        .failure();
}

#[test]
fn create_without_title_or_team_fails_before_any_request() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api);
    cli.run(&["issue", "create", "--no-interactive", "-t", "No team"])
        .failure();
    cli.env("LINEAR_TEAM_ID", "ENG")
        .run(&["issue", "create", "--no-interactive"])
        .failure();
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

/// Without `--team`, updates resolve the team from the issue identifier and send it back as
/// `teamId`. That is redundant, so `update_input` leaves it out.
fn expect_own_team(api: &MockLinear) {
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"));
}

fn update_input(api: &MockLinear) -> Value {
    let mut input = input(api, "UpdateIssue");
    let team = input
        .as_object_mut()
        .expect("input object")
        .remove("teamId");
    assert!(matches!(team, None | Some(Value::String(_))), "{team:?}");
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
        .stdout_has("APP-3");
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
    expect_own_team(&api);
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
    expect_own_team(&api);
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
        assert_ne!(run.code, 0, "{run}");
    }
    assert!(api.requests().is_empty(), "{:?}", api.operations());
}

#[test]
fn update_reports_an_unsuccessful_mutation() {
    let api = MockLinear::start();
    expect_own_team(&api);
    let mut rejected = updated("ENG-1", "x");
    rejected["issueUpdate"]["success"] = json!(false);
    api.on("UpdateIssue", rejected);
    Cli::for_api(&api)
        .run(&["issue", "update", "ENG-1", "-t", "x"])
        .failure();
}

fn archive_details(archived_at: Value) -> Value {
    json!({ "issue": { "identifier": "ENG-1", "title": "Old work", "archivedAt": archived_at } })
}

#[test]
fn archive_by_url_with_confirm() {
    let api = MockLinear::start();
    api.on("GetIssueArchiveDetails", archive_details(Value::Null))
        .on(
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
        .stdout_has("ENG-1")
        .stdout_has("Old work");
    assert_eq!(
        api.variables("GetIssueArchiveDetails"),
        json!({ "id": "ENG-1" })
    );
    assert_eq!(api.variables("ArchiveIssue"), json!({ "id": "ENG-1" }));
}

#[test]
fn archive_of_an_archived_issue_is_a_no_op() {
    let api = MockLinear::start();
    api.on(
        "GetIssueArchiveDetails",
        archive_details(json!("2026-01-01T00:00:00.000Z")),
    );
    Cli::for_api(&api)
        .run(&["issue", "archive", "ENG-1", "--confirm"])
        .success()
        .stdout_has("already archived");
}

#[test]
fn archive_without_confirmation_or_terminal_fails_before_any_request() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&["issue", "archive", "ENG-1"])
        .failure()
        .stderr_has("--confirm");
    assert!(api.requests().is_empty());
}

#[test]
fn archive_bulk_archives_each_issue() {
    let api = MockLinear::start();
    for id in ["ENG-1", "ENG-2"] {
        api.on(
            "GetIssueDetailsForBulkArchive",
            json!({ "issue": { "identifier": id, "title": "t", "archivedAt": null } }),
        )
        .on(
            "BulkArchiveIssue",
            json!({ "issueArchive": { "success": true } }),
        );
    }
    Cli::for_api(&api)
        .run(&["issue", "archive", "--confirm", "--bulk", "ENG-1", "ENG-2"])
        .success();
    let mut archived: Vec<Value> = api
        .requests()
        .into_iter()
        .filter(|r| r.operation.as_deref() == Some("BulkArchiveIssue"))
        .map(|r| r.variables["id"].clone())
        .collect();
    archived.sort_by_key(|id| id.to_string());
    assert_eq!(archived, [json!("ENG-1"), json!("ENG-2")]);
}

#[test]
fn delete_with_confirm() {
    let api = MockLinear::start();
    api.on(
        "GetIssueDeleteDetails",
        json!({ "issue": { "identifier": "ENG-3", "title": "Mistake" } }),
    )
    .on(
        "DeleteIssue",
        json!({ "issueDelete": { "success": true, "entity": null } }),
    );
    Cli::for_api(&api)
        .run(&["issue", "delete", "eng-3", "--confirm"])
        .success()
        .stdout_has("ENG-3");
    assert_eq!(
        api.variables("GetIssueDeleteDetails"),
        json!({ "id": "ENG-3" })
    );
    assert_eq!(api.variables("DeleteIssue"), json!({ "id": "ENG-3" }));
}

#[test]
fn delete_without_confirmation_or_terminal_fails_before_any_request() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&["issue", "delete", "ENG-3"])
        .failure()
        .stderr_has("--confirm");
    assert!(api.requests().is_empty());
}

#[test]
fn delete_rejects_a_positional_issue_with_bulk() {
    Cli::new()
        .run(&["issue", "delete", "ENG-9", "--confirm", "--bulk", "ENG-6"])
        .usage_error()
        .stderr_has("--bulk");
}

#[test]
fn delete_reports_a_missing_issue() {
    let api = MockLinear::start();
    api.on_raw(
        "GetIssueDeleteDetails",
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
            "GetIssueDetailsForBulkDelete",
            json!({ "issue": { "identifier": id, "title": "t" } }),
        )
        .on(
            "BulkDeleteIssue",
            json!({ "issueDelete": { "success": true } }),
        );
    }
    Cli::for_api(&api)
        .file("cwd/ids.txt", "ENG-5\nENG-6\n")
        .run(&["issue", "delete", "--confirm", "--bulk-file", "ids.txt"])
        .success()
        .stdout_has("2");
    let mut deleted: Vec<Value> = api
        .requests()
        .into_iter()
        .filter(|r| r.operation.as_deref() == Some("BulkDeleteIssue"))
        .map(|r| r.variables["id"].clone())
        .collect();
    deleted.sort_by_key(|id| id.to_string());
    assert_eq!(deleted, [json!("ENG-5"), json!("ENG-6")]);
}

#[test]
fn archive_bulk_reports_unusable_references_and_archives_the_rest() {
    let api = MockLinear::start();
    api.on(
        "GetIssueDetailsForBulkArchive",
        json!({ "issue": { "identifier": "ENG-1", "title": "t", "archivedAt": null } }),
    )
    .on(
        "BulkArchiveIssue",
        json!({ "issueArchive": { "success": true } }),
    );
    let run = Cli::for_api(&api).run(&[
        "issue",
        "archive",
        "--confirm",
        "--bulk",
        "3",
        "https://linear.app/acme/settings/x",
        "eng-1",
    ]);
    run.failure()
        .stdout_has("no team is set")
        .stdout_has("is not an entity this command can use");
    let ids: Vec<Value> = api
        .requests()
        .into_iter()
        .map(|r| r.variables["id"].clone())
        .collect();
    assert_eq!(ids, [json!("ENG-1"), json!("ENG-1")]);
}
