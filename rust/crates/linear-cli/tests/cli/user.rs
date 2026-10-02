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
    let json = Cli::for_api(&api)
        .run(&["user", "list", "--json"])
        .success()
        .json();
    assert_eq!(json["nodes"], json!([ada]));
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
