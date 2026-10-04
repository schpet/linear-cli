//! The `milestone` command group.
use serde_json::{Value, json};

use crate::support::{Cli, MockLinear, assert_json, nodes};

const PROJECT_ID: &str = "3b9a5c7e-1d2f-4a6b-8c9d-0e1f2a3b4c5d";
const MILESTONE_ID: &str = "00000000-0000-4000-8000-000000000001";

fn project_ids(ids: &[&str]) -> Value {
    let nodes: Vec<Value> = ids.iter().map(|id| json!({ "id": id })).collect();
    json!({ "projects": { "nodes": nodes } })
}

fn project() -> Value {
    json!({ "id": PROJECT_ID, "name": "Mobile App" })
}

fn list_node(id: &str, name: &str) -> Value {
    json!({ "id": id, "name": name, "targetDate": "2026-10-31", "sortOrder": 1, "project": project() })
}

fn milestones(nodes: Vec<Value>) -> Value {
    json!({ "project": {
        "id": PROJECT_ID, "name": "Mobile App",
        "projectMilestones": {
            "nodes": nodes,
            "pageInfo": { "hasNextPage": false, "endCursor": null }
        }
    } })
}

#[test]
fn list_json_by_project_id() {
    let api = MockLinear::start();
    let launch = list_node("m-1", "Launch");
    api.on("GetProjectMilestones", milestones(vec![launch.clone()]));
    let listed = Cli::for_api(&api)
        .run(&["milestone", "list", "--project", PROJECT_ID, "--json"])
        .success()
        .json_nodes();
    assert_eq!(listed, [launch]);
    assert_eq!(
        api.variables("GetProjectMilestones"),
        json!({ "projectId": PROJECT_ID, "first": 100 })
    );
}

#[test]
fn list_resolves_a_project_name() {
    let api = MockLinear::start();
    api.on("GetProjectIdByName", project_ids(&[PROJECT_ID])).on(
        "GetProjectMilestones",
        milestones(vec![list_node("m-1", "Launch")]),
    );
    Cli::for_api(&api)
        .run(&["milestone", "list", "--project", "Mobile App"])
        .success()
        .stdout_has("Launch");
    assert_eq!(
        api.variables("GetProjectIdByName"),
        json!({ "name": "Mobile App" })
    );
    assert_eq!(
        api.variables("GetProjectMilestones")["projectId"],
        PROJECT_ID
    );
}

#[test]
fn list_follows_pages_and_puts_undated_milestones_last() {
    let api = MockLinear::start();
    let undated = json!({ "id": "m-3", "name": "Someday", "targetDate": null, "sortOrder": 3, "project": project() });
    let late = json!({ "id": "m-2", "name": "Late", "targetDate": "2026-12-01", "sortOrder": 2.5, "project": project() });
    let early = json!({ "id": "m-1", "name": "Early", "targetDate": "2026-01-01", "sortOrder": 1, "project": project() });
    api.on(
        "GetProjectMilestones",
        json!({ "project": { "id": PROJECT_ID, "name": "Mobile App", "projectMilestones": {
            "nodes": [undated.clone(), late.clone()],
            "pageInfo": { "hasNextPage": true, "endCursor": "cursor-1" }
        } } }),
    )
    .on("GetProjectMilestones", milestones(vec![early.clone()]));
    let listed = Cli::for_api(&api)
        .run(&["milestone", "list", "--project", PROJECT_ID, "--json"])
        .success()
        .json_nodes();
    assert_eq!(listed, [early, late, undated]);
    let variables: Vec<Value> = api.requests().into_iter().map(|r| r.variables).collect();
    assert_eq!(
        variables,
        [
            json!({ "projectId": PROJECT_ID, "first": 100 }),
            json!({ "projectId": PROJECT_ID, "first": 100, "after": "cursor-1" })
        ]
    );
}

#[test]
fn list_of_a_missing_project_is_not_found() {
    let api = MockLinear::start();
    api.on("GetProjectMilestones", json!({ "project": null }));
    Cli::for_api(&api)
        .run(&["milestone", "list", "--project", PROJECT_ID])
        .failure()
        .stderr_has("Failed to list milestones: Project not found");
}

#[test]
fn list_requires_a_project() {
    Cli::new()
        .env("LINEAR_API_KEY", "key")
        .run(&["milestone", "list"])
        .usage_error();
}

fn issue(n: u32) -> Value {
    json!({
        "id": format!("issue-{n}"), "identifier": format!("APP-{n}"), "title": format!("Issue {n}"),
        "state": { "name": "Started", "type": "started" }
    })
}

fn details(issues: Vec<Value>, end_cursor: Value, has_next: bool) -> Value {
    json!({ "projectMilestone": {
        "id": MILESTONE_ID, "name": "Launch", "description": "Ship the release.",
        "targetDate": "2025-03-01", "sortOrder": 4,
        "createdAt": "2020-01-01T00:00:00.000Z", "updatedAt": "2020-02-01T00:00:00.000Z",
        "project": {
            "id": PROJECT_ID, "name": "Mobile App", "slugId": "abc123def456",
            "url": "https://linear.app/acme/project/mobile-app-abc123def456"
        },
        "issues": {
            "nodes": issues,
            "pageInfo": { "hasNextPage": has_next, "endCursor": end_cursor }
        }
    } })
}

#[test]
fn view_json_returns_the_milestone() {
    let api = MockLinear::start();
    let reply = details(vec![issue(1)], Value::Null, false);
    api.on("GetMilestoneDetails", reply.clone());
    let json = Cli::for_api(&api)
        .run(&["milestone", "view", MILESTONE_ID, "--json"])
        .success()
        .json();
    assert_json(&json, &reply["projectMilestone"]);
    assert_eq!(
        api.variables("GetMilestoneDetails"),
        json!({ "id": MILESTONE_ID, "first": 100 })
    );
}

#[test]
fn view_text_shows_name_and_issues() {
    let api = MockLinear::start();
    api.on(
        "GetMilestoneDetails",
        details(vec![issue(1)], Value::Null, false),
    );
    Cli::for_api(&api)
        .run(&["milestone", "view", MILESTONE_ID])
        .success()
        .stdout_has("Launch")
        .stdout_has("Mobile App")
        .stdout_has("APP-1");
}

#[test]
fn view_text_previews_ten_issues_unless_all() {
    let api = MockLinear::start();
    let reply = details((1..=11).map(issue).collect(), Value::Null, false);
    api.on("GetMilestoneDetails", reply.clone())
        .on("GetMilestoneDetails", reply);
    let preview = Cli::for_api(&api).run(&["milestone", "view", MILESTONE_ID]);
    preview
        .success()
        .stdout_has("Total Issues:** 11")
        .stdout_has("APP-10")
        .stdout_has("...and 1 more issue. Re-run with `--all` to list them.");
    assert!(!preview.stdout.contains("APP-11"), "{preview}");
    Cli::for_api(&api)
        .run(&["milestone", "view", MILESTONE_ID, "--all"])
        .success()
        .stdout_has("APP-11");
}

#[test]
fn view_json_follows_every_issue_page() {
    let api = MockLinear::start();
    api.on(
        "GetMilestoneDetails",
        details(vec![issue(1)], json!("cursor-1"), true),
    )
    .on(
        "GetMilestoneDetails",
        details(vec![issue(2)], Value::Null, false),
    );
    let json = Cli::for_api(&api)
        .run(&["milestone", "view", MILESTONE_ID, "--json"])
        .success()
        .json();
    assert_eq!(nodes(&json["issues"]), [issue(1), issue(2)]);
    let variables: Vec<Value> = api.requests().into_iter().map(|r| r.variables).collect();
    assert_eq!(
        variables,
        [
            json!({ "id": MILESTONE_ID, "first": 100 }),
            json!({ "id": MILESTONE_ID, "first": 100, "after": "cursor-1" })
        ]
    );
}

#[test]
fn view_by_name_resolves_within_the_project() {
    let api = MockLinear::start();
    api.on(
        "GetProjectMilestonesForLookup",
        json!({ "project": { "projectMilestones": { "nodes": [
            { "id": MILESTONE_ID, "name": "Launch" }
        ] } } }),
    )
    .on("GetMilestoneDetails", details(vec![], Value::Null, false));
    Cli::for_api(&api)
        .run(&[
            "milestone",
            "view",
            "launch",
            "--project",
            PROJECT_ID,
            "--json",
        ])
        .success();
    // Linear filters by name, so milestones past the first page are found.
    assert_eq!(
        api.variables("GetProjectMilestonesForLookup"),
        json!({ "projectId": PROJECT_ID, "name": "launch" })
    );
    assert!(
        api.request("GetProjectMilestonesForLookup")
            .query
            .contains("eqIgnoreCase: $name")
    );
    assert_eq!(api.variables("GetMilestoneDetails")["id"], MILESTONE_ID);
}

#[test]
fn view_by_name_needs_a_project() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&["milestone", "view", "Launch"])
        .failure()
        .stderr_has("--project");
    assert!(api.requests().is_empty());
}

fn created(target_date: Value) -> Value {
    json!({ "projectMilestoneCreate": { "success": true, "projectMilestone": {
        "id": "m-new", "name": "Launch", "targetDate": target_date, "project": project()
    } } })
}

#[test]
fn create_sends_all_fields() {
    let api = MockLinear::start();
    api.on("CreateProjectMilestone", created(json!("2026-10-31")));
    Cli::for_api(&api)
        .run(&[
            "milestone",
            "create",
            "--project",
            PROJECT_ID,
            "--name",
            "Launch",
            "--description",
            "Ship **it**",
            "--target-date",
            "2026-10-31",
        ])
        .success()
        .stdout_has("Launch");
    assert_eq!(
        api.variables("CreateProjectMilestone"),
        json!({ "input": {
            "projectId": PROJECT_ID, "name": "Launch",
            "description": "Ship **it**", "targetDate": "2026-10-31"
        } })
    );
}

#[test]
fn create_resolves_a_project_url_by_slug() {
    let api = MockLinear::start();
    api.on("GetProjectIdBySlugId", project_ids(&[PROJECT_ID]))
        .on("CreateProjectMilestone", created(Value::Null));
    Cli::for_api(&api)
        .run(&[
            "milestone",
            "create",
            "--project",
            "https://linear.app/acme/project/mobile-app-abc123def456",
            "--name",
            "Beta",
        ])
        .success();
    assert_eq!(
        api.variables("GetProjectIdBySlugId"),
        json!({ "slugId": "abc123def456" })
    );
    assert_eq!(
        api.variables("CreateProjectMilestone"),
        json!({ "input": { "projectId": PROJECT_ID, "name": "Beta" } })
    );
}

#[test]
fn create_warns_that_an_unreadable_reply_may_have_created_it() {
    let api = MockLinear::start();
    api.on_raw("CreateProjectMilestone", 200, "not json");
    Cli::for_api(&api)
        .run(&[
            "milestone",
            "create",
            "--project",
            PROJECT_ID,
            "--name",
            "Beta",
        ])
        .failure()
        .stderr_has("milestone may already exist");
    assert_eq!(api.operations(), ["CreateProjectMilestone"]);
}

#[test]
fn create_requires_a_name() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&["milestone", "create", "--project", PROJECT_ID])
        .usage_error();
}

#[test]
fn create_with_unknown_project_fails_without_creating() {
    let api = MockLinear::start();
    api.on("GetProjectIdByName", project_ids(&[]))
        .on("GetProjectIdBySlugId", project_ids(&[]));
    Cli::for_api(&api)
        .run(&["milestone", "create", "--project", "Nope", "--name", "Beta"])
        .failure();
}

#[test]
fn update_sends_the_changed_fields() {
    let api = MockLinear::start();
    api.on(
        "UpdateProjectMilestone",
        json!({ "projectMilestoneUpdate": { "success": true, "projectMilestone": {
            "id": MILESTONE_ID, "name": "Launch", "targetDate": "2026-12-01", "sortOrder": 2,
            "project": project()
        } } }),
    );
    Cli::for_api(&api)
        .run(&[
            "milestone",
            "update",
            MILESTONE_ID,
            "--name",
            "Launch",
            "--description",
            "Ship",
            "--target-date",
            "2026-12-01",
            "--sort-order",
            "2",
            "--project",
            PROJECT_ID,
        ])
        .success()
        .stdout_has("Launch");
    assert_eq!(
        api.variables("UpdateProjectMilestone"),
        json!({ "id": MILESTONE_ID, "input": {
            "name": "Launch", "description": "Ship", "targetDate": "2026-12-01",
            "sortOrder": 2, "projectId": PROJECT_ID
        } })
    );
    // A whole sort order is sent as an integer, not 2.0.
    let body = api.request("UpdateProjectMilestone").body;
    assert!(String::from_utf8_lossy(&body).contains(r#""sortOrder":2,"#));
}

#[test]
fn update_sends_a_fractional_sort_order() {
    let api = MockLinear::start();
    api.on(
        "UpdateProjectMilestone",
        json!({ "projectMilestoneUpdate": { "success": true, "projectMilestone": {
            "id": MILESTONE_ID, "name": "Launch", "targetDate": null, "sortOrder": -1.5,
            "project": project()
        } } }),
    );
    Cli::for_api(&api)
        .run(&["milestone", "update", MILESTONE_ID, "--sort-order", "-1.5"])
        .success()
        .stdout_has("Sort Order: -1.5");
    assert_eq!(
        api.variables("UpdateProjectMilestone"),
        json!({ "id": MILESTONE_ID, "input": { "sortOrder": -1.5 } })
    );
}

#[test]
fn update_without_changes_is_a_usage_error() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&["milestone", "update", MILESTONE_ID])
        .usage_error();
    assert!(api.requests().is_empty());
}

#[test]
fn update_and_create_reject_malformed_dates_before_any_request() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&[
            "milestone",
            "update",
            MILESTONE_ID,
            "--target-date",
            "tomorrow",
        ])
        .usage_error();
    Cli::for_api(&api)
        .run(&[
            "milestone",
            "create",
            "--project",
            PROJECT_ID,
            "--name",
            "Beta",
            "--target-date",
            "2026-02-30",
        ])
        .usage_error();
    assert!(api.requests().is_empty());
}

#[test]
fn update_rejects_a_non_numeric_sort_order_before_any_request() {
    let api = MockLinear::start();
    let run = Cli::for_api(&api).run(&["milestone", "update", MILESTONE_ID, "--sort-order", "x"]);
    run.usage_error();
}

#[test]
fn delete_with_yes_deletes() {
    let api = MockLinear::start();
    api.on("GetMilestoneName", milestone_name()).on(
        "DeleteProjectMilestone",
        json!({ "projectMilestoneDelete": { "success": true } }),
    );
    Cli::for_api(&api)
        .run(&["milestone", "delete", MILESTONE_ID, "--yes"])
        .success()
        .stdout_has("✓ Deleted milestone Beta");
    assert_eq!(
        api.variables("DeleteProjectMilestone"),
        json!({ "id": MILESTONE_ID })
    );
}

#[test]
fn delete_without_yes_needs_a_confirmation() {
    let api = MockLinear::start();
    api.on("GetMilestoneName", milestone_name());
    Cli::for_api(&api)
        .run(&["milestone", "delete", MILESTONE_ID])
        .failure()
        .stderr_has("--yes");
    assert_eq!(api.operations(), ["GetMilestoneName"]);
}

fn milestone_name() -> Value {
    json!({ "projectMilestone": {
        "name": "Beta", "project": { "id": "project-1", "name": "Mobile" }
    } })
}

#[test]
fn delete_names_the_milestone_and_its_project_on_a_terminal() {
    let api = MockLinear::start();
    api.on("GetMilestoneName", milestone_name());
    Cli::for_api(&api)
        .run_tty(
            &["milestone", "delete", MILESTONE_ID],
            &[(
                "delete milestone \"Beta\" of project \"Mobile\"? (y/N)",
                "\r",
            )],
        )
        .success()
        .stdout_has("Canceled.");
    assert_eq!(api.operations(), ["GetMilestoneName"]);
}

#[test]
fn delete_reports_an_unknown_milestone_without_asking() {
    let api = MockLinear::start();
    api.on("GetMilestoneName", json!({ "projectMilestone": null }));
    Cli::for_api(&api)
        .run(&["milestone", "delete", MILESTONE_ID])
        .failure()
        .stderr_has(&format!("Milestone not found: {MILESTONE_ID}"));
}
