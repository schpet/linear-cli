//! The `initiative-update` and `project-update` command groups (timeline status posts).
use serde_json::{Value, json};

use crate::support::{Cli, MockLinear, Run, assert_json, nodes};

const INITIATIVE_ID: &str = "6a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";
const PROJECT_ID: &str = "85d3dad6-136e-49ff-9593-33dc4b22b5ee";

/// The updates in a `list --json` output, which may be printed bare or under their parent
/// entity's `connection` field.
fn listed(run: &Run, connection: &str) -> Value {
    let json = run.success().json();
    let list = json.get(connection).unwrap_or(&json);
    Value::Array(nodes(list))
}

fn initiative_name() -> Value {
    json!({ "initiative": { "name": "Roadmap", "slugId": "1a2b3c4d5e6f" } })
}

fn initiative_created(health: Value) -> Value {
    json!({
        "initiativeUpdateCreate": {
            "success": true,
            "initiativeUpdate": {
                "id": "update-1", "body": "Shipped", "health": health,
                "url": "https://linear.app/acme/initiative/roadmap/updates#update-1",
                "createdAt": "2026-03-01T00:00:00.000Z",
                "initiative": { "name": "Roadmap", "slugId": "1a2b3c4d5e6f" }
            }
        }
    })
}

fn project_created(health: Value) -> Value {
    json!({
        "projectUpdateCreate": {
            "success": true,
            "projectUpdate": {
                "id": "update-2", "body": "Shipped", "health": health,
                "url": "https://linear.app/acme/project/mobile/updates#update-2",
                "createdAt": "2026-03-01T00:00:00.000Z",
                "project": { "name": "Mobile", "slugId": "mobile-abc" }
            }
        }
    })
}

#[test]
fn initiative_create_posts_body_and_health() {
    let api = MockLinear::start();
    api.on("GetInitiativeNameForStatusUpdate", initiative_name())
        .on(
            "CreateInitiativeUpdate",
            initiative_created(json!("atRisk")),
        );
    Cli::for_api(&api)
        .run(&[
            "initiative-update",
            "create",
            INITIATIVE_ID,
            "--body",
            "Shipped",
            "--health",
            "atRisk",
        ])
        .success()
        .stdout_has("Roadmap")
        .stdout_has("https://linear.app/acme/initiative/roadmap/updates#update-1");
    assert_eq!(
        api.variables("GetInitiativeNameForStatusUpdate"),
        json!({ "id": INITIATIVE_ID })
    );
    assert_eq!(
        api.variables("CreateInitiativeUpdate"),
        json!({ "input": { "initiativeId": INITIATIVE_ID, "body": "Shipped", "health": "atRisk" } })
    );
}

#[test]
fn initiative_create_resolves_names_and_reads_body_files() {
    let api = MockLinear::start();
    api.on(
        "GetInitiativeBySlugForStatusUpdate",
        json!({ "initiatives": { "nodes": [] } }),
    )
    .on(
        "GetInitiativeByNameForStatusUpdate",
        json!({ "initiatives": { "nodes": [{ "id": INITIATIVE_ID, "name": "Roadmap" }] } }),
    )
    .on("GetInitiativeNameForStatusUpdate", initiative_name())
    .on("CreateInitiativeUpdate", initiative_created(Value::Null));
    Cli::for_api(&api)
        .file("cwd/update.md", "## Progress\n\n- shipped, finally\n")
        .run(&[
            "initiative-update",
            "create",
            "Roadmap",
            "--body-file",
            "update.md",
        ])
        .success();
    assert_eq!(
        api.variables("GetInitiativeByNameForStatusUpdate"),
        json!({ "name": "Roadmap" })
    );
    let input = &api.variables("CreateInitiativeUpdate")["input"];
    assert_eq!(input["initiativeId"], INITIATIVE_ID);
    assert_eq!(
        input["body"].as_str().map(str::trim_end),
        Some("## Progress\n\n- shipped, finally")
    );
}

#[test]
fn initiative_create_rejects_unknown_health() {
    let api = MockLinear::start();
    // The initiative is looked up before the health value is checked.
    api.on("GetInitiativeNameForStatusUpdate", initiative_name());
    Cli::for_api(&api)
        .run(&[
            "initiative-update",
            "create",
            INITIATIVE_ID,
            "--body",
            "x",
            "--health",
            "great",
        ])
        .failure()
        .stderr_has("great");
    assert!(
        !api.operations()
            .contains(&"CreateInitiativeUpdate".to_owned())
    );
}

#[test]
fn project_create_rejects_unknown_health_before_any_request() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&[
            "project-update",
            "create",
            PROJECT_ID,
            "--body",
            "x",
            "--health",
            "great",
        ])
        .failure()
        .stderr_has("great");
    assert!(api.requests().is_empty());
}

#[test]
fn initiative_create_interactive_requires_a_terminal() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&[
            "initiative-update",
            "create",
            INITIATIVE_ID,
            "--interactive",
        ])
        .failure()
        .stderr_has("terminal");
    assert!(api.requests().is_empty());
}

fn initiative_updates() -> Value {
    json!({
        "initiative": {
            "name": "Roadmap", "slugId": "1a2b3c4d5e6f",
            "initiativeUpdates": { "nodes": [
                {
                    "id": "update-1", "body": "Shipped the beta", "health": "onTrack",
                    "url": "https://linear.app/acme/initiative/roadmap/updates#update-1",
                    "createdAt": "2026-03-01T00:00:00.000Z", "user": { "name": "Ada" }
                },
                {
                    "id": "update-0", "body": "Kickoff", "health": "atRisk",
                    "url": "https://linear.app/acme/initiative/roadmap/updates#update-0",
                    "createdAt": "2026-02-01T00:00:00.000Z", "user": null
                }
            ] }
        }
    })
}

#[test]
fn initiative_list_json_returns_updates() {
    let api = MockLinear::start();
    api.on("ListInitiativeUpdates", initiative_updates());
    let run = Cli::for_api(&api).run(&[
        "initiative-update",
        "list",
        INITIATIVE_ID,
        "--limit",
        "5",
        "--json",
    ]);
    assert_json(
        &listed(&run, "initiativeUpdates"),
        &initiative_updates()["initiative"]["initiativeUpdates"]["nodes"],
    );
    assert_eq!(
        api.variables("ListInitiativeUpdates"),
        json!({ "id": INITIATIVE_ID, "first": 5 })
    );
}

#[test]
fn initiative_list_text_shows_updates() {
    let api = MockLinear::start();
    api.on("ListInitiativeUpdates", initiative_updates());
    Cli::for_api(&api)
        .run(&["initiative-update", "list", INITIATIVE_ID])
        .success()
        .stdout_has("Shipped the beta")
        .stdout_has("Kickoff");
    assert_eq!(
        api.variables("ListInitiativeUpdates"),
        json!({ "id": INITIATIVE_ID, "first": 10 })
    );
}

#[test]
fn list_rejects_a_zero_limit() {
    let cli = Cli::new();
    cli.run(&["initiative-update", "list", INITIATIVE_ID, "--limit", "0"])
        .usage_error();
    cli.run(&["project-update", "list", PROJECT_ID, "--limit", "0"])
        .usage_error();
}

#[test]
fn project_create_posts_body_and_health() {
    let api = MockLinear::start();
    api.on("CreateProjectUpdate", project_created(json!("offTrack")));
    Cli::for_api(&api)
        .run(&[
            "project-update",
            "create",
            PROJECT_ID,
            "--body",
            "Shipped",
            "--health",
            "offTrack",
        ])
        .success()
        .stdout_has("https://linear.app/acme/project/mobile/updates#update-2");
    assert_eq!(
        api.variables("CreateProjectUpdate"),
        json!({ "input": { "projectId": PROJECT_ID, "body": "Shipped", "health": "offTrack" } })
    );
}

#[test]
fn project_create_resolves_project_names() {
    let api = MockLinear::start();
    api.on(
        "GetProjectIdByName",
        json!({ "projects": { "nodes": [{ "id": PROJECT_ID }] } }),
    )
    .on("CreateProjectUpdate", project_created(Value::Null));
    Cli::for_api(&api)
        .run(&["project-update", "create", "Mobile", "--body", "Shipped"])
        .success();
    assert_eq!(
        api.variables("GetProjectIdByName"),
        json!({ "name": "Mobile" })
    );
    assert_eq!(
        api.variables("CreateProjectUpdate")["input"]["projectId"],
        PROJECT_ID
    );
}

#[test]
fn project_create_reads_a_piped_body() {
    let api = MockLinear::start();
    api.on("CreateProjectUpdate", project_created(Value::Null));
    Cli::for_api(&api)
        .stdin(b"Shipped\n")
        .run(&["project-update", "create", PROJECT_ID])
        .success();
    let input = &api.variables("CreateProjectUpdate")["input"];
    assert_eq!(input["body"].as_str().map(str::trim_end), Some("Shipped"));
}

#[test]
fn project_create_reports_graphql_errors() {
    let api = MockLinear::start();
    api.on_error("CreateProjectUpdate", "Project is archived");
    Cli::for_api(&api)
        .run(&["project-update", "create", PROJECT_ID, "--body", "x"])
        .failure()
        .stderr_has("Project is archived");
}

fn project_updates(has_next: bool) -> Value {
    json!({
        "project": {
            "name": "Mobile", "slugId": "mobile-abc",
            "projectUpdates": {
                "nodes": [{
                    "id": "update-2", "body": "Beta is out", "health": "onTrack",
                    "url": "https://linear.app/acme/project/mobile/updates#update-2",
                    "createdAt": "2026-03-01T00:00:00.000Z",
                    "user": { "name": "Ada Lovelace", "displayName": "ada" }
                }],
                "pageInfo": { "hasNextPage": has_next, "endCursor": "cursor-1" }
            }
        }
    })
}

#[test]
fn project_list_json_returns_updates() {
    let api = MockLinear::start();
    api.on("ListProjectUpdates", project_updates(false));
    let run = Cli::for_api(&api).run(&["project-update", "list", PROJECT_ID, "--json"]);
    assert_json(
        &listed(&run, "projectUpdates"),
        &project_updates(false)["project"]["projectUpdates"]["nodes"],
    );
    assert_eq!(
        api.variables("ListProjectUpdates"),
        json!({ "id": PROJECT_ID, "first": 10 })
    );
}

#[test]
fn project_list_text_shows_updates() {
    let api = MockLinear::start();
    api.on("ListProjectUpdates", project_updates(true));
    Cli::for_api(&api)
        .run(&["project-update", "list", PROJECT_ID, "--limit", "1"])
        .success()
        .stdout_has("Beta is out");
    assert_eq!(
        api.variables("ListProjectUpdates"),
        json!({ "id": PROJECT_ID, "first": 1 })
    );
}

#[test]
fn project_list_unknown_project_fails() {
    let api = MockLinear::start();
    api.on("ListProjectUpdates", json!({ "project": null }));
    Cli::for_api(&api)
        .run(&["project-update", "list", PROJECT_ID])
        .failure()
        .stderr_has(PROJECT_ID);
}
