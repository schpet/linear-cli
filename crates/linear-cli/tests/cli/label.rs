//! The `label` command group.
use serde_json::{Value, json};

use crate::support::{Cli, MockLinear};
use crate::team::{resolve_vars, resolved};

const ENG_ID: &str = "team-eng-id";
const LABEL_UUID: &str = "abcdefab-1234-4678-90ab-abcdefabcdef";

fn label(id: &str, name: &str, team: Value) -> Value {
    json!({ "id": id, "name": name, "description": null, "color": "#ff0000", "team": team })
}

fn eng() -> Value {
    json!({ "key": "ENG", "name": "Engineering" })
}

fn labels(nodes: Vec<Value>, end_cursor: Value, has_next: bool) -> Value {
    json!({ "issueLabels": {
        "nodes": nodes,
        "pageInfo": { "hasNextPage": has_next, "endCursor": end_cursor }
    } })
}

fn team_or_workspace_filter() -> Value {
    json!({ "or": [{ "team": { "key": { "eq": "ENG" } } }, { "team": { "null": true } }] })
}

#[test]
fn list_json_follows_pages() {
    let api = MockLinear::start();
    let bug = label("l-bug", "Bug", eng());
    let shared = label("l-shared", "Shared", Value::Null);
    api.on(
        "GetIssueLabels",
        labels(vec![bug.clone()], json!("cursor-1"), true),
    )
    .on(
        "GetIssueLabels",
        labels(vec![shared.clone()], Value::Null, false),
    );
    let listed = Cli::for_api(&api)
        .run(&["label", "list", "--json"])
        .success()
        .json_nodes();
    let mut names: Vec<&str> = listed
        .iter()
        .map(|node| node["name"].as_str().expect("label name"))
        .collect();
    names.sort_unstable();
    assert_eq!(names, ["Bug", "Shared"]);
    let variables: Vec<Value> = api.requests().into_iter().map(|r| r.variables).collect();
    assert_eq!(
        variables,
        [
            json!({ "first": 100 }),
            json!({ "first": 100, "after": "cursor-1" })
        ]
    );
}

#[test]
fn list_text_shows_names_and_teams() {
    let api = MockLinear::start();
    api.on(
        "GetIssueLabels",
        labels(vec![label("l-bug", "Bug", eng())], Value::Null, false),
    );
    Cli::for_api(&api)
        .run(&["label", "list"])
        .success()
        .stdout_has("Bug")
        .stdout_has("ENG");
}

#[test]
fn list_scopes_to_the_configured_team_plus_workspace_labels() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
        .on("GetIssueLabels", labels(vec![], Value::Null, false));
    Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "eng")
        .run(&["label", "list", "--json"])
        .success();
    assert_eq!(api.variables("ResolveTeam"), resolve_vars("ENG"));
    assert_eq!(
        api.variables("GetIssueLabels"),
        json!({ "first": 100, "filter": team_or_workspace_filter() })
    );
}

#[test]
fn list_scope_flags_conflict() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api);
    cli.run(&["label", "list", "--workspace-only", "--all-teams"])
        .usage_error();
    cli.run(&["label", "list", "--all-teams", "--team", "ENG"])
        .usage_error();
}

#[test]
fn list_all_ignores_the_configured_team() {
    let api = MockLinear::start();
    api.on("GetIssueLabels", labels(vec![], Value::Null, false));
    Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .run(&["label", "list", "--all-teams", "--json"])
        .success();
    assert_eq!(api.variables("GetIssueLabels"), json!({ "first": 100 }));
}

#[test]
fn list_team_flag_resolves_the_team() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
        .on("GetIssueLabels", labels(vec![], Value::Null, false));
    Cli::for_api(&api)
        .run(&["label", "list", "--team", "Engineering", "--json"])
        .success();
    assert_eq!(api.variables("ResolveTeam"), resolve_vars("Engineering"));
    assert_eq!(
        api.variables("GetIssueLabels"),
        json!({ "first": 100, "filter": team_or_workspace_filter() })
    );
}

#[test]
fn list_workspace_only_filters_to_labels_without_a_team() {
    let api = MockLinear::start();
    api.on("GetIssueLabels", labels(vec![], Value::Null, false));
    Cli::for_api(&api)
        .run(&["label", "list", "--workspace-only", "--json"])
        .success();
    assert_eq!(
        api.variables("GetIssueLabels"),
        json!({ "first": 100, "filter": { "team": { "null": true } } })
    );
}

fn created(name: &str) -> Value {
    json!({ "issueLabelCreate": { "success": true, "issueLabel": {
        "id": "l-new", "name": name, "color": "#abcdef", "description": "Crashes", "team": eng()
    } } })
}

#[test]
fn create_team_label_sends_all_fields() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
        .on("CreateIssueLabel", created("Bug"));
    Cli::for_api(&api)
        .run(&[
            "label", "create", "-n", "Bug", "-c", "#ABCDEF", "-d", "Crashes", "-t", "eng",
        ])
        .success()
        .stdout_has("Bug");
    assert_eq!(api.variables("ResolveTeam"), resolve_vars("eng"));
    assert_eq!(
        api.variables("CreateIssueLabel"),
        json!({ "input": {
            "name": "Bug", "color": "#ABCDEF", "description": "Crashes", "teamId": ENG_ID
        } })
    );
}

#[test]
fn create_workspace_label_defaults_the_color() {
    let api = MockLinear::start();
    api.on("CreateIssueLabel", created("Shared"));
    Cli::for_api(&api)
        .run(&["label", "create", "-n", "Shared"])
        .success();
    let input = &api.variables("CreateIssueLabel")["input"];
    assert_eq!(input["name"], "Shared");
    assert!(input["color"].is_string());
    assert!(input.get("teamId").is_none());
}

#[test]
fn create_reports_unsuccessful_mutations() {
    let api = MockLinear::start();
    api.on(
        "CreateIssueLabel",
        json!({ "issueLabelCreate": { "success": false, "issueLabel": null } }),
    );
    Cli::for_api(&api)
        .run(&["label", "create", "-n", "Bug"])
        .failure();
}

fn by_name(nodes: Vec<Value>) -> Value {
    json!({ "issueLabels": { "nodes": nodes } })
}

fn deleted() -> Value {
    json!({ "issueLabelDelete": { "success": true } })
}

#[test]
fn delete_by_name_deletes_the_matching_label() {
    let api = MockLinear::start();
    api.on(
        "GetLabelByName",
        by_name(vec![label("l-bug", "Bug", eng())]),
    )
    .on("DeleteIssueLabel", deleted());
    Cli::for_api(&api)
        .run(&["label", "delete", "Bug", "--yes"])
        .success()
        .stdout_has("Bug");
    assert_eq!(api.variables("GetLabelByName"), json!({ "name": "Bug" }));
    assert_eq!(api.variables("DeleteIssueLabel"), json!({ "id": "l-bug" }));
}

#[test]
fn delete_by_name_with_team_picks_that_teams_label() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
        .on(
            "GetLabelByName",
            by_name(vec![
                label("l-shared", "Bug", Value::Null),
                label("l-eng", "Bug", eng()),
            ]),
        )
        .on("DeleteIssueLabel", deleted());
    Cli::for_api(&api)
        .run(&["label", "delete", "Bug", "--team", "ENG", "--yes"])
        .success();
    assert_eq!(api.variables("DeleteIssueLabel"), json!({ "id": "l-eng" }));
}

#[test]
fn delete_by_id_looks_the_label_up_directly() {
    let api = MockLinear::start();
    api.on(
        "GetLabelById",
        json!({ "issueLabel": label(LABEL_UUID, "Bug", eng()) }),
    )
    .on("DeleteIssueLabel", deleted());
    Cli::for_api(&api)
        .run(&["label", "delete", LABEL_UUID, "--yes"])
        .success();
    assert_eq!(api.variables("GetLabelById"), json!({ "id": LABEL_UUID }));
    assert_eq!(
        api.variables("DeleteIssueLabel"),
        json!({ "id": LABEL_UUID })
    );
}

#[test]
fn delete_ambiguous_name_fails_without_deleting() {
    let api = MockLinear::start();
    api.on(
        "GetLabelByName",
        by_name(vec![
            label("l-1", "Bug", eng()),
            label("l-2", "Bug", json!({ "key": "OPS", "name": "Ops" })),
        ]),
    );
    Cli::for_api(&api)
        .run(&["label", "delete", "Bug", "--yes"])
        .failure();
}

#[test]
fn delete_without_yes_refuses_before_any_request() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&["label", "delete", "Bug"])
        .failure()
        .stderr_has("--yes");
}

#[test]
fn delete_by_name_uses_the_configured_team() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
        .on(
            "GetLabelByName",
            by_name(vec![
                label("l-ops", "Bug", json!({ "key": "OPS", "name": "Ops" })),
                label("l-eng", "Bug", eng()),
            ]),
        )
        .on("DeleteIssueLabel", deleted());
    Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "eng")
        .run(&["label", "delete", "Bug", "--yes"])
        .success()
        .stdout_has("Bug (ENG)");
    assert_eq!(api.variables("DeleteIssueLabel"), json!({ "id": "l-eng" }));
}

#[test]
fn delete_falls_back_to_the_workspace_label() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
        .on(
            "GetLabelByName",
            by_name(vec![
                label("l-ops", "Bug", json!({ "key": "OPS", "name": "Ops" })),
                label("l-shared", "Bug", Value::Null),
            ]),
        )
        .on("DeleteIssueLabel", deleted());
    Cli::for_api(&api)
        .run(&["label", "delete", "Bug", "--team", "ENG", "--yes"])
        .success();
    assert_eq!(
        api.variables("DeleteIssueLabel"),
        json!({ "id": "l-shared" })
    );
}

#[test]
fn delete_missing_label_is_not_found() {
    let api = MockLinear::start();
    api.on("GetLabelByName", by_name(vec![]));
    Cli::for_api(&api)
        .run(&["label", "delete", "Nope", "--yes"])
        .failure()
        .stderr_has("Label not found: Nope");
}

#[test]
fn create_without_a_name_needs_a_terminal() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&["label", "create", "-c", "#ABCDEF"])
        .usage_error()
        .stderr_has("--name");
}

#[test]
fn create_rejects_a_bad_color_before_any_request() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&["label", "create", "-n", "Bug", "-c", "red"])
        .usage_error();
}
