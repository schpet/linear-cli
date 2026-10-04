//! The `initiative-update` and `project-update` command groups (timeline status posts).
use serde_json::{Value, json};

use crate::support::{Cli, MockLinear, Run, assert_json};

const INITIATIVE_ID: &str = "6a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";
const PROJECT_ID: &str = "85d3dad6-136e-49ff-9593-33dc4b22b5ee";

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
    api.on(
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
    assert_eq!(api.operations(), ["CreateInitiativeUpdate"]);
    assert_eq!(
        api.variables("CreateInitiativeUpdate"),
        json!({ "input": { "initiativeId": INITIATIVE_ID, "body": "Shipped", "health": "atRisk" } })
    );
}

#[test]
fn initiative_create_resolves_names_and_reads_body_files() {
    let api = MockLinear::start();
    api.on(
        "ResolveInitiativeBySlug",
        json!({ "initiatives": { "nodes": [] } }),
    )
    .on(
        "ResolveInitiativeByName",
        json!({ "initiatives": { "nodes": [
            { "id": INITIATIVE_ID, "name": "Roadmap", "slugId": "1a2b3c4d5e6f" }
        ] } }),
    )
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
        api.variables("ResolveInitiativeBySlug"),
        json!({ "slugId": "Roadmap", "includeArchived": false })
    );
    assert_eq!(
        api.variables("ResolveInitiativeByName"),
        json!({ "name": "Roadmap", "includeArchived": false })
    );
    let input = &api.variables("CreateInitiativeUpdate")["input"];
    assert_eq!(input["initiativeId"], INITIATIVE_ID);
    assert_eq!(
        input["body"].as_str().map(str::trim_end),
        Some("## Progress\n\n- shipped, finally")
    );
}

#[test]
fn initiative_create_rejects_unknown_health_before_any_request() {
    let api = MockLinear::start();
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
        .usage_error()
        .stderr_has("great");
    assert!(api.requests().is_empty());
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
        .usage_error()
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
                    "createdAt": "2026-03-01T00:00:00.000Z", "user": { "name": "Ada", "displayName": "ada" }
                },
                {
                    "id": "update-0", "body": "Kickoff", "health": "atRisk",
                    "url": "https://linear.app/acme/initiative/roadmap/updates#update-0",
                    "createdAt": "2026-02-01T00:00:00.000Z", "user": null
                }
            ], "pageInfo": { "hasNextPage": false, "endCursor": null } }
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
    assert_update_keys(&run);
    assert_json(
        &run.success().json(),
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
    assert_update_keys(&run);
    assert_json(
        &run.success().json(),
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

#[test]
fn both_lists_render_the_same_table() {
    let api = MockLinear::start();
    api.on("ListInitiativeUpdates", initiative_updates())
        .on("ListProjectUpdates", project_updates(false));
    let cli = Cli::for_api(&api);
    let initiative = cli.run(&["initiative-update", "list", INITIATIVE_ID]);
    let project = cli.run(&["project-update", "list", PROJECT_ID]);
    let lines = |run: &Run| -> Vec<String> {
        run.success()
            .stdout
            .lines()
            .map(|line| line.trim_end().to_owned())
            .collect()
    };
    let initiative = lines(&initiative);
    let project = lines(&project);
    let cells = |line: &str| -> Vec<String> {
        line.split("  ")
            .map(str::trim)
            .filter(|cell| !cell.is_empty())
            .map(str::to_owned)
            .collect()
    };
    assert_eq!(
        cells(&initiative[0]),
        ["DATE", "HEALTH", "AUTHOR", "UPDATE"]
    );
    assert_eq!(
        cells(&initiative[1])[1..],
        ["On Track", "ada", "Shipped the beta"]
    );
    assert_eq!(cells(&initiative[2])[1..], ["At Risk", "-", "Kickoff"]);
    assert_eq!(initiative.len(), 3, "{initiative:?}");
    assert_eq!(cells(&project[0]), cells(&initiative[0]));
    assert_eq!(cells(&project[1])[1..], ["On Track", "ada", "Beta is out"]);
}

#[test]
fn create_reads_a_missing_body_file_as_an_error_before_any_request() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&[
            "project-update",
            "create",
            PROJECT_ID,
            "--body-file",
            "missing.md",
        ])
        .failure()
        .stderr_has("missing.md");
    assert!(api.requests().is_empty());
}

#[test]
fn initiative_create_reports_lookup_failures() {
    let api = MockLinear::start();
    api.on_error("ResolveInitiativeBySlug", "Rate limited");
    Cli::for_api(&api)
        .run(&["initiative-update", "create", "Roadmap", "--body", "x"])
        .failure()
        .stderr_has("Rate limited");
    assert!(
        !api.operations()
            .contains(&"CreateInitiativeUpdate".to_owned())
    );
}

#[test]
fn create_warns_that_an_unreadable_reply_may_have_created_it() {
    let api = MockLinear::start();
    api.on_raw("CreateInitiativeUpdate", 200, "not json");
    Cli::for_api(&api)
        .run(&[
            "initiative-update",
            "create",
            INITIATIVE_ID,
            "--body",
            "Shipped",
        ])
        .failure()
        .stderr_has("status update may already exist");
}

fn assert_update_keys(run: &Run) {
    let json = run.success().json();
    let row = json[0].as_object().expect("update object");
    assert_eq!(
        row.keys().map(String::as_str).collect::<Vec<_>>(),
        ["id", "body", "health", "url", "createdAt", "user"]
    );
    if let Some(user) = row["user"].as_object() {
        assert_eq!(
            user.keys().map(String::as_str).collect::<Vec<_>>(),
            ["name", "displayName"]
        );
    }
}

#[test]
fn lists_preserve_unknown_health_and_author_fallbacks() {
    for (command, id, operation, parent, connection) in [
        (
            "project-update",
            PROJECT_ID,
            "ListProjectUpdates",
            "project",
            "projectUpdates",
        ),
        (
            "initiative-update",
            INITIATIVE_ID,
            "ListInitiativeUpdates",
            "initiative",
            "initiativeUpdates",
        ),
    ] {
        for (user, author) in [
            (
                json!({"name": "Full Name", "displayName": "handle"}),
                "handle",
            ),
            (json!({"name": "Full Name", "displayName": ""}), "Full Name"),
            (json!({"name": "", "displayName": ""}), "-"),
            (Value::Null, "-"),
        ] {
            let mut reply = if parent == "project" {
                project_updates(false)
            } else {
                initiative_updates()
            };
            reply[parent][connection]["nodes"][0]["health"] = json!("futureHealth");
            reply[parent][connection]["nodes"][0]["user"] = user.clone();
            let api = MockLinear::start();
            api.on(operation, reply.clone())
                .on(operation, reply.clone());
            let cli = Cli::for_api(&api);
            let run = cli.run(&[command, "list", id, "--json"]);
            assert_update_keys(&run);
            assert_json(&run.success().json(), &reply[parent][connection]["nodes"]);
            let text = cli.run(&[command, "list", id]);
            let first = text
                .success()
                .stdout
                .lines()
                .nth(1)
                .expect("first update row");
            let columns: Vec<_> = first
                .split("  ")
                .map(str::trim)
                .filter(|cell| !cell.is_empty())
                .collect();
            assert_eq!(columns[1], "futureHealth", "{first}");
            assert_eq!(columns[2], author, "{first}");
        }
    }
}

#[test]
fn project_list_preserves_null_health_and_user() {
    let mut reply = project_updates(false);
    reply["project"]["projectUpdates"]["nodes"][0]["health"] = Value::Null;
    reply["project"]["projectUpdates"]["nodes"][0]["user"] = Value::Null;
    let api = MockLinear::start();
    api.on("ListProjectUpdates", reply.clone())
        .on("ListProjectUpdates", reply.clone());
    let cli = Cli::for_api(&api);
    let run = cli.run(&["project-update", "list", PROJECT_ID, "--json"]);
    assert_json(
        &run.success().json(),
        &reply["project"]["projectUpdates"]["nodes"],
    );
    assert_update_keys(&run);
    let run = cli.run(&["project-update", "list", PROJECT_ID]);
    let row = run.success().stdout.lines().nth(1).expect("update row");
    let columns: Vec<_> = row
        .split("  ")
        .map(str::trim)
        .filter(|cell| !cell.is_empty())
        .collect();
    assert_eq!(columns[1], "-", "missing health: {row}");
    assert_eq!(columns[2], "-", "missing author: {row}");
}

/// An editor that writes `Shipped the beta` into the file it is given.
const EDITOR: &str = "printf 'Shipped the beta\\n' > \"$1\"";

fn editor_cli(api: &MockLinear, script: &str) -> Cli {
    Cli::for_api(api)
        .stub_bin("editor", script)
        .env("VISUAL", "editor")
}

#[test]
fn project_create_on_a_terminal_asks_for_health_and_confirms_after_the_editor() {
    let api = MockLinear::start();
    api.on("CreateProjectUpdate", project_created(json!("atRisk")));
    editor_cli(&api, EDITOR)
        .run_tty(
            &["project-update", "create", PROJECT_ID],
            &[
                // The second choice, At Risk.
                ("Health status", "\x1b[B\r"),
                ("Post this update to", "y\r"),
            ],
        )
        .success()
        .stdout_has("Created status update for Mobile");
    assert_eq!(
        api.variables("CreateProjectUpdate"),
        json!({ "input": { "projectId": PROJECT_ID, "body": "Shipped the beta", "health": "atRisk" } })
    );
}

#[test]
fn project_create_on_a_terminal_posts_nothing_unless_confirmed() {
    let api = MockLinear::start();
    editor_cli(&api, EDITOR)
        .run_tty(
            &[
                "project-update",
                "create",
                PROJECT_ID,
                "--health",
                "on-track",
            ],
            &[("(y/N)", "\r")],
        )
        .success()
        .stdout_has("Canceled.");
    assert!(api.requests().is_empty());
}

#[test]
fn initiative_create_cancels_when_the_editor_is_left_empty() {
    let api = MockLinear::start();
    editor_cli(&api, ": > \"$1\"")
        .run_tty(&["initiative-update", "create", INITIATIVE_ID], &[])
        .success()
        .stdout_has("No content entered.")
        .stdout_has("Canceled.");
    assert!(api.requests().is_empty());
}

#[test]
fn create_with_yes_posts_the_edited_update_without_asking() {
    let api = MockLinear::start();
    api.on("CreateProjectUpdate", project_created(json!("onTrack")));
    let run = editor_cli(&api, EDITOR).run_tty(
        &[
            "project-update",
            "create",
            PROJECT_ID,
            "--health",
            "on-track",
            "--yes",
        ],
        &[],
    );
    run.success();
    assert!(!run.stdout.contains("(y/N)"), "{run}");
    assert_eq!(api.operations(), ["CreateProjectUpdate"]);
}
