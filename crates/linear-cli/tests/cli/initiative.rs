//! The `initiative` command group.
use serde_json::{Value, json};

use crate::support::{Cli, MockLinear, assert_json};
use crate::web::{open_stubs, opened};

const ID: &str = "6a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";
const OTHER_ID: &str = "7b2c3d4e-5f6a-4b7c-9d8e-0f1a2b3c4d5e";
const THIRD_ID: &str = "8c3d4e5f-6a7b-4c8d-8e9f-1a2b3c4d5e6f";
const PROJECT_ID: &str = "85d3dad6-136e-49ff-9593-33dc4b22b5ee";
const URL: &str = "https://linear.app/acme/initiative/roadmap-1a2b3c4d5e6f";

fn page(nodes: Value, end_cursor: Value, has_next: bool) -> Value {
    json!({ "nodes": nodes, "pageInfo": { "hasNextPage": has_next, "endCursor": end_cursor } })
}

fn project_node(id: &str, name: &str) -> Value {
    json!({ "id": id, "name": name, "status": { "name": "Started" } })
}

fn list_node(id: &str, name: &str, status: &str) -> Value {
    json!({
        "id": id, "slugId": format!("{}-slug", name.to_lowercase()), "name": name,
        "description": "Summary", "status": status, "targetDate": "2026-12-31",
        "health": "onTrack", "color": "#5E6AD2", "icon": null,
        "url": format!("https://linear.app/acme/initiative/{}", name.to_lowercase()),
        "archivedAt": null,
        "owner": { "id": "user-1", "displayName": "ada", "initials": "AL" },
        "projects": page(json!([project_node(PROJECT_ID, "Mobile")]), Value::Null, false)
    })
}

fn initiatives(nodes: Value) -> Value {
    json!({ "initiatives": page(nodes, Value::Null, false) })
}

fn details() -> Value {
    json!({
        "id": ID, "slugId": "1a2b3c4d5e6f", "name": "Roadmap", "description": "Ship the roadmap",
        "status": "Active", "targetDate": "2026-12-31", "health": "atRisk", "color": "#5E6AD2",
        "icon": null, "url": URL, "archivedAt": null,
        "createdAt": "2026-01-01T00:00:00.000Z", "updatedAt": "2026-02-01T00:00:00.000Z",
        "owner": { "id": "user-1", "name": "Ada Lovelace", "displayName": "ada" },
        "projects": { "nodes": [{
            "id": PROJECT_ID, "slugId": "mobile-abc", "name": "Mobile",
            "status": { "name": "Started", "type": "started" }
        }] }
    })
}

fn found(nodes: Value) -> Value {
    json!({ "initiatives": { "nodes": nodes } })
}

fn by_id(id: &str) -> Value {
    found(json!([{ "id": id, "name": "Roadmap", "slugId": "1a2b3c4d5e6f" }]))
}

fn none() -> Value {
    found(json!([]))
}

fn viewer_id() -> Value {
    json!({ "viewer": { "id": "user-me" } })
}

fn users() -> Value {
    json!({ "users": { "nodes": [
        { "id": "user-x", "email": "x@example.com", "displayName": "x", "name": "Alice X" },
        { "id": "user-alice", "email": "alice@example.com", "displayName": "alice", "name": "Alice" }
    ] } })
}

#[test]
fn list_json_defaults_to_active_initiatives() {
    let api = MockLinear::start();
    let nodes = json!([list_node(ID, "Roadmap", "Active")]);
    api.on("GetInitiatives", initiatives(nodes.clone()));
    let listed = Cli::for_api(&api)
        .run(&["initiative", "list", "--json"])
        .success()
        .json_nodes();
    assert_json(&Value::Array(listed), &nodes);
    assert_eq!(
        api.variables("GetInitiatives"),
        json!({ "filter": { "status": { "eq": "Active" } }, "includeArchived": false, "first": 25 })
    );
}

#[test]
fn list_sorts_by_status_then_name_and_follows_pages() {
    let api = MockLinear::start();
    api.on(
        "GetInitiatives",
        json!({ "initiatives": page(
            json!([list_node(ID, "Zeta", "Planned"), list_node(OTHER_ID, "Beta", "Active")]),
            json!("cursor-1"),
            true
        ) }),
    )
    .on(
        "GetInitiatives",
        initiatives(json!([list_node("init-3", "Alpha", "Active")])),
    );
    let listed = Cli::for_api(&api)
        .run(&["initiative", "list", "--all-statuses", "--json"])
        .success()
        .json_nodes();
    let names: Vec<&str> = listed
        .iter()
        .map(|node| node["name"].as_str().expect("name"))
        .collect();
    assert_eq!(names, ["Alpha", "Beta", "Zeta"]);
    let variables: Vec<Value> = api
        .requests()
        .into_iter()
        .map(|request| request.variables)
        .collect();
    assert_eq!(
        variables,
        [
            json!({ "includeArchived": false, "first": 25 }),
            json!({ "includeArchived": false, "first": 25, "after": "cursor-1" }),
        ]
    );
}

#[test]
fn list_fetches_the_remaining_projects_of_an_initiative_with_many() {
    let api = MockLinear::start();
    let mut node = list_node(ID, "Roadmap", "Active");
    node["projects"] = page(
        json!([project_node(PROJECT_ID, "Mobile")]),
        json!("projects-1"),
        true,
    );
    api.on("GetInitiatives", initiatives(json!([node]))).on(
        "GetInitiativeProjectsPage",
        json!({ "initiative": { "projects": page(
                json!([project_node(OTHER_ID, "Web")]),
                Value::Null,
                false
            ) } }),
    );
    let listed = Cli::for_api(&api)
        .run(&["initiative", "list", "--json"])
        .success()
        .json_nodes();
    let projects: Vec<&str> = listed[0]["projects"]
        .as_array()
        .expect("projects")
        .iter()
        .map(|project| project["name"].as_str().expect("name"))
        .collect();
    assert_eq!(projects, ["Mobile", "Web"]);
    assert_eq!(
        api.variables("GetInitiativeProjectsPage"),
        json!({ "id": ID, "first": 100, "after": "projects-1" })
    );
    assert!(
        api.request("GetInitiatives")
            .query
            .contains("projects(first: 50)"),
        "the nested projects are bounded so a page stays under Linear's complexity limit"
    );
}

#[test]
fn list_filters_by_owner_status_and_archived() {
    let api = MockLinear::start();
    api.on("GetViewerId", viewer_id())
        .on("GetInitiatives", initiatives(json!([])))
        .on("LookupUser", users())
        .on("GetInitiatives", initiatives(json!([])));
    let cli = Cli::for_api(&api);
    cli.run(&[
        "initiative",
        "list",
        "--owner",
        "@me",
        "--status",
        "planned",
        "--archived",
    ])
    .success()
    .stdout_has("No initiatives found");
    cli.run(&["initiative", "list", "-o", "alice@example.com", "--json"])
        .success();
    assert_eq!(
        api.variables("LookupUser"),
        json!({ "input": "alice@example.com" })
    );
    let variables: Vec<Value> = api
        .requests()
        .into_iter()
        .filter(|r| r.operation.as_deref() == Some("GetInitiatives"))
        .map(|r| r.variables)
        .collect();
    assert_eq!(
        variables,
        [
            json!({
                "filter": { "status": { "eq": "Planned" }, "owner": { "id": { "eq": "user-me" } } },
                "includeArchived": true,
                "first": 25
            }),
            json!({
                "filter": { "status": { "eq": "Active" }, "owner": { "id": { "eq": "user-alice" } } },
                "includeArchived": false,
                "first": 25
            })
        ]
    );
}

#[test]
fn list_text_shows_names_and_statuses() {
    let api = MockLinear::start();
    api.on(
        "GetInitiatives",
        initiatives(json!([list_node(ID, "Roadmap", "Active")])),
    );
    Cli::for_api(&api)
        .run(&["initiative", "list"])
        .success()
        .stdout_has("Roadmap")
        .stdout_has("Active");
}

#[test]
fn list_rejects_an_unknown_status_before_any_request() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&["initiative", "list", "--status", "someday"])
        .usage_error()
        .stderr_has("someday");
    assert!(api.requests().is_empty());
}

#[test]
fn list_web_opens_the_workspace_initiatives_page() {
    let api = MockLinear::start();
    api.on(
        "GetViewer",
        json!({ "viewer": { "organization": { "urlKey": "acme" } } }),
    );
    let cli = open_stubs(Cli::for_api(&api));
    cli.run(&["initiative", "list", "--web"])
        .success()
        .stdout_has("https://linear.app/acme/initiatives");
    assert_eq!(opened(&cli), ["https://linear.app/acme/initiatives"]);
}

#[test]
fn view_json_by_id_returns_the_initiative() {
    let api = MockLinear::start();
    api.on("GetInitiativeDetails", json!({ "initiative": details() }));
    let run = Cli::for_api(&api).run(&["initiative", "view", ID, "--json"]);
    assert_json(&run.success().json(), &details());
    assert_eq!(api.variables("GetInitiativeDetails"), json!({ "id": ID }));
}

#[test]
fn view_resolves_slugs_then_names_and_shows_details() {
    let api = MockLinear::start();
    api.on("ResolveInitiativeBySlug", none())
        .on("ResolveInitiativeByName", by_id(ID))
        .on("GetInitiativeDetails", json!({ "initiative": details() }));
    Cli::for_api(&api)
        .run(&["initiative", "view", "Roadmap"])
        .success()
        .stdout_has("Roadmap")
        .stdout_has("Ship the roadmap")
        .stdout_has("Mobile");
    assert_eq!(
        api.variables("ResolveInitiativeBySlug"),
        json!({ "slugId": "Roadmap", "includeArchived": false })
    );
    assert_eq!(
        api.variables("ResolveInitiativeByName"),
        json!({ "name": "Roadmap", "includeArchived": false })
    );
}

#[test]
fn view_resolves_urls_by_slug() {
    let api = MockLinear::start();
    api.on("ResolveInitiativeBySlug", by_id(ID))
        .on("GetInitiativeDetails", json!({ "initiative": details() }));
    Cli::for_api(&api)
        .run(&["initiative", "view", URL, "--json"])
        .success();
    assert_eq!(
        api.variables("ResolveInitiativeBySlug"),
        json!({ "slugId": "1a2b3c4d5e6f", "includeArchived": false })
    );
    assert_eq!(api.variables("GetInitiativeDetails"), json!({ "id": ID }));
}

#[test]
fn view_of_a_missing_url_never_falls_back_to_a_name() {
    let api = MockLinear::start();
    api.on("ResolveInitiativeBySlug", none());
    Cli::for_api(&api)
        .run(&["initiative", "view", URL])
        .failure()
        .stderr_has("Initiative not found")
        .stderr_has("may have been deleted");
    assert_eq!(api.operations(), ["ResolveInitiativeBySlug"]);
}

#[test]
fn view_unknown_initiative_fails() {
    let api = MockLinear::start();
    api.on("ResolveInitiativeBySlug", none())
        .on("ResolveInitiativeByName", none());
    Cli::for_api(&api)
        .run(&["initiative", "view", "nothing-here"])
        .failure()
        .stderr_has("nothing-here");
}

#[test]
fn view_web_and_app_open_the_initiative_url() {
    let api = MockLinear::start();
    api.on("GetInitiativeDetails", json!({ "initiative": details() }))
        .on("GetInitiativeDetails", json!({ "initiative": details() }));
    let cli = open_stubs(Cli::for_api(&api));
    cli.run(&["initiative", "view", ID, "--web"])
        .success()
        .stdout_has(URL);
    cli.run(&["initiative", "view", ID, "--app"])
        .success()
        .stdout_has("Linear.app");
    assert_eq!(opened(&cli), [URL, URL]);
}

fn created() -> Value {
    json!({
        "initiativeCreate": {
            "success": true,
            "initiative": { "id": ID, "slugId": "1a2b3c4d5e6f", "name": "Roadmap", "url": URL }
        }
    })
}

#[test]
fn create_sends_input_and_reports_the_url() {
    let api = MockLinear::start();
    api.on("GetViewerId", viewer_id())
        .on("CreateInitiative", created());
    Cli::for_api(&api)
        .run(&[
            "initiative",
            "create",
            "--name",
            "Roadmap",
            "--description",
            "Ship it",
            "--status",
            "Active",
            "--owner",
            "@me",
            "--target-date",
            "2026-12-31",
            "--color",
            "#5E6AD2",
            "--icon",
            "Rocket",
        ])
        .success()
        .stdout_has(URL);
    assert_eq!(
        api.variables("CreateInitiative"),
        json!({
            "input": {
                "name": "Roadmap", "description": "Ship it", "status": "Active",
                "ownerId": "user-me", "targetDate": "2026-12-31", "color": "#5E6AD2",
                "icon": "Rocket"
            }
        })
    );
}

#[test]
fn create_refuses_an_owner_name_shared_by_two_people() {
    let api = MockLinear::start();
    api.on(
        "LookupUser",
        json!({ "users": { "nodes": [
            { "id": "user-1", "name": "Sam Lee", "displayName": "sam", "email": "lee@example.com" },
            { "id": "user-2", "name": "Sam Ray", "displayName": "sam", "email": "ray@example.com" },
        ] } }),
    );
    Cli::for_api(&api)
        .run(&["initiative", "create", "-n", "Roadmap", "-o", "sam"])
        .failure()
        .stderr_has("Owner \"sam\" is ambiguous; it matches:")
        .stderr_has("Sam Ray (sam, ray@example.com)");
    assert_eq!(api.operations(), ["LookupUser"]);
}

#[test]
fn create_looks_up_owners_by_email() {
    let api = MockLinear::start();
    api.on("LookupUser", users())
        .on("CreateInitiative", created());
    Cli::for_api(&api)
        .run(&[
            "initiative",
            "create",
            "-n",
            "Roadmap",
            "-o",
            "alice@example.com",
        ])
        .success();
    assert_eq!(
        api.variables("CreateInitiative"),
        json!({ "input": { "name": "Roadmap", "ownerId": "user-alice" } })
    );
}

#[test]
fn create_validates_flags_before_any_request() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api);
    cli.run(&["initiative", "create"])
        .usage_error()
        .stderr_has("--name");
    cli.run(&["initiative", "create", "-n", "X", "-i"])
        .failure()
        .stderr_has("needs a terminal");
    cli.run(&["initiative", "create", "-n", "X", "--status", "someday"])
        .usage_error()
        .stderr_has("someday");
    cli.run(&["initiative", "create", "-n", "X", "--color", "blue"])
        .usage_error()
        .stderr_has("hex");
    cli.run(&["initiative", "create", "-n", "X", "--target-date", "soon"])
        .usage_error()
        .stderr_has("YYYY-MM-DD");
    assert!(api.requests().is_empty());
}

fn updated() -> Value {
    json!({
        "initiativeUpdate": {
            "success": true,
            "initiative": { "id": ID, "slugId": "1a2b3c4d5e6f", "name": "Renamed", "url": URL }
        }
    })
}

#[test]
fn update_sends_changed_fields() {
    let api = MockLinear::start();
    api.on("LookupUser", users())
        .on("UpdateInitiative", updated());
    Cli::for_api(&api)
        .run(&[
            "initiative",
            "update",
            ID,
            "--name",
            "Renamed",
            "--owner",
            "alice",
            "--target-date",
            "2027-01-31",
        ])
        .success()
        .stdout_has("Renamed");
    assert_eq!(api.operations(), ["LookupUser", "UpdateInitiative"]);
    assert_eq!(
        api.variables("UpdateInitiative"),
        json!({
            "id": ID,
            "input": {
                "name": "Renamed", "ownerId": "user-alice",
                "targetDate": "2027-01-31"
            }
        })
    );
}

#[test]
fn update_resolves_names() {
    let api = MockLinear::start();
    api.on("ResolveInitiativeBySlug", none())
        .on("ResolveInitiativeByName", by_id(ID))
        .on("UpdateInitiative", updated());
    Cli::for_api(&api)
        .run(&["initiative", "update", "Roadmap", "-d", "New summary"])
        .success();
    assert_eq!(
        api.variables("ResolveInitiativeByName"),
        json!({ "name": "Roadmap", "includeArchived": false })
    );
    assert_eq!(
        api.variables("UpdateInitiative"),
        json!({ "id": ID, "input": { "description": "New summary" } })
    );
}

#[test]
fn update_without_changes_fails_before_any_request() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api);
    cli.run(&["initiative", "update", ID])
        .usage_error()
        .stderr_has("No changes");
    cli.run(&["initiative", "update", ID, "-i"])
        .failure()
        .stderr_has("terminal");
    assert!(api.requests().is_empty());
}

#[test]
fn update_validates_flags_before_any_request() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api);
    cli.run(&["initiative", "update", ID, "--status", "someday"])
        .usage_error()
        .stderr_has("someday");
    cli.run(&["initiative", "update", ID, "--color", "blue"])
        .usage_error()
        .stderr_has("hex");
    cli.run(&["initiative", "update", ID, "--target-date", "2026-02-30"])
        .usage_error()
        .stderr_has("YYYY-MM-DD");
    assert!(api.requests().is_empty());
}

#[test]
fn update_sends_statuses_as_linear_enum_values() {
    let api = MockLinear::start();
    api.on("UpdateInitiative", updated());
    Cli::for_api(&api)
        .run(&["initiative", "update", ID, "--status", "completed"])
        .success();
    assert_eq!(
        api.variables("UpdateInitiative"),
        json!({ "id": ID, "input": { "status": "Completed" } })
    );
}

fn archive_detail(archived_at: Value) -> Value {
    json!({ "initiative": { "id": ID, "slugId": "1a2b3c4d5e6f", "name": "Roadmap", "archivedAt": archived_at } })
}

#[test]
fn archive_with_yes_archives_by_id() {
    let api = MockLinear::start();
    api.on("GetInitiativeForArchive", archive_detail(Value::Null))
        .on(
            "ArchiveInitiative",
            json!({ "initiativeArchive": { "success": true } }),
        );
    Cli::for_api(&api)
        .run(&["initiative", "archive", ID, "--yes"])
        .success()
        .stdout_has("Roadmap");
    assert_eq!(
        api.variables("GetInitiativeForArchive"),
        json!({ "id": ID })
    );
    assert_eq!(api.variables("ArchiveInitiative"), json!({ "id": ID }));
}

#[test]
fn archive_skips_already_archived_initiatives() {
    let api = MockLinear::start();
    api.on(
        "GetInitiativeForArchive",
        archive_detail(json!("2026-01-01T00:00:00.000Z")),
    );
    Cli::for_api(&api)
        .run(&["initiative", "archive", ID, "--yes"])
        .success()
        .stdout_has("already archived");
    assert_eq!(api.operations(), ["GetInitiativeForArchive"]);
}

#[test]
fn archive_and_delete_without_yes_or_tty_fail_before_any_request() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api);
    let commands: [&[&str]; 4] = [
        &["initiative", "archive", ID],
        &["initiative", "delete", ID],
        &["initiative", "unarchive", ID],
        &["initiative", "remove-project", ID, PROJECT_ID],
    ];
    for command in commands {
        cli.run(command).failure().stderr_has("--yes");
    }
    cli.run(&["initiative", "archive", "--bulk", ID])
        .failure()
        .stderr_has("--yes");
    assert!(api.requests().is_empty());
}

#[test]
fn archive_target_conflicts_with_bulk_flags() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&["initiative", "archive", ID, "--bulk", OTHER_ID, "--yes"])
        .usage_error();
    assert!(api.requests().is_empty());
}

#[test]
fn archive_requires_a_target() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&["initiative", "archive", "--yes"])
        .usage_error()
        .stderr_has("--bulk");
    assert!(api.requests().is_empty());
}

#[test]
fn delete_with_yes_resolves_names_and_deletes() {
    let api = MockLinear::start();
    api.on("ResolveInitiativeBySlug", none())
        .on("ResolveInitiativeByName", by_id(ID))
        .on(
            "GetInitiativeForDelete",
            json!({ "initiative": {
                "id": ID, "slugId": "1a2b3c4d5e6f", "name": "Roadmap",
                "projects": { "nodes": [{ "id": PROJECT_ID }] }
            } }),
        )
        .on(
            "DeleteInitiative",
            json!({ "initiativeDelete": { "success": true } }),
        );
    Cli::for_api(&api)
        .run(&["initiative", "delete", "Roadmap", "--yes"])
        .success()
        .stdout_has("Roadmap");
    assert_eq!(
        api.variables("ResolveInitiativeBySlug"),
        json!({ "slugId": "Roadmap", "includeArchived": true })
    );
    assert_eq!(
        api.variables("ResolveInitiativeByName"),
        json!({ "name": "Roadmap", "includeArchived": true })
    );
    assert_eq!(api.variables("DeleteInitiative"), json!({ "id": ID }));
}

/// Variables of every `operation` request, sorted so concurrent batches compare stably.
fn sorted_variables(api: &MockLinear, operation: &str) -> Vec<String> {
    let mut variables: Vec<String> = api
        .requests()
        .into_iter()
        .filter(|r| r.operation.as_deref() == Some(operation))
        .map(|r| r.variables.to_string())
        .collect();
    variables.sort();
    variables
}

fn ids(list: &[&str]) -> Vec<String> {
    let mut ids: Vec<String> = list
        .iter()
        .map(|id| json!({ "id": id }).to_string())
        .collect();
    ids.sort();
    ids
}

fn bulk_archive(id: &str, name: &str) -> Value {
    json!({ "initiative": { "id": id, "slugId": "slug", "name": name, "archivedAt": null } })
}

fn bulk_delete(id: &str, name: &str) -> Value {
    json!({ "initiative": { "id": id, "slugId": "slug", "name": name, "projects": { "nodes": [] } } })
}

#[test]
fn archive_bulk_archives_every_target() {
    let api = MockLinear::start();
    api.on("GetInitiativeForArchive", bulk_archive(ID, "Roadmap"))
        .on("GetInitiativeForArchive", bulk_archive(OTHER_ID, "Other"))
        .on(
            "ArchiveInitiative",
            json!({ "initiativeArchive": { "success": true } }),
        )
        .on(
            "ArchiveInitiative",
            json!({ "initiativeArchive": { "success": true } }),
        );
    Cli::for_api(&api)
        .run(&["initiative", "archive", "--bulk", ID, OTHER_ID, "--yes"])
        .success()
        .stdout_has("2");
    assert_eq!(
        sorted_variables(&api, "ArchiveInitiative"),
        ids(&[ID, OTHER_ID])
    );
}

#[test]
fn delete_bulk_reads_ids_from_a_file_and_stdin() {
    let api = MockLinear::start();
    for _ in 0..3 {
        api.on(
            "DeleteInitiative",
            json!({ "initiativeDelete": { "success": true } }),
        );
    }
    api.on("GetInitiativeForDelete", bulk_delete(ID, "Roadmap"))
        .on("GetInitiativeForDelete", bulk_delete(OTHER_ID, "Other"))
        .on("GetInitiativeForDelete", bulk_delete(THIRD_ID, "Third"));
    Cli::for_api(&api)
        .file("cwd/ids.txt", &format!("{ID}\n{OTHER_ID}\n"))
        .run(&["initiative", "delete", "--bulk-file", "ids.txt", "--yes"])
        .success();
    Cli::for_api(&api)
        .stdin(format!("{THIRD_ID}\n").as_bytes())
        .run(&["initiative", "delete", "--bulk-stdin", "--yes"])
        .success();
    assert_eq!(
        sorted_variables(&api, "DeleteInitiative"),
        ids(&[ID, OTHER_ID, THIRD_ID])
    );
}

#[test]
fn bulk_failures_are_reported_and_fail_the_command() {
    let api = MockLinear::start();
    api.on("GetInitiativeForArchive", bulk_archive(ID, "Roadmap"))
        .on("GetInitiativeForArchive", bulk_archive(OTHER_ID, "Other"))
        .on(
            "ArchiveInitiative",
            json!({ "initiativeArchive": { "success": true } }),
        )
        .on_error("ArchiveInitiative", "Permission denied");
    let run = Cli::for_api(&api).run(&["initiative", "archive", "--bulk", ID, OTHER_ID, "--yes"]);
    run.failure().stdout_has("Permission denied");
}

fn unarchive_detail() -> Value {
    json!({ "initiatives": { "nodes": [{
        "id": ID, "slugId": "1a2b3c4d5e6f", "name": "Roadmap",
        "archivedAt": "2026-01-01T00:00:00.000Z"
    }] } })
}

#[test]
fn unarchive_with_yes_unarchives() {
    let api = MockLinear::start();
    api.on("GetInitiativeForUnarchive", unarchive_detail()).on(
        "UnarchiveInitiative",
        json!({ "initiativeUnarchive": {
            "success": true,
            "entity": { "id": ID, "slugId": "1a2b3c4d5e6f", "name": "Roadmap", "url": URL }
        } }),
    );
    Cli::for_api(&api)
        .run(&["initiative", "unarchive", ID, "--yes"])
        .success()
        .stdout_has("Roadmap");
    assert_eq!(
        api.variables("GetInitiativeForUnarchive"),
        json!({ "id": ID })
    );
    assert_eq!(api.variables("UnarchiveInitiative"), json!({ "id": ID }));
}

#[test]
fn unarchive_resolves_archived_names() {
    let api = MockLinear::start();
    api.on("ResolveInitiativeBySlug", none())
        .on("ResolveInitiativeByName", by_id(ID))
        .on("GetInitiativeForUnarchive", unarchive_detail())
        .on(
            "UnarchiveInitiative",
            json!({ "initiativeUnarchive": {
                "success": true,
                "entity": { "id": ID, "slugId": "1a2b3c4d5e6f", "name": "Roadmap", "url": URL }
            } }),
        );
    Cli::for_api(&api)
        .run(&["initiative", "unarchive", "Roadmap", "-y"])
        .success();
    assert_eq!(
        api.variables("ResolveInitiativeByName"),
        json!({ "name": "Roadmap", "includeArchived": true })
    );
    assert_eq!(api.variables("UnarchiveInitiative"), json!({ "id": ID }));
}

/// The names of both sides and the initiatives the project is linked to.
fn links(initiatives: &[(&str, &str)]) -> Value {
    let nodes: Vec<Value> = initiatives
        .iter()
        .map(|(link, initiative)| json!({ "id": link, "initiative": { "id": initiative } }))
        .collect();
    json!({
        "initiative": { "name": "Roadmap" },
        "project": {
            "name": "Mobile",
            "initiativeToProjects": page(json!(nodes), Value::Null, false)
        }
    })
}

#[test]
fn add_project_links_with_a_sort_order() {
    let api = MockLinear::start();
    api.on(
        "GetInitiativeProjectLinks",
        links(&[("link-other", OTHER_ID)]),
    )
    .on(
        "AddProjectToInitiative",
        json!({ "initiativeToProjectCreate": { "success": true } }),
    );
    Cli::for_api(&api)
        .run(&[
            "initiative",
            "add-project",
            ID,
            PROJECT_ID,
            "--sort-order",
            "2.5",
        ])
        .success()
        .stdout_has("✓ Added project Mobile to initiative Roadmap");
    assert_eq!(
        api.variables("GetInitiativeProjectLinks"),
        json!({ "initiativeId": ID, "projectId": PROJECT_ID, "after": null })
    );
    assert_eq!(
        api.variables("AddProjectToInitiative"),
        json!({ "input": { "initiativeId": ID, "projectId": PROJECT_ID, "sortOrder": 2.5 } })
    );
}

#[test]
fn add_project_resolves_names() {
    let api = MockLinear::start();
    api.on("ResolveInitiativeBySlug", none())
        .on("ResolveInitiativeByName", by_id(ID))
        .on(
            "GetProjectIdByName",
            json!({ "projects": { "nodes": [{ "id": PROJECT_ID }] } }),
        )
        .on("GetInitiativeProjectLinks", links(&[]))
        .on(
            "AddProjectToInitiative",
            json!({ "initiativeToProjectCreate": { "success": true } }),
        );
    Cli::for_api(&api)
        .run(&["initiative", "add-project", "Roadmap", "Mobile"])
        .success();
    assert_eq!(
        api.variables("AddProjectToInitiative")["input"],
        json!({ "initiativeId": ID, "projectId": PROJECT_ID })
    );
}

#[test]
fn add_project_reports_an_existing_link_without_adding() {
    let api = MockLinear::start();
    api.on("GetInitiativeProjectLinks", links(&[("link-1", ID)]));
    Cli::for_api(&api)
        .run(&["initiative", "add-project", ID, PROJECT_ID])
        .success()
        .stdout_has("already linked");
    assert_eq!(api.operations(), ["GetInitiativeProjectLinks"]);
}

#[test]
fn remove_project_deletes_the_link() {
    let api = MockLinear::start();
    api.on(
        "GetInitiativeProjectLinks",
        links(&[("link-other", OTHER_ID), ("link-1", ID)]),
    )
    .on(
        "RemoveProjectFromInitiative",
        json!({ "initiativeToProjectDelete": { "success": true } }),
    );
    Cli::for_api(&api)
        .run(&["initiative", "remove-project", ID, PROJECT_ID, "--yes"])
        .success()
        .stdout_has("✓ Removed project Mobile from initiative Roadmap");
    assert_eq!(
        api.variables("RemoveProjectFromInitiative"),
        json!({ "id": "link-1" })
    );
}

#[test]
fn remove_project_follows_link_pages() {
    let api = MockLinear::start();
    let mut first = links(&[("link-other", OTHER_ID)]);
    first["project"]["initiativeToProjects"]["pageInfo"] =
        json!({ "hasNextPage": true, "endCursor": "cursor-1" });
    api.on("GetInitiativeProjectLinks", first)
        .on("GetInitiativeProjectLinks", links(&[("link-1", ID)]))
        .on(
            "RemoveProjectFromInitiative",
            json!({ "initiativeToProjectDelete": { "success": true } }),
        );
    Cli::for_api(&api)
        .run(&["initiative", "remove-project", ID, PROJECT_ID, "--yes"])
        .success();
    let cursors: Vec<Value> = api
        .requests()
        .into_iter()
        .filter(|r| r.operation.as_deref() == Some("GetInitiativeProjectLinks"))
        .map(|r| r.variables["after"].clone())
        .collect();
    assert_eq!(cursors, [Value::Null, json!("cursor-1")]);
    assert_eq!(
        api.variables("RemoveProjectFromInitiative"),
        json!({ "id": "link-1" })
    );
}

#[test]
fn remove_project_fails_for_an_unlinked_project() {
    let api = MockLinear::start();
    api.on(
        "GetInitiativeProjectLinks",
        links(&[("link-other", OTHER_ID)]),
    );
    Cli::for_api(&api)
        .run(&["initiative", "remove-project", ID, PROJECT_ID, "--yes"])
        .failure()
        .stderr_has("not linked");
    assert_eq!(api.operations(), ["GetInitiativeProjectLinks"]);
}

fn comment(id: &str, body: &str, parent: Option<&str>) -> Value {
    json!({
        "id": id, "body": body, "quotedText": null,
        "createdAt": "2026-01-02T12:00:00.000Z", "updatedAt": "2026-01-02T12:00:00.000Z", "editedAt": null,
        "url": format!("https://linear.app/acme/initiative/roadmap#{id}"),
        "user": { "id": "user-1", "name": "ada", "displayName": "Ada Lovelace" },
        "externalUser": null, "botActor": null,
        "parent": parent.map(|id| json!({ "id": id }))
    })
}

#[test]
fn comment_add_resolves_the_initiative_and_posts_the_body() {
    let api = MockLinear::start();
    api.on("ResolveInitiativeBySlug", none())
        .on("ResolveInitiativeByName", by_id(ID))
        .on(
            "AddComment",
            json!({ "commentCreate": {
            "success": true,
            "comment": { "id": "comment-1", "url": "https://linear.app/acme/comment/c0de" }
        } }),
        );
    Cli::for_api(&api)
        .run(&[
            "initiative",
            "comment",
            "add",
            "Roadmap",
            "--body",
            "Hello",
            "--parent",
            "comment-0",
        ])
        .success()
        .stdout_has("https://linear.app/acme/comment/c0de");
    assert_eq!(
        api.variables("AddComment"),
        json!({ "input": { "body": "Hello", "initiativeId": ID, "parentId": "comment-0" } })
    );
    assert_eq!(
        api.variables("ResolveInitiativeBySlug"),
        json!({ "slugId": "Roadmap", "includeArchived": false })
    );
}

#[test]
fn comment_add_rejects_ambiguous_names() {
    let api = MockLinear::start();
    api.on("ResolveInitiativeBySlug", none()).on(
        "ResolveInitiativeByName",
        found(json!([
            { "id": ID, "name": "Roadmap", "slugId": "1a2b3c4d5e6f" },
            { "id": OTHER_ID, "name": "roadmap", "slugId": "6f5e4d3c2b1a" }
        ])),
    );
    Cli::for_api(&api)
        .run(&["initiative", "comment", "add", "Roadmap", "-b", "Hi"])
        .failure()
        .stderr_has(&format!(
            "Initiative \"Roadmap\" is ambiguous; it matches:\n  Roadmap — 1a2b3c4d5e6f ({ID})\n  roadmap — 6f5e4d3c2b1a ({OTHER_ID})"
        ))
        .stderr_has("Pass the initiative's slug ID or UUID instead.");
    assert!(!api.operations().contains(&"AddComment".to_owned()));
}

#[test]
fn comment_add_rejects_a_parent_link_before_any_request() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&[
            "initiative",
            "comment",
            "add",
            "Roadmap",
            "-b",
            "Hi",
            "-p",
            "https://linear.app/acme/project/x-000000000001",
        ])
        .failure()
        .stderr_has("Pass the UUID of the comment to reply to.");
    assert!(api.requests().is_empty());
}

#[test]
fn comment_list_json_returns_comments() {
    let comments = json!([
        comment("comment-1", "Root A", None),
        comment("comment-2", "Reply B", Some("comment-1"))
    ]);
    let api = MockLinear::start();
    api.on(
        "GetInitiativeComments",
        json!({
            "initiative": { "id": ID, "name": "Roadmap" },
            "comments": page(comments.clone(), Value::Null, false)
        }),
    );
    let json = Cli::for_api(&api)
        .run(&["initiative", "comment", "list", ID, "--json"])
        .success()
        .json_nodes();
    assert_eq!(Value::Array(json), comments);
    assert_eq!(
        api.variables("GetInitiativeComments"),
        json!({ "id": ID, "filterId": ID, "after": null, "first": 100 })
    );
}

#[test]
fn comment_list_stops_on_an_empty_cursor() {
    let api = MockLinear::start();
    api.on(
        "GetInitiativeComments",
        json!({
            "initiative": { "id": ID, "name": "Roadmap" },
            "comments": page(json!([comment("comment-1", "Root A", None)]), json!(""), true)
        }),
    );
    Cli::for_api(&api)
        .run(&["initiative", "comment", "list", ID])
        .failure()
        .stderr_has("cursor");
    assert_eq!(api.operations(), ["GetInitiativeComments"]);
}
