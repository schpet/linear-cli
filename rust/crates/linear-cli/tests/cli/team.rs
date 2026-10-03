//! The `team` command group (`team id` is covered in config.rs).
use serde_json::{Value, json};

use crate::support::{Cli, MockLinear};
use crate::web::{open_stubs, opened};

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
    let listed = Cli::for_api(&api)
        .run(&["team", "list", "--json"])
        .success()
        .json_nodes();
    assert_eq!(listed, [alpha, zulu]);
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
fn list_stops_on_a_repeated_cursor() {
    let api = MockLinear::start();
    let alpha = team("t-a", "A", "Alpha", Value::Null);
    api.on(
        "GetTeams",
        json!({ "teams": page(vec![alpha.clone()], json!("cursor-1"), true) }),
    )
    .on(
        "GetTeams",
        json!({ "teams": page(vec![alpha], json!("cursor-1"), true) }),
    );
    Cli::for_api(&api)
        .run(&["team", "list", "--json"])
        .failure()
        .stderr_has("same pagination cursor");
    assert_eq!(api.requests().len(), 2);
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
    let listed = Cli::for_api(&api)
        .run(&["team", "members", "eng", "--all", "--json"])
        .success()
        .json_nodes();
    assert_eq!(listed, [ada]);
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

#[test]
fn members_without_a_team_fail_before_any_request() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&["team", "members"])
        .failure()
        .stderr_has("No team given");
    assert!(api.requests().is_empty());
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
    let listed = Cli::for_api(&api)
        .run(&["team", "states", "eng", "--json"])
        .success()
        .json_nodes();
    let mut names: Vec<&str> = listed
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
fn create_warns_that_an_unreadable_reply_may_have_created_it() {
    let api = MockLinear::start();
    api.on_raw("CreateTeam", 200, "not json");
    Cli::for_api(&api)
        .run(&["team", "create", "--name", "Platform"])
        .failure()
        .stderr_has("team may already exist");
    assert_eq!(api.operations(), ["CreateTeam"]);
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

fn issue_page(identifiers: &[&str], end_cursor: Value, has_next: bool) -> Value {
    let nodes: Vec<Value> = identifiers
        .iter()
        .map(|identifier| json!({ "id": format!("id-{identifier}"), "identifier": identifier }))
        .collect();
    json!({ "team": { "issues": page(nodes, end_cursor, has_next) } })
}

fn moved(success: bool) -> Value {
    json!({ "issueUpdate": { "success": success } })
}

fn move_variables(api: &MockLinear) -> Vec<Value> {
    api.requests()
        .into_iter()
        .filter(|request| request.operation.as_deref() == Some("MoveIssueToTeam"))
        .map(|request| request.variables)
        .collect()
}

#[test]
fn delete_with_force_deletes_the_resolved_team() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved("t-src", "SRC", "Source"))
        .on("GetTeamIssuesForMove", issue_page(&[], Value::Null, false))
        .on("DeleteTeam", json!({ "teamDelete": { "success": true } }));
    Cli::for_api(&api)
        .run(&[
            "team",
            "delete",
            "https://linear.app/acme/team/SRC/all",
            "-y",
        ])
        .success()
        .stdout_has("Deleted team SRC: Source");
    assert_eq!(api.variables("ResolveTeam"), resolve_vars("SRC"));
    assert_eq!(
        api.variables("GetTeamIssuesForMove"),
        json!({ "teamId": "t-src", "first": 100 })
    );
    assert_eq!(api.variables("DeleteTeam"), json!({ "id": "t-src" }));
}

#[test]
fn delete_moves_every_issue_before_deleting() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved("t-src", "SRC", "Source"))
        .on("ResolveTeam", resolved("t-dest", "DEST", "Destination"))
        .on(
            "GetTeamIssuesForMove",
            issue_page(&["SRC-1"], json!("more"), true),
        )
        .on(
            "GetTeamIssuesForMove",
            issue_page(&["SRC-2"], Value::Null, false),
        )
        .on("MoveIssueToTeam", moved(true))
        .on("MoveIssueToTeam", moved(true))
        .on("DeleteTeam", json!({ "teamDelete": { "success": true } }));
    Cli::for_api(&api)
        .run(&["team", "delete", "SRC", "--force", "--move-issues", "DEST"])
        .success()
        .stdout_has("Moved 2 issue(s) to DEST")
        .stdout_has("Deleted team SRC: Source");
    let pages: Vec<Value> = api
        .requests()
        .into_iter()
        .filter(|request| request.operation.as_deref() == Some("GetTeamIssuesForMove"))
        .map(|request| request.variables)
        .collect();
    assert_eq!(
        pages,
        [
            json!({ "teamId": "t-src", "first": 100 }),
            json!({ "teamId": "t-src", "first": 100, "after": "more" })
        ]
    );
    assert_eq!(
        move_variables(&api),
        [
            json!({ "id": "id-SRC-1", "teamId": "t-dest" }),
            json!({ "id": "id-SRC-2", "teamId": "t-dest" }),
        ]
    );
    assert_eq!(api.variables("DeleteTeam"), json!({ "id": "t-src" }));
}

#[test]
fn delete_keeps_the_team_when_some_issues_fail_to_move() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved("t-src", "SRC", "Source"))
        .on("ResolveTeam", resolved("t-dest", "DEST", "Destination"))
        .on(
            "GetTeamIssuesForMove",
            issue_page(&["SRC-1", "SRC-2", "SRC-3"], Value::Null, false),
        )
        .on("MoveIssueToTeam", moved(true))
        .on_error("MoveIssueToTeam", "Issue is locked")
        .on("MoveIssueToTeam", moved(false));
    Cli::for_api(&api)
        .run(&["team", "delete", "SRC", "--force", "--move-issues", "DEST"])
        .failure()
        .stdout_has("Moved 1 of 3 issue(s) to DEST")
        .stdout_has("SRC-2: Issue is locked")
        .stdout_has("SRC-3: Linear did not move the issue")
        .stderr_has("2 issue(s) could not be moved, so team SRC was not deleted");
    assert_eq!(move_variables(&api).len(), 3);
    assert!(!api.operations().contains(&"DeleteTeam".to_owned()));
}

#[test]
fn delete_fails_before_moving_when_issue_pages_have_no_cursor() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved("t-src", "SRC", "Source"))
        .on("ResolveTeam", resolved("t-dest", "DEST", "Destination"))
        .on(
            "GetTeamIssuesForMove",
            issue_page(&["SRC-1"], Value::Null, true),
        );
    Cli::for_api(&api)
        .run(&["team", "delete", "SRC", "--force", "--move-issues", "DEST"])
        .failure()
        .stderr_has("no cursor");
    assert!(move_variables(&api).is_empty());
    assert!(!api.operations().contains(&"DeleteTeam".to_owned()));
}

#[test]
fn delete_refuses_to_move_issues_to_the_same_team() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved("t-src", "SRC", "Source"))
        .on("ResolveTeam", resolved("t-src", "SRC", "Source"));
    Cli::for_api(&api)
        .run(&["team", "delete", "SRC", "--force", "--move-issues", "src"])
        .failure()
        .stderr_has("Cannot move issues to the team being deleted");
    assert_eq!(api.operations(), ["ResolveTeam", "ResolveTeam"]);
}

#[test]
fn delete_of_a_team_with_issues_needs_a_target_without_a_terminal() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved("t-src", "SRC", "Source"))
        .on(
            "GetTeamIssuesForMove",
            issue_page(&["SRC-1"], Value::Null, false),
        );
    Cli::for_api(&api)
        .run(&["team", "delete", "SRC", "--force"])
        .failure()
        .stderr_has("Team SRC has 1 issue(s)")
        .stderr_has("--move-issues");
    assert!(move_variables(&api).is_empty());
    assert!(!api.operations().contains(&"DeleteTeam".to_owned()));
}

#[test]
fn delete_requires_force_without_a_terminal() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .stdin(b"y\n")
        .run(&["team", "delete", "SRC"])
        .failure()
        .stderr_has("--force");
    assert!(api.requests().is_empty());
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
fn autolinks_uses_the_workspace_flag() {
    let cli = Cli::new()
        .env("LINEAR_TEAM_ID", "ENG")
        .env("LINEAR_WORKSPACE", "acme")
        .stub_bin("gh", "exit 0");
    cli.run(&["team", "autolinks", "--workspace", "other"])
        .success();
    assert_eq!(
        cli.calls("gh")[0][5],
        "url_template=https://linear.app/other/issue/ENG-<num>"
    );
}

#[test]
fn autolinks_looks_up_the_workspace_for_an_api_key() {
    let api = MockLinear::start();
    api.on(
        "GetViewer",
        json!({ "viewer": { "organization": { "urlKey": "acme" } } }),
    );
    let cli = Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .stub_bin("gh", "exit 0");
    cli.run(&["team", "autolinks"]).success();
    assert_eq!(
        cli.calls("gh")[0][5],
        "url_template=https://linear.app/acme/issue/ENG-<num>"
    );
}

#[test]
fn autolinks_exits_with_the_status_of_a_failed_gh() {
    let cli = Cli::new()
        .env("LINEAR_TEAM_ID", "ENG")
        .env("LINEAR_WORKSPACE", "acme")
        .stub_bin("gh", "echo 'HTTP 422' >&2; exit 4");
    let run = cli.run(&["team", "autolinks"]);
    assert_eq!(run.code, 4, "{run}");
    run.stderr_has("HTTP 422");
}

#[test]
fn autolinks_without_a_configured_team_fails_before_running_gh() {
    let cli = Cli::new()
        .env("LINEAR_WORKSPACE", "acme")
        .stub_bin("gh", "exit 0");
    cli.run(&["team", "autolinks"])
        .failure()
        .stderr_has("No team is configured");
    assert!(cli.calls("gh").is_empty());
}

#[test]
fn autolinks_reports_a_missing_gh() {
    Cli::new()
        .env("LINEAR_TEAM_ID", "ENG")
        .env("LINEAR_WORKSPACE", "acme")
        .run(&["team", "autolinks"])
        .failure()
        .stderr_has("`gh`")
        .stderr_has("https://cli.github.com");
}

#[test]
fn id_prints_the_configured_team() {
    Cli::new()
        .env("LINEAR_TEAM_ID", "ENG")
        .run(&["team", "id"])
        .success()
        .stdout_has("ENG\n");
}

#[test]
fn id_without_a_configured_team_fails() {
    Cli::new()
        .run(&["team", "id"])
        .failure()
        .stderr_has("No team id configured")
        .stderr_has("linear config");
}

#[test]
fn list_web_looks_up_the_workspace_for_an_api_key() {
    let api = MockLinear::start();
    api.on(
        "GetViewer",
        json!({ "viewer": { "organization": { "urlKey": "acme" } } }),
    );
    let cli = open_stubs(Cli::for_api(&api));
    cli.run(&["team", "list", "--web"]).success();
    assert_eq!(opened(&cli), ["https://linear.app/acme/settings/teams"]);
    assert_eq!(api.operations(), ["GetViewer"]);
}

#[test]
fn list_web_conflicts_with_json() {
    Cli::new()
        .run(&["team", "list", "--web", "--json"])
        .usage_error();
}
