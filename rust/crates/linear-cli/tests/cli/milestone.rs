//! The `milestone` command group.
use serde_json::{Value, json};

use crate::support::{Cli, MockLinear};

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
    let json = Cli::for_api(&api)
        .run(&["milestone", "list", "--project", PROJECT_ID, "--json"])
        .success()
        .json();
    assert_eq!(json["nodes"], json!([launch]));
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
        "createdAt": "2020-01-01T00:00:00Z", "updatedAt": "2020-02-01T00:00:00Z",
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
    assert_eq!(json, reply["projectMilestone"]);
    assert_eq!(
        api.variables("GetMilestoneDetails"),
        json!({ "id": MILESTONE_ID, "first": 50 })
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
fn view_all_follows_issue_pages() {
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
        .run(&["milestone", "view", MILESTONE_ID, "--all", "--json"])
        .success()
        .json();
    assert_eq!(json["issues"]["nodes"], json!([issue(1), issue(2)]));
    let variables: Vec<Value> = api.requests().into_iter().map(|r| r.variables).collect();
    assert_eq!(
        variables,
        [
            json!({ "id": MILESTONE_ID, "first": 50 }),
            json!({ "id": MILESTONE_ID, "first": 50, "after": "cursor-1" })
        ]
    );
}

#[test]
fn view_by_name_resolves_within_the_project() {
    let api = MockLinear::start();
    api.on(
        "GetProjectMilestonesForLookup",
        json!({ "project": { "projectMilestones": { "nodes": [
            { "id": MILESTONE_ID, "name": "Launch" },
            { "id": "m-other", "name": "Beta" }
        ] } } }),
    )
    .on("GetMilestoneDetails", details(vec![], Value::Null, false));
    Cli::for_api(&api)
        .run(&[
            "milestone",
            "view",
            "Launch",
            "--project",
            PROJECT_ID,
            "--json",
        ])
        .success();
    assert_eq!(api.variables("GetMilestoneDetails")["id"], MILESTONE_ID);
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
    let mut variables = api.variables("UpdateProjectMilestone");
    // Only the numeric value matters, not whether it is spelled 2 or 2.0.
    let sort_order = variables["input"]
        .as_object_mut()
        .expect("input object")
        .remove("sortOrder");
    assert_eq!(sort_order.as_ref().and_then(Value::as_f64), Some(2.0));
    assert_eq!(
        variables,
        json!({ "id": MILESTONE_ID, "input": {
            "name": "Launch", "description": "Ship", "targetDate": "2026-12-01",
            "projectId": PROJECT_ID
        } })
    );
}

#[test]
fn update_rejects_a_non_numeric_sort_order_before_any_request() {
    let api = MockLinear::start();
    let run = Cli::for_api(&api).run(&["milestone", "update", MILESTONE_ID, "--sort-order", "x"]);
    assert_ne!(run.code, 0, "{run}");
}

#[test]
fn delete_with_force_deletes() {
    let api = MockLinear::start();
    api.on(
        "DeleteProjectMilestone",
        json!({ "projectMilestoneDelete": { "success": true } }),
    );
    Cli::for_api(&api)
        .run(&["milestone", "delete", MILESTONE_ID, "--force"])
        .success()
        .stdout_has(MILESTONE_ID);
    assert_eq!(
        api.variables("DeleteProjectMilestone"),
        json!({ "id": MILESTONE_ID })
    );
}

#[test]
fn delete_without_force_needs_a_confirmation() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&["milestone", "delete", MILESTONE_ID])
        .failure();
}
