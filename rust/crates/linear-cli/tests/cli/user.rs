//! The `user` command group.
use serde_json::{Value, json};

use crate::support::{Cli, MockLinear};

fn member(id: &str, name: &str, active: bool) -> Value {
    json!({
        "id": id, "name": name, "displayName": id, "email": format!("{id}@example.com"),
        "active": active, "initials": "XX", "description": null, "timezone": null,
        "lastSeen": null, "statusEmoji": null, "statusLabel": null, "guest": false,
        "isAssignable": true, "admin": false, "owner": false, "isMe": false,
        "url": format!("https://linear.app/acme/profiles/{id}")
    })
}

fn members(nodes: Vec<Value>, end_cursor: Value, has_next: bool) -> Value {
    json!({ "viewer": { "organization": { "users": {
        "nodes": nodes,
        "pageInfo": { "hasNextPage": has_next, "endCursor": end_cursor }
    } } } })
}

#[test]
fn list_json_returns_active_members() {
    let api = MockLinear::start();
    let ada = member("ada", "Ada Lovelace", true);
    api.on(
        "GetOrganizationMembers",
        members(vec![ada.clone()], Value::Null, false),
    );
    let listed = Cli::for_api(&api)
        .run(&["user", "list", "--json"])
        .success()
        .json_nodes();
    assert_eq!(listed, [ada]);
    assert_eq!(
        api.variables("GetOrganizationMembers"),
        json!({ "includeDisabled": false, "first": 100 })
    );
}

#[test]
fn list_all_includes_inactive_members_and_follows_pages() {
    let api = MockLinear::start();
    api.on(
        "GetOrganizationMembers",
        members(vec![member("ada", "Ada Lovelace", true)], json!("c1"), true),
    )
    .on(
        "GetOrganizationMembers",
        members(
            vec![member("bob", "Bob Builder", false)],
            Value::Null,
            false,
        ),
    );
    Cli::for_api(&api)
        .run(&["user", "list", "--all"])
        .success()
        .stdout_has("Ada Lovelace")
        .stdout_has("Bob Builder");
    let variables: Vec<Value> = api.requests().into_iter().map(|r| r.variables).collect();
    assert_eq!(
        variables,
        [
            json!({ "includeDisabled": true, "first": 100 }),
            json!({ "includeDisabled": true, "first": 100, "after": "c1" })
        ]
    );
}

#[test]
fn list_uses_the_selected_workspace_key() {
    let api = MockLinear::start();
    api.on(
        "GetOrganizationMembers",
        members(vec![], Value::Null, false),
    );
    Cli::new()
        .endpoint(&api)
        .credentials("default = \"acme\"\nacme = \"key-acme\"\nbeta = \"key-beta\"\n")
        .run(&["--workspace", "beta", "user", "list", "--json"])
        .success();
    assert_eq!(
        api.request("GetOrganizationMembers")
            .header("authorization"),
        Some("key-beta")
    );
}

#[test]
fn list_reports_graphql_errors() {
    let api = MockLinear::start();
    api.on_error("GetOrganizationMembers", "Rate limited");
    Cli::for_api(&api)
        .run(&["user", "list"])
        .failure()
        .stderr_has("Rate limited");
}

#[test]
fn list_text_is_a_table_of_members() {
    let api = MockLinear::start();
    let mut rich = member("rich", "Rich Person", true);
    rich["displayName"] = json!("");
    rich["description"] = json!("Engineer");
    rich["timezone"] = json!("America/Los_Angeles");
    rich["statusEmoji"] = json!("🌿");
    rich["statusLabel"] = json!("Away");
    rich["guest"] = json!(true);
    rich["isAssignable"] = json!(false);
    rich["admin"] = json!(true);
    rich["owner"] = json!(true);
    rich["isMe"] = json!(true);
    api.on(
        "GetOrganizationMembers",
        members(vec![rich], Value::Null, false),
    );
    let run = Cli::for_api(&api).run(&["user", "list"]);
    run.success();
    assert_eq!(
        run.stdout,
        "NAME               USERNAME  EMAIL             ROLE   LAST SEEN\n\
         Rich Person (you)            rich@example.com  Owner\n"
    );
}

#[test]
fn list_text_explains_empty_and_all_inactive_results() {
    let api = MockLinear::start();
    api.on(
        "GetOrganizationMembers",
        members(vec![], Value::Null, false),
    )
    .on(
        "GetOrganizationMembers",
        members(vec![member("old", "Old Timer", false)], Value::Null, false),
    );
    let cli = Cli::for_api(&api);
    cli.run(&["user", "list"])
        .success()
        .stdout_has("No members found in this workspace.");
    cli.run(&["user", "list"])
        .success()
        .stdout_has("Use --all to include inactive members.");
}

#[test]
fn list_fails_when_the_cursor_does_not_advance() {
    let api = MockLinear::start();
    api.on(
        "GetOrganizationMembers",
        members(
            vec![member("ada", "Ada Lovelace", true)],
            json!("same"),
            true,
        ),
    )
    .on(
        "GetOrganizationMembers",
        members(
            vec![member("ada", "Ada Lovelace", true)],
            json!("same"),
            true,
        ),
    );
    let run = Cli::for_api(&api).run(&["user", "list"]);
    run.failure().stderr_has("same pagination cursor");
    assert!(run.stdout.is_empty());
}
