//! The `team` command group (`team id` is covered in config.rs).
use serde_json::{Value, json};

use crate::support::{Cli, MockLinear};

const ENG_ID: &str = "team-eng-id";

/// `ResolveTeam` reply for a key/name reference.
pub fn resolved(id: &str, key: &str, name: &str) -> Value {
    json!({ "teams": { "nodes": [{ "id": id, "key": key, "name": name }] } })
}

pub fn resolve_vars(reference: &str) -> Value {
    json!({ "reference": reference, "id": null, "isUuid": false })
}

fn team(id: &str, key: &str, name: &str, archived_at: Value) -> Value {
    json!({
        "id": id, "name": name, "key": key, "description": null, "icon": null,
        "color": "#4466aa", "cyclesEnabled": true,
        "createdAt": "2025-01-01T00:00:00.000Z", "updatedAt": "2025-01-02T00:00:00.000Z",
        "archivedAt": archived_at,
        "organization": { "id": "org-1", "name": "Acme" }
    })
}

fn page(nodes: Vec<Value>, end_cursor: Value, has_next: bool) -> Value {
    json!({ "nodes": nodes, "pageInfo": { "hasNextPage": has_next, "endCursor": end_cursor } })
}

#[test]
fn list_json_follows_pages_and_skips_archived_teams() {
    let api = MockLinear::start();
    let zulu = team("t-z", "Z", "Zulu", Value::Null);
    let alpha = team("t-a", "A", "Alpha", Value::Null);
    let archived = team(
        "t-old",
        "OLD",
        "Archived",
        json!("2025-02-01T00:00:00.000Z"),
    );
    api.on(
        "GetTeams",
        json!({ "teams": page(vec![zulu.clone()], json!("cursor-1"), true) }),
    )
    .on(
        "GetTeams",
        json!({ "teams": page(vec![alpha.clone(), archived], json!("cursor-2"), false) }),
    );
    let json = Cli::for_api(&api)
        .run(&["team", "list", "--json"])
        .success()
        .json();
    assert_eq!(json["nodes"], json!([alpha, zulu]));
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
fn list_text_shows_keys_and_names() {
    let api = MockLinear::start();
    api.on(
        "GetTeams",
        json!({ "teams": page(vec![team("t-e", "ENG", "Engineering", Value::Null)], Value::Null, false) }),
    );
    Cli::for_api(&api)
        .run(&["team", "list"])
        .success()
        .stdout_has("ENG")
        .stdout_has("Engineering");
}

#[test]
fn list_reports_graphql_errors() {
    let api = MockLinear::start();
    api.on_error("GetTeams", "Rate limited");
    Cli::for_api(&api)
        .run(&["team", "list"])
        .failure()
        .stderr_has("Rate limited");
}

fn member(id: &str, name: &str, active: bool) -> Value {
    json!({
        "id": id, "name": name, "displayName": id, "email": format!("{id}@example.com"),
        "active": active, "initials": "XX", "description": null, "timezone": null,
        "lastSeen": null, "statusEmoji": null, "statusLabel": null, "guest": false,
        "isAssignable": true, "admin": false, "owner": false, "isMe": false,
        "url": format!("https://linear.app/acme/profiles/{id}")
    })
}

#[test]
fn members_json_resolves_the_team_and_includes_inactive_with_all() {
    let api = MockLinear::start();
    let ada = member("ada", "Ada Lovelace", false);
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
        .on(
            "GetTeamMembers",
            json!({ "team": { "members": page(vec![ada.clone()], Value::Null, false) } }),
        );
    let json = Cli::for_api(&api)
        .run(&["team", "members", "eng", "--all", "--json"])
        .success()
        .json();
    assert_eq!(json["nodes"], json!([ada]));
    assert_eq!(api.variables("ResolveTeam"), resolve_vars("eng"));
    assert_eq!(
        api.variables("GetTeamMembers"),
        json!({ "teamKey": "ENG", "includeDisabled": true, "first": 100 })
    );
}

#[test]
fn members_default_to_the_configured_team_and_active_members() {
    let api = MockLinear::start();
    api.on(
        "GetTeamMembers",
        json!({ "team": { "members": page(vec![member("bob", "Bob Builder", true)], Value::Null, false) } }),
    );
    Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .run(&["team", "members"])
        .success()
        .stdout_has("Bob Builder");
    assert_eq!(
        api.variables("GetTeamMembers"),
        json!({ "teamKey": "ENG", "includeDisabled": false, "first": 100 })
    );
}

fn states() -> Value {
    json!({ "team": { "states": { "nodes": [
        { "id": "done", "name": "Done", "type": "completed", "position": 3 },
        { "id": "todo", "name": "Todo", "type": "unstarted", "position": 1 },
        { "id": "backlog", "name": "Backlog", "type": "backlog", "position": 0 },
        { "id": "progress", "name": "In Progress", "type": "started", "position": 2 }
    ] } } })
}

#[test]
fn states_json_lists_the_teams_states() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved(ENG_ID, "ENG", "Engineering"))
        .on("GetWorkflowStates", states());
    let json = Cli::for_api(&api)
        .run(&["team", "states", "eng", "--json"])
        .success()
        .json();
    let mut names: Vec<&str> = json["nodes"]
        .as_array()
        .expect("nodes array")
        .iter()
        .map(|state| state["name"].as_str().expect("state name"))
        .collect();
    names.sort_unstable();
    assert_eq!(names, ["Backlog", "Done", "In Progress", "Todo"]);
    assert_eq!(api.variables("ResolveTeam"), resolve_vars("eng"));
    assert_eq!(
        api.variables("GetWorkflowStates"),
        json!({ "teamKey": "ENG" })
    );
}

#[test]
fn states_text_uses_the_configured_team() {
    let api = MockLinear::start();
    api.on("GetWorkflowStates", states());
    Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .run(&["team", "states"])
        .success()
        .stdout_has("In Progress")
        .stdout_has("completed");
    assert_eq!(
        api.variables("GetWorkflowStates"),
        json!({ "teamKey": "ENG" })
    );
}

#[test]
fn create_sends_the_given_fields() {
    let api = MockLinear::start();
    api.on(
        "CreateTeam",
        json!({ "teamCreate": { "success": true, "team": { "id": "t-new", "name": "Platform", "key": "PLT" } } }),
    );
    Cli::for_api(&api)
        .run(&[
            "team",
            "create",
            "-n",
            "Platform",
            "-d",
            "Owns builds",
            "-k",
            "plt",
            "--private",
        ])
        .success()
        .stdout_has("PLT");
    assert_eq!(
        api.variables("CreateTeam"),
        json!({ "input": { "name": "Platform", "description": "Owns builds", "key": "plt", "private": true } })
    );
}

#[test]
fn create_with_only_a_name_omits_optional_fields() {
    let api = MockLinear::start();
    api.on(
        "CreateTeam",
        json!({ "teamCreate": { "success": true, "team": { "id": "t-new", "name": "Public", "key": "PUB" } } }),
    );
    Cli::for_api(&api)
        .run(&["team", "create", "--name", "Public"])
        .success()
        .stdout_has("PUB");
    assert_eq!(
        api.variables("CreateTeam"),
        json!({ "input": { "name": "Public" } })
    );
}

#[test]
fn create_without_a_name_fails_before_any_request() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&["team", "create", "--no-interactive"])
        .failure()
        .stderr_has("name");
    assert!(api.requests().is_empty());
}

fn source_details(issue_ids: &[&str]) -> Value {
    let nodes: Vec<Value> = issue_ids.iter().map(|id| json!({ "id": id })).collect();
    json!({ "team": { "id": "t-src", "key": "SRC", "name": "Source", "issues": { "nodes": nodes } } })
}

#[test]
fn delete_with_force_deletes_the_resolved_team() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved("t-src", "SRC", "Source"))
        .on("GetTeamDetails", source_details(&[]))
        .on("DeleteTeam", json!({ "teamDelete": { "success": true } }));
    Cli::for_api(&api)
        .run(&[
            "team",
            "delete",
            "https://linear.app/acme/team/SRC/all",
            "-y",
        ])
        .success()
        .stdout_has("Source");
    assert_eq!(api.variables("ResolveTeam"), resolve_vars("SRC"));
    assert_eq!(api.variables("GetTeamDetails"), json!({ "id": "t-src" }));
    assert_eq!(api.variables("DeleteTeam"), json!({ "id": "t-src" }));
}

#[test]
fn delete_moves_every_issue_before_deleting() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved("t-src", "SRC", "Source"))
        .on("GetTeamDetails", source_details(&["i-1"]))
        .on("ResolveTeam", resolved("t-dest", "DEST", "Destination"))
        .on(
            "GetTeamIssuesForMove",
            json!({ "team": { "issues": page(
                vec![json!({ "id": "i-1", "identifier": "SRC-1" })], json!("more"), true
            ) } }),
        )
        .on(
            "GetTeamIssuesForMove",
            json!({ "team": { "issues": page(
                vec![json!({ "id": "i-2", "identifier": "SRC-2" })], Value::Null, false
            ) } }),
        )
        .on(
            "MoveIssueToTeam",
            json!({ "issueUpdate": { "success": true } }),
        )
        .on(
            "MoveIssueToTeam",
            json!({ "issueUpdate": { "success": true } }),
        )
        .on("DeleteTeam", json!({ "teamDelete": { "success": true } }));
    Cli::for_api(&api)
        .run(&["team", "delete", "SRC", "--force", "--move-issues", "DEST"])
        .success()
        .stdout_has("Source");
    let moves: Vec<Value> = api
        .requests()
        .into_iter()
        .filter(|request| request.operation.as_deref() == Some("MoveIssueToTeam"))
        .map(|request| request.variables)
        .collect();
    assert_eq!(
        moves,
        [
            json!({ "id": "i-1", "teamId": "t-dest" }),
            json!({ "id": "i-2", "teamId": "t-dest" }),
        ]
    );
    assert_eq!(api.variables("DeleteTeam"), json!({ "id": "t-src" }));
}

#[test]
fn delete_requires_force_without_a_terminal() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved("t-src", "SRC", "Source"))
        .on("GetTeamDetails", source_details(&[]));
    Cli::for_api(&api)
        .stdin(b"y\n")
        .run(&["team", "delete", "SRC"])
        .failure()
        .stderr_has("--force");
    assert!(!api.operations().contains(&"DeleteTeam".to_owned()));
}

#[test]
fn autolinks_registers_the_team_prefix_with_gh() {
    let cli = Cli::new()
        .env("LINEAR_TEAM_ID", "ENG")
        .env("LINEAR_WORKSPACE", "acme")
        .stub_bin("gh", "exit 0");
    cli.run(&["team", "autolinks"]).success();
    assert_eq!(
        cli.calls("gh"),
        [vec![
            "api",
            "repos/{owner}/{repo}/autolinks",
            "-f",
            "key_prefix=ENG-",
            "-f",
            "url_template=https://linear.app/acme/issue/ENG-<num>",
        ]]
    );
}

#[test]
fn autolinks_fails_when_gh_fails_or_config_is_missing() {
    let cli = Cli::new()
        .env("LINEAR_TEAM_ID", "ENG")
        .env("LINEAR_WORKSPACE", "acme")
        .stub_bin("gh", "exit 1");
    cli.run(&["team", "autolinks"]).failure();
    let cli = Cli::new()
        .env("LINEAR_WORKSPACE", "acme")
        .stub_bin("gh", "exit 0");
    cli.run(&["team", "autolinks"]).failure();
    assert!(cli.calls("gh").is_empty());
}
