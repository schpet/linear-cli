//! The `project` command group.
use serde_json::{Value, json};

use crate::support::{Cli, MockLinear, assert_json};

const ID: &str = "85d3dad6-136e-49ff-9593-33dc4b22b5ee";
const TEAM_ID: &str = "00000000-0000-4000-9000-000000002501";

fn page(nodes: Value) -> Value {
    json!({ "nodes": nodes, "pageInfo": { "hasNextPage": false, "endCursor": null } })
}

fn team(key: &str, id: &str) -> Value {
    json!({ "teams": { "nodes": [{ "id": id, "key": key, "name": format!("Team {key}") }] } })
}

fn ids(id: &str) -> Value {
    json!({ "projects": { "nodes": [{ "id": id }] } })
}

fn no_ids() -> Value {
    json!({ "projects": { "nodes": [] } })
}

fn list_node(name: &str) -> Value {
    json!({
        "id": "project-1", "name": name, "description": "Short description",
        "slugId": "roadmap-abc123", "icon": null, "color": "#cccccc", "sortOrder": 1,
        "status": { "id": "status-1", "name": "Started", "color": "#abc", "type": "started" },
        "lead": { "name": "Ada Lovelace", "displayName": "ada", "initials": "AL" },
        "priority": 2, "health": "onTrack", "startDate": "2024-02-01", "targetDate": null,
        "startedAt": null, "completedAt": null, "canceledAt": null,
        "createdAt": "2024-01-01T00:00:00.000Z", "updatedAt": "2024-01-02T00:00:00.000Z",
        "url": "https://linear.app/acme/project/roadmap-abc123",
        "teams": { "nodes": [{ "key": "ENG" }] }
    })
}

fn details() -> Value {
    let empty = page(json!([]));
    json!({
        "id": ID, "name": "Simple Project", "identifier": null,
        "description": "One line summary", "content": "## Overview\n\nThe plan.",
        "slugId": "simple-abc123def456", "icon": null, "color": "#64748b",
        "progress": 0.5, "scope": 4, "url": "https://linear.app/acme/project/simple-abc123def456",
        "priority": 0, "health": null, "healthUpdatedAt": null,
        "startDate": null, "startDateResolution": null,
        "targetDate": null, "targetDateResolution": null,
        "startedAt": null, "completedAt": null, "canceledAt": null,
        "archivedAt": null, "autoArchivedAt": null,
        "createdAt": "2024-01-20T12:00:00Z", "updatedAt": "2024-01-20T12:00:00Z",
        "status": { "id": "s1", "name": "Backlog", "color": "#94a3b8", "type": "backlog", "position": 0 },
        "creator": null,
        "lead": { "id": "user-1", "name": "ada", "displayName": "Ada Lovelace" },
        "teams": page(json!([{ "id": TEAM_ID, "key": "ENG", "name": "Engineering" }])),
        "labels": empty, "members": empty, "initiatives": empty, "projectMilestones": empty,
        "externalLinks": empty, "documents": empty, "attachments": empty, "relations": empty,
        "inverseRelations": empty, "issues": empty, "lastUpdate": null
    })
}

fn created() -> Value {
    json!({
        "projectCreate": {
            "success": true,
            "project": {
                "id": ID, "slugId": "fixture-project", "name": "Fixture project",
                "url": "https://linear.app/acme/project/fixture-project"
            }
        }
    })
}

fn updated() -> Value {
    json!({
        "projectUpdate": {
            "success": true,
            "project": {
                "id": ID, "slugId": "fixture-project", "name": "Fixture project",
                "description": "returned", "url": "https://linear.app/acme/project/fixture-project",
                "updatedAt": "2026-09-30T00:00:00Z"
            }
        }
    })
}

fn comment_created() -> Value {
    json!({
        "commentCreate": {
            "success": true,
            "comment": { "id": "comment-1", "url": "https://linear.app/acme/comment/c0de" }
        }
    })
}

fn comment(id: &str, body: &str, parent: Option<&str>) -> Value {
    json!({
        "id": id, "body": body, "quotedText": null,
        "createdAt": "2020-01-02T12:00:00Z", "updatedAt": "2020-02-01T00:00:00Z", "editedAt": null,
        "url": format!("https://linear.app/acme/project/simple/activity#{id}"),
        "user": { "id": "user-1", "name": "ada", "displayName": "Ada Lovelace" },
        "externalUser": null, "botActor": null,
        "parent": parent.map(|id| json!({ "id": id }))
    })
}

#[test]
fn list_json_filters_by_team_and_status() {
    let api = MockLinear::start();
    api.on("ResolveTeam", team("ENG", TEAM_ID)).on(
        "GetProjects",
        json!({ "projects": page(json!([list_node("Roadmap")])) }),
    );
    let run = Cli::for_api(&api).run(&[
        "project", "list", "--team", "ENG", "--status", "Started", "--json",
    ]);
    assert_json(
        &Value::Array(run.success().json_nodes()),
        &json!([list_node("Roadmap")]),
    );
    assert_eq!(api.variables("ResolveTeam")["reference"], "ENG");
    assert_eq!(
        api.variables("GetProjects"),
        json!({
            "filter": {
                "accessibleTeams": { "some": { "key": { "eq": "ENG" } } },
                "status": { "name": { "eq": "Started" } }
            },
            "first": 100
        })
    );
}

#[test]
fn list_defaults_to_the_configured_team() {
    let api = MockLinear::start();
    api.on("GetProjects", json!({ "projects": page(json!([])) }));
    Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .run(&["project", "list", "--json"])
        .success();
    assert_eq!(
        api.variables("GetProjects"),
        json!({ "filter": { "accessibleTeams": { "some": { "key": { "eq": "ENG" } } } }, "first": 100 })
    );
}

#[test]
fn list_all_teams_follows_pages() {
    let api = MockLinear::start();
    let first = json!({
        "projects": {
            "nodes": [list_node("First")],
            "pageInfo": { "hasNextPage": true, "endCursor": "cursor-1" }
        }
    });
    api.on("GetProjects", first).on(
        "GetProjects",
        json!({ "projects": page(json!([list_node("Second")])) }),
    );
    let run =
        Cli::for_api(&api)
            .env("LINEAR_TEAM_ID", "ENG")
            .run(&["project", "list", "--all-teams"]);
    run.success().stdout_has("First").stdout_has("Second");
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
fn view_json_by_id_returns_the_project() {
    let api = MockLinear::start();
    api.on("GetProjectDetails", json!({ "project": details() }));
    let run = Cli::for_api(&api).run(&["project", "view", ID, "--json"]);
    assert_json(&run.success().json(), &details());
    assert_eq!(
        api.variables("GetProjectDetails"),
        json!({ "id": ID, "first": 250 })
    );
}

#[test]
fn view_resolves_urls_by_slug_and_shows_details() {
    let api = MockLinear::start();
    api.on("GetProjectIdBySlugId", ids(ID))
        .on("GetProjectDetails", json!({ "project": details() }));
    Cli::for_api(&api)
        .run(&[
            "project",
            "view",
            "https://linear.app/acme/project/simple-abc123def456",
            "--no-pager",
        ])
        .success()
        .stdout_has("Simple Project")
        .stdout_has("The plan.");
    assert_eq!(
        api.variables("GetProjectIdBySlugId"),
        json!({ "slugId": "abc123def456" })
    );
    assert_eq!(api.variables("GetProjectDetails")["id"], ID);
}

#[test]
fn view_resolves_names_then_slugs() {
    let api = MockLinear::start();
    api.on("GetProjectIdByName", no_ids())
        .on("GetProjectIdBySlugId", ids(ID))
        .on("GetProjectDetails", json!({ "project": details() }));
    Cli::for_api(&api)
        .run(&["project", "view", "abc123def456", "--json"])
        .success();
    assert_eq!(
        api.operations(),
        [
            "GetProjectIdByName",
            "GetProjectIdBySlugId",
            "GetProjectDetails"
        ]
    );
    assert_eq!(
        api.variables("GetProjectIdByName"),
        json!({ "name": "abc123def456" })
    );
}

#[test]
fn view_unknown_project_fails() {
    let api = MockLinear::start();
    api.on("GetProjectIdByName", no_ids())
        .on("GetProjectIdBySlugId", no_ids());
    Cli::for_api(&api)
        .run(&["project", "view", "nosuchproject", "--json"])
        .failure()
        .stderr_has("nosuchproject");
}

#[test]
fn create_sends_input_and_prints_json() {
    let api = MockLinear::start();
    api.on("ResolveTeam", team("SRC", TEAM_ID))
        .on("CreateProject", created());
    let run = Cli::for_api(&api).run(&[
        "project",
        "create",
        "--name",
        "New",
        "--team",
        "SRC",
        "--description",
        "Short",
        "--content",
        "# Overview",
        "--json",
    ]);
    assert_json(
        &run.success().json(),
        &created()["projectCreate"]["project"],
    );
    assert_eq!(api.variables("ResolveTeam")["reference"], "SRC");
    assert_eq!(
        api.variables("CreateProject"),
        json!({
            "input": {
                "name": "New", "teamIds": [TEAM_ID],
                "description": "Short", "content": "# Overview"
            }
        })
    );
}

#[test]
fn create_reads_bodies_from_files_and_reports_the_url() {
    let api = MockLinear::start();
    api.on("ResolveTeam", team("SRC", TEAM_ID))
        .on("CreateProject", created());
    Cli::for_api(&api)
        .file("cwd/description.txt", "From a file")
        .file("cwd/content.md", "## Plan\n")
        .run(&[
            "project",
            "create",
            "-n",
            "Files",
            "-t",
            "SRC",
            "--description-file",
            "description.txt",
            "--content-file",
            "content.md",
        ])
        .success()
        .stdout_has("Fixture project")
        .stdout_has("https://linear.app/acme/project/fixture-project");
    let input = &api.variables("CreateProject")["input"];
    assert_eq!(input["description"], "From a file");
    assert_eq!(input["content"], "## Plan\n");
}

#[test]
fn create_counts_the_description_limit_in_utf16_units() {
    let api = MockLinear::start();
    // 128 characters, but 256 UTF-16 units: over Linear's 255 limit.
    let description = "🚀".repeat(128);
    Cli::for_api(&api)
        .run(&[
            "project",
            "create",
            "-n",
            "X",
            "-t",
            "SRC",
            "--description",
            &description,
        ])
        .failure()
        .stderr_has("over Linear's limit of 255");
    assert!(api.requests().is_empty());
}

#[test]
fn create_adds_the_project_to_an_initiative() {
    let initiative = "00000000-0000-4000-9000-000000002509";
    let api = MockLinear::start();
    api.on("ResolveTeam", team("SRC", TEAM_ID))
        .on("CreateProject", created())
        .on(
            "AddProjectToInitiative",
            json!({ "initiativeToProjectCreate": { "success": true } }),
        );
    Cli::for_api(&api)
        .run(&[
            "project",
            "create",
            "-n",
            "X",
            "-t",
            "SRC",
            "--initiative",
            initiative,
        ])
        .success();
    assert_eq!(
        api.variables("AddProjectToInitiative"),
        json!({ "input": { "initiativeId": initiative, "projectId": ID } })
    );
}

#[test]
fn create_requires_a_name_and_team_before_any_request() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api);
    cli.run(&["project", "create", "--team", "SRC"])
        .failure()
        .stderr_has("--name");
    cli.run(&["project", "create", "--name", "New"])
        .failure()
        .stderr_has("--team");
    assert!(api.requests().is_empty());
}

#[test]
fn update_sends_fields_and_clears() {
    let api = MockLinear::start();
    api.on("UpdateProject", updated());
    Cli::for_api(&api)
        .file("cwd/content.md", "Overview")
        .run(&[
            "project",
            "update",
            ID,
            "--name",
            "Renamed",
            "--description",
            "New summary",
            "--content-file",
            "content.md",
            "--clear-lead",
            "--clear-start-date",
            "--target-date",
            "2025-03-01",
        ])
        .success()
        .stdout_has("Fixture project");
    assert_eq!(
        api.variables("UpdateProject"),
        json!({
            "id": ID,
            "input": {
                "name": "Renamed", "description": "New summary", "content": "Overview",
                "startDate": null, "targetDate": "2025-03-01", "leadId": null
            }
        })
    );
}

#[test]
fn update_changes_teams_incrementally() {
    let project = "00000000-0000-4000-9000-000000002508";
    let api = MockLinear::start();
    api.on("ResolveTeam", team("NEW", "team-new"))
        .on("ResolveTeam", team("SRC", "team-src"))
        .on(
            "GetProjectTeamsForUpdate",
            json!({
                "project": {
                    "teams": {
                        "nodes": [
                            { "id": "team-src", "key": "SRC", "name": "Team SRC" },
                            { "id": "team-other", "key": "OTHER", "name": "Team OTHER" }
                        ],
                        "pageInfo": { "hasNextPage": false, "endCursor": null }
                    }
                }
            }),
        )
        .on("UpdateProject", updated());
    Cli::for_api(&api)
        .run(&[
            "project",
            "update",
            project,
            "--add-team",
            "NEW",
            "--remove-team",
            "SRC",
        ])
        .success();
    assert_eq!(
        api.variables("UpdateProject"),
        json!({ "id": project, "input": { "teamIds": ["team-other", "team-new"] } })
    );
}

#[test]
fn update_rejects_conflicting_flags_before_any_request() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api);
    let run = cli.run(&["project", "update", ID, "--lead", "@me", "--clear-lead"]);
    assert_ne!(run.code, 0, "{run}");
    let run = cli.run(&[
        "project",
        "update",
        ID,
        "--team",
        "ENG",
        "--add-team",
        "OPS",
    ]);
    assert_ne!(run.code, 0, "{run}");
    assert!(api.requests().is_empty());
}

#[test]
fn delete_with_force_deletes_by_id_or_url() {
    let deleted = json!({ "projectDelete": { "success": true, "entity": { "id": ID, "name": "Mobile App" } } });
    let api = MockLinear::start();
    api.on("DeleteProject", deleted.clone())
        .on("GetProjectIdBySlugId", ids(ID))
        .on("DeleteProject", deleted);
    let cli = Cli::for_api(&api);
    cli.run(&["project", "delete", ID, "--force"])
        .success()
        .stdout_has("Mobile App");
    cli.run(&[
        "project",
        "delete",
        "https://linear.app/acme/project/mobile-app-abc123def456",
        "--force",
    ])
    .success();
    let deletes: Vec<Value> = api
        .requests()
        .into_iter()
        .filter(|r| r.operation.as_deref() == Some("DeleteProject"))
        .map(|r| r.variables)
        .collect();
    assert_eq!(deletes, [json!({ "id": ID }), json!({ "id": ID })]);
    assert_eq!(
        api.variables("GetProjectIdBySlugId"),
        json!({ "slugId": "abc123def456" })
    );
}

#[test]
fn delete_without_confirmation_does_not_delete() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&["project", "delete", ID])
        .failure()
        .stderr_has("--force");
    assert!(!api.operations().contains(&"DeleteProject".to_owned()));
}

#[test]
fn comment_add_resolves_the_project_and_posts_the_body() {
    let api = MockLinear::start();
    api.on("GetProjectIdByName", ids(ID))
        .on("AddComment", comment_created());
    Cli::for_api(&api)
        .run(&["project", "comment", "add", "Roadmap", "--body", "Hello"])
        .success()
        .stdout_has("https://linear.app/acme/comment/c0de");
    assert_eq!(
        api.variables("GetProjectIdByName"),
        json!({ "name": "Roadmap" })
    );
    assert_eq!(
        api.variables("AddComment"),
        json!({ "input": { "body": "Hello", "projectId": ID } })
    );
}

#[test]
fn comment_add_replies_with_a_body_file() {
    let api = MockLinear::start();
    api.on("AddComment", comment_created());
    Cli::for_api(&api)
        .file("cwd/reply.md", "**Bold** reply\n")
        .run(&[
            "project",
            "comment",
            "add",
            ID,
            "--body-file",
            "reply.md",
            "--parent",
            "comment-0",
        ])
        .success();
    let input = &api.variables("AddComment")["input"];
    assert_eq!(input["projectId"], ID);
    assert_eq!(input["parentId"], "comment-0");
    assert_eq!(
        input["body"].as_str().map(str::trim_end),
        Some("**Bold** reply")
    );
}

#[test]
fn comment_add_reports_a_comment_linear_did_not_create() {
    let api = MockLinear::start();
    api.on(
        "AddComment",
        json!({ "commentCreate": {
            "success": false,
            "comment": { "id": "comment-1", "url": "https://linear.app/acme/comment/c0de" }
        } }),
    );
    Cli::for_api(&api)
        .run(&["project", "comment", "add", ID, "--body", "Hi"])
        .failure()
        .stderr_has("Failed to create comment");
}

#[test]
fn comment_add_without_a_returned_comment_is_not_retried() {
    let api = MockLinear::start();
    api.on(
        "AddComment",
        json!({ "commentCreate": { "success": true, "comment": null } }),
    );
    Cli::for_api(&api)
        .run(&["project", "comment", "add", ID, "--body", "Hi"])
        .failure()
        .stderr_has("comment may already exist");
    assert_eq!(api.operations(), ["AddComment"]);
}

#[test]
fn comment_list_json_returns_comments() {
    let comments = json!([
        comment("comment-1", "Root A", None),
        comment("comment-2", "Reply B", Some("comment-1"))
    ]);
    let api = MockLinear::start();
    api.on(
        "GetProjectComments",
        json!({
            "project": { "id": ID, "name": "Mobile App" },
            "comments": { "nodes": comments, "pageInfo": { "hasNextPage": false, "endCursor": "end" } }
        }),
    );
    let run = Cli::for_api(&api).run(&["project", "comment", "list", ID, "--json"]);
    assert_eq!(Value::Array(run.success().json_nodes()), comments);
    assert_eq!(
        api.variables("GetProjectComments"),
        json!({ "id": ID, "filterId": ID, "after": null })
    );
}

#[test]
fn comment_list_shows_threads() {
    let api = MockLinear::start();
    api.on(
        "GetProjectComments",
        json!({
            "project": { "id": ID, "name": "Mobile App" },
            "comments": page(json!([
                comment("comment-1", "Root A", None),
                comment("comment-2", "Reply B", Some("comment-1"))
            ]))
        }),
    );
    Cli::for_api(&api)
        .run(&["project", "comment", "list", ID])
        .success()
        .stdout_has("Root A")
        .stdout_has("Reply B");
}

#[test]
fn update_sets_priority_status_and_dates() {
    let api = MockLinear::start();
    api.on(
        "GetProjectStatuses",
        json!({ "projectStatuses": { "nodes": [
            { "id": "status-planned", "name": "Planned", "type": "planned" },
            { "id": "status-started", "name": "In Progress", "type": "started" }
        ] } }),
    )
    .on("UpdateProject", updated());
    Cli::for_api(&api)
        .run(&[
            "project",
            "update",
            ID,
            "--priority",
            "High",
            "--status",
            "in-progress",
            "--start-date",
            "2025-01-31",
        ])
        .success();
    assert_eq!(
        api.variables("UpdateProject"),
        json!({
            "id": ID,
            "input": { "priority": 2, "statusId": "status-started", "startDate": "2025-01-31" }
        })
    );
}

#[test]
fn create_and_update_reject_invalid_values_before_any_request() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api);
    for args in [
        &[
            "project",
            "create",
            "-n",
            "X",
            "-t",
            "SRC",
            "--start-date",
            "tomorrow",
        ][..],
        &[
            "project",
            "create",
            "-n",
            "X",
            "-t",
            "SRC",
            "--priority",
            "9",
        ],
        &[
            "project", "create", "-n", "X", "-t", "SRC", "--status", "done",
        ],
        &["project", "update", ID, "--priority", "extreme"],
        &["project", "update", ID, "--target-date", "2025-02-30"],
        &[
            "project",
            "update",
            ID,
            "--description",
            "a",
            "--description-file",
            "b",
        ],
    ] {
        cli.run(args).usage_error();
    }
    assert!(api.requests().is_empty());
}

#[test]
fn update_without_changes_fails_before_any_request() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&["project", "update", ID])
        .failure()
        .stderr_has("No changes");
    assert!(api.requests().is_empty());
}

#[test]
fn update_reports_partially_applied_initiative_links() {
    let first = "00000000-0000-4000-9000-00000000000a";
    let second = "00000000-0000-4000-9000-00000000000b";
    let initiative =
        |id: &str, name: &str| json!({ "initiatives": { "nodes": [{ "id": id, "name": name }] } });
    let api = MockLinear::start();
    api.on("GetInitiativeByIdForUpdate", initiative(first, "Alpha"))
        .on("GetInitiativeByIdForUpdate", initiative(second, "Beta"))
        .on(
            "GetProjectInitiativeLinksForUpdate",
            json!({ "project": {
                "id": ID, "name": "Mobile App", "url": "https://linear.app/acme/project/mobile",
                "initiativeToProjects": page(json!([]))
            } }),
        )
        .on(
            "AddProjectToInitiative",
            json!({ "initiativeToProjectCreate": { "success": true } }),
        )
        .on_error("AddProjectToInitiative", "Initiative is archived");
    let run = Cli::for_api(&api).run(&[
        "project",
        "update",
        ID,
        "--add-initiative",
        first,
        "--add-initiative",
        second,
    ]);
    run.failure()
        .stderr_has("after 1 of 2 changes")
        .stderr_has("Applied: added \"Alpha\"")
        .stderr_has("Initiative is archived")
        .stderr_has(&format!("--add-initiative {second}"));
    assert!(!api.operations().contains(&"UpdateProject".to_owned()));
}

#[test]
fn create_fails_before_creating_when_the_initiative_is_unknown() {
    let api = MockLinear::start();
    api.on("ResolveTeam", team("SRC", TEAM_ID))
        .on(
            "ResolveInitiativeBySlug",
            json!({ "initiatives": { "nodes": [] } }),
        )
        .on(
            "ResolveInitiativeByName",
            json!({ "initiatives": { "nodes": [] } }),
        );
    Cli::for_api(&api)
        .run(&[
            "project",
            "create",
            "-n",
            "X",
            "-t",
            "SRC",
            "--initiative",
            "Nope",
        ])
        .failure()
        .stderr_has("Nope");
    assert!(!api.operations().contains(&"CreateProject".to_owned()));
}

#[test]
fn create_reports_a_failed_initiative_link_after_creating() {
    let initiative = "00000000-0000-4000-9000-000000002509";
    let api = MockLinear::start();
    api.on("ResolveTeam", team("SRC", TEAM_ID))
        .on("CreateProject", created())
        .on_error("AddProjectToInitiative", "Initiative is archived");
    Cli::for_api(&api)
        .run(&[
            "project",
            "create",
            "-n",
            "X",
            "-t",
            "SRC",
            "--initiative",
            initiative,
        ])
        .failure()
        .stdout_has("Created project: Fixture project")
        .stderr_has("Initiative is archived")
        .stderr_has("--add-initiative");
}

#[test]
fn comment_add_without_a_body_needs_a_terminal_before_any_request() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&["project", "comment", "add", "Roadmap"])
        .failure()
        .stderr_has("--body");
    assert!(api.requests().is_empty());
}

#[test]
fn comment_list_follows_every_page() {
    let api = MockLinear::start();
    api.on(
        "GetProjectComments",
        json!({
            "project": { "id": ID, "name": "Mobile App" },
            "comments": {
                "nodes": [comment("comment-1", "Root A", None)],
                "pageInfo": { "hasNextPage": true, "endCursor": "cursor-1" }
            }
        }),
    )
    .on(
        "GetProjectComments",
        json!({
            "project": { "id": ID, "name": "Mobile App" },
            "comments": page(json!([comment("comment-2", "Root B", None)]))
        }),
    );
    Cli::for_api(&api)
        .run(&["project", "comment", "list", ID])
        .success()
        .stdout_has("Root A")
        .stdout_has("Root B");
    let afters: Vec<Value> = api
        .requests()
        .into_iter()
        .map(|request| request.variables["after"].clone())
        .collect();
    assert_eq!(afters, [Value::Null, json!("cursor-1")]);
}

#[test]
fn delete_resolves_names() {
    let api = MockLinear::start();
    api.on("GetProjectIdByName", ids(ID)).on(
        "DeleteProject",
        json!({ "projectDelete": { "success": true, "entity": { "id": ID, "name": "Mobile App" } } }),
    );
    Cli::for_api(&api)
        .run(&["project", "delete", "Mobile App", "--force"])
        .success()
        .stdout_has("Deleted project: Mobile App");
    assert_eq!(api.variables("DeleteProject"), json!({ "id": ID }));
}
