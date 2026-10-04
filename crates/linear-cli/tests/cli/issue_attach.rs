//! `issue attach`, `issue link`, `issue relation` and `issue agent-session`.
use serde_json::{Value, json};

use crate::issue_comment::file_upload;
use crate::support::{Cli, MockLinear, nodes};

const ISSUE_1: &str = "00000000-0000-4000-9000-000000000001";
const ISSUE_2: &str = "00000000-0000-4000-9000-000000000002";
const SESSION_ID: &str = "00000000-0000-4000-9000-000000000065";

fn issue_id(id: &str) -> Value {
    json!({ "issue": { "id": id } })
}

fn attachment_created(title: &str) -> Value {
    json!({
        "attachmentCreate": {
            "success": true,
            "attachment": {
                "id": "attachment-1",
                "url": "https://uploads.linear.app/acme/notes.txt",
                "title": title
            }
        }
    })
}

#[test]
fn attach_uploads_the_file_and_creates_an_attachment() {
    let api = MockLinear::start();
    api.on("GetIssueId", issue_id(ISSUE_1))
        .on(
            "FileUpload",
            file_upload(&api, "notes.txt", "/signed/notes.txt"),
        )
        .on_http("PUT", "/signed/notes.txt", 200, b"")
        .on("AttachmentCreate", attachment_created("notes.txt"));
    Cli::for_api(&api)
        .file("cwd/notes.txt", "some notes\n")
        .run(&["issue", "attach", "eng-1", "notes.txt"])
        .success()
        .stdout_has(
            "✓ Attached file notes.txt to issue ENG-1\nhttps://uploads.linear.app/acme/notes.txt\n",
        );
    assert_eq!(api.variables("GetIssueId"), json!({ "id": "ENG-1" }));
    assert_eq!(
        api.variables("FileUpload"),
        json!({ "contentType": "text/plain", "filename": "notes.txt", "size": 11, "makePublic": false })
    );
    let put = api
        .requests()
        .into_iter()
        .find(|request| request.method == "PUT")
        .expect("signed upload");
    assert_eq!(put.body, b"some notes\n");
    assert_eq!(
        api.variables("AttachmentCreate"),
        json!({
            "input": {
                "issueId": ISSUE_1,
                "title": "notes.txt",
                "url": "https://uploads.linear.app/acme/notes.txt"
            }
        })
    );
}

#[test]
fn attach_sends_title_comment_and_public_flag() {
    let api = MockLinear::start();
    api.on("GetIssueId", issue_id(ISSUE_1))
        .on(
            "FileUpload",
            file_upload(&api, "shot.png", "/signed/shot.png"),
        )
        .on_http("PUT", "/signed/shot.png", 200, b"")
        .on("AttachmentCreate", attachment_created("Screenshot"));
    Cli::for_api(&api)
        .file("cwd/shot.png", "png-bytes")
        .run(&[
            "issue",
            "attach",
            "ENG-1",
            "shot.png",
            "--title",
            "Screenshot",
            "--comment",
            "See the screenshot",
            "--public",
        ])
        .success()
        .stdout_has("Screenshot");
    assert_eq!(api.variables("FileUpload")["makePublic"], true);
    assert_eq!(
        api.variables("AttachmentCreate"),
        json!({
            "input": {
                "issueId": ISSUE_1,
                "title": "Screenshot",
                "url": "https://uploads.linear.app/acme/shot.png",
                "commentBody": "See the screenshot"
            }
        })
    );
}

#[test]
fn attach_with_a_missing_file_fails_without_uploading() {
    let api = MockLinear::start();
    // The issue may or may not be looked up first; nothing must be uploaded.
    let run = Cli::for_api(&api).run(&["issue", "attach", "ENG-1", "missing.txt"]);
    run.failure();
    assert!(
        api.operations()
            .iter()
            .all(|operation| operation == "GetIssueId"),
        "{:?}",
        api.operations()
    );
}

fn linked(title: &str) -> Value {
    json!({
        "attachmentLinkURL": {
            "success": true,
            "attachment": { "id": "attachment-1", "title": title, "url": "https://example.com/a" }
        }
    })
}

#[test]
fn link_attaches_a_url_with_an_optional_title() {
    let api = MockLinear::start();
    api.on("GetIssueId", issue_id(ISSUE_1))
        .on("AttachmentLinkURL", linked("Design doc"));
    Cli::for_api(&api)
        .run(&[
            "issue",
            "link",
            "eng-1",
            "https://example.com/a",
            "-t",
            "Design doc",
        ])
        .success()
        .stdout_has("✓ Linked issue ENG-1 to Design doc\nhttps://example.com/a\n");
    assert_eq!(api.variables("GetIssueId"), json!({ "id": "ENG-1" }));
    assert_eq!(
        api.variables("AttachmentLinkURL"),
        json!({ "issueId": ISSUE_1, "url": "https://example.com/a", "title": "Design doc" })
    );
}

#[test]
fn link_without_title_omits_it() {
    let api = MockLinear::start();
    api.on("GetIssueId", issue_id(ISSUE_1))
        .on("AttachmentLinkURL", linked("example.com"));
    Cli::for_api(&api)
        .run(&["issue", "link", "ENG-1", "https://example.com/a"])
        .success();
    assert_eq!(
        api.variables("AttachmentLinkURL"),
        json!({ "issueId": ISSUE_1, "url": "https://example.com/a" })
    );
}

#[test]
fn link_to_a_missing_issue_fails() {
    let api = MockLinear::start();
    api.on_error("GetIssueId", "Entity not found: Issue");
    Cli::for_api(&api)
        .run(&["issue", "link", "ENG-1", "https://example.com/a"])
        .failure()
        .stderr_has("ENG-1");
}

#[test]
fn relation_add_creates_the_relation_in_the_right_direction() {
    let api = MockLinear::start();
    let created = json!({
        "issueRelationCreate": { "success": true, "issueRelation": { "id": "relation-1" } }
    });
    api.on("GetIssueId", issue_id(ISSUE_1))
        .on("GetIssueId", issue_id(ISSUE_2))
        .on("CreateIssueRelation", created.clone());
    let cli = Cli::for_api(&api);
    cli.run(&["issue", "relation", "add", "eng-1", "blocks", "eng-2"])
        .success()
        .stdout_has("✓ Created relation ENG-1 blocks ENG-2\n");
    assert_eq!(
        api.variables("CreateIssueRelation"),
        json!({ "input": { "issueId": ISSUE_1, "relatedIssueId": ISSUE_2, "type": "blocks" } })
    );

    // `A blocked-by B` is stored as `B blocks A`.
    let api = MockLinear::start();
    api.on("GetIssueId", issue_id(ISSUE_1))
        .on("GetIssueId", issue_id(ISSUE_2))
        .on("CreateIssueRelation", created);
    Cli::for_api(&api)
        .run(&["issue", "relation", "add", "ENG-1", "blocked-by", "ENG-2"])
        .success();
    assert_eq!(
        api.variables("CreateIssueRelation"),
        json!({ "input": { "issueId": ISSUE_2, "relatedIssueId": ISSUE_1, "type": "blocks" } })
    );
}

#[test]
fn relation_add_rejects_an_unknown_type_before_any_request() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&["issue", "relation", "add", "ENG-1", "parent-of", "ENG-2"])
        .usage_error();
    assert!(api.requests().is_empty());
}

fn relations_of(kind: &str, related: &str) -> Value {
    json!({
        "issue": {
            "relations": {
                "nodes": [{ "id": "relation-1", "type": kind, "relatedIssue": { "id": related } }]
            }
        }
    })
}

#[test]
fn relation_delete_asks_first_and_defaults_to_no() {
    let api = MockLinear::start();
    api.on("GetIssueId", issue_id(ISSUE_1))
        .on("GetIssueId", issue_id(ISSUE_2))
        .on("FindIssueRelation", relations_of("blocks", ISSUE_2));
    Cli::for_api(&api)
        .run_tty(
            &["issue", "relation", "delete", "ENG-1", "blocks", "ENG-2"],
            &[("delete the relation ENG-1 blocks ENG-2? (y/N)", "\r")],
        )
        .success()
        .stdout_has("Canceled.");
    assert!(!api.operations().contains(&"DeleteIssueRelation".to_owned()));
    let api = MockLinear::start();
    api.on("GetIssueId", issue_id(ISSUE_1))
        .on("GetIssueId", issue_id(ISSUE_2))
        .on("FindIssueRelation", relations_of("blocks", ISSUE_2));
    Cli::for_api(&api)
        .run(&["issue", "relation", "delete", "ENG-1", "blocks", "ENG-2"])
        .failure()
        .stderr_has("--yes");
    assert!(!api.operations().contains(&"DeleteIssueRelation".to_owned()));
}

#[test]
fn relation_delete_finds_and_deletes_the_matching_relation() {
    let api = MockLinear::start();
    api.on("GetIssueId", issue_id(ISSUE_1))
        .on("GetIssueId", issue_id(ISSUE_2))
        .on("FindIssueRelation", relations_of("blocks", ISSUE_2))
        .on(
            "DeleteIssueRelation",
            json!({ "issueRelationDelete": { "success": true } }),
        );
    Cli::for_api(&api)
        .run(&[
            "issue", "relation", "delete", "ENG-1", "blocks", "ENG-2", "--yes",
        ])
        .success()
        .stdout_has("✓ Deleted relation ENG-1 blocks ENG-2\n");
    assert_eq!(
        api.variables("FindIssueRelation"),
        json!({ "issueId": ISSUE_1 })
    );
    assert_eq!(
        api.variables("DeleteIssueRelation"),
        json!({ "id": "relation-1" })
    );
}

#[test]
fn relation_delete_fails_when_no_relation_matches() {
    let api = MockLinear::start();
    api.on("GetIssueId", issue_id(ISSUE_1))
        .on("GetIssueId", issue_id(ISSUE_2))
        .on("FindIssueRelation", relations_of("related", ISSUE_2));
    Cli::for_api(&api)
        .run(&[
            "issue", "relation", "delete", "ENG-1", "blocks", "ENG-2", "--yes",
        ])
        .failure()
        .stderr_has("not found");
}

#[test]
fn relation_list_shows_outgoing_and_incoming_relations() {
    let api = MockLinear::start();
    api.on(
        "ListIssueRelations",
        json!({
            "issue": {
                "identifier": "ENG-1", "title": "Main issue",
                "relations": { "nodes": [
                    { "id": "r1", "type": "blocks", "relatedIssue": { "identifier": "ENG-2", "title": "Blocked one" } }
                ] },
                "inverseRelations": { "nodes": [
                    { "id": "r2", "type": "related", "issue": { "identifier": "ENG-3", "title": "Related one" } }
                ] }
            }
        }),
    );
    Cli::for_api(&api)
        .run(&["issue", "relation", "list", "eng-1"])
        .success()
        .stdout_has("Main issue")
        .stdout_has("ENG-2")
        .stdout_has("Blocked one")
        .stdout_has("ENG-3");
    assert_eq!(
        api.variables("ListIssueRelations"),
        json!({ "issueId": "ENG-1" })
    );
}

fn session(id: &str, status: &str) -> Value {
    json!({
        "id": id, "status": status, "type": "commentThread",
        "createdAt": "2026-01-02T03:04:05.000Z", "startedAt": null, "endedAt": null,
        "summary": format!("Session {id}"),
        "creator": { "name": "Alice" }, "appUser": { "name": "Agent" }
    })
}

fn sessions_reply() -> Value {
    json!({
        "issue": {
            "comments": {
                "nodes": [
                    { "agentSession": null },
                    { "agentSession": session("s1", "active") },
                    { "agentSession": session("s2", "complete") }
                ],
                "pageInfo": { "hasNextPage": false, "endCursor": null }
            }
        }
    })
}

/// Sessions in list output, whether printed bare or wrapped as `{agentSession}` comment nodes
/// (null wrappers for comments without a session are skipped).
fn sessions(listed: &[Value]) -> impl Iterator<Item = &Value> {
    listed
        .iter()
        .map(|node| node.get("agentSession").unwrap_or(node))
        .filter(|session| !session.is_null())
}

#[test]
fn agent_session_list_json_lists_the_sessions() {
    let api = MockLinear::start();
    api.on("GetIssueAgentSessions", sessions_reply());
    let listed = Cli::for_api(&api)
        .run(&["issue", "agent-session", "list", "eng-1", "--json"])
        .success()
        .json_nodes();
    let ids: Vec<Value> = sessions(&listed).map(|s| s["id"].clone()).collect();
    assert_eq!(ids, ["s1", "s2"]);
    assert_eq!(
        api.variables("GetIssueAgentSessions"),
        json!({ "issueId": "ENG-1", "first": 100 })
    );
}

#[test]
fn agent_session_list_filters_by_status() {
    let api = MockLinear::start();
    api.on("GetIssueAgentSessions", sessions_reply());
    let listed = Cli::for_api(&api)
        .run(&[
            "issue",
            "agent-session",
            "list",
            "ENG-1",
            "--json",
            "--status",
            "complete",
        ])
        .success()
        .json_nodes();
    let listed: Vec<&Value> = sessions(&listed).collect();
    assert_eq!(listed, [&session("s2", "complete")]);
}

#[test]
fn agent_session_list_text_names_sessions() {
    let api = MockLinear::start();
    api.on("GetIssueAgentSessions", sessions_reply());
    Cli::for_api(&api)
        .run(&["issue", "agent-session", "list", "ENG-1"])
        .success()
        .stdout_has("s1")
        .stdout_has("Agent");
}

fn session_details() -> Value {
    json!({
        "id": SESSION_ID, "status": "awaitingInput", "type": "commentThread",
        "createdAt": "2026-01-02T03:04:05.000Z", "updatedAt": "2026-01-02T03:04:05.000Z",
        "startedAt": "2026-01-02T03:04:05.000Z", "endedAt": null, "dismissedAt": null,
        "summary": "Investigating the bug", "externalLink": "https://example.com/session",
        "creator": { "name": "Alice" }, "appUser": { "name": "Agent" }, "dismissedBy": null,
        "issue": { "identifier": "ENG-1", "title": "Issue title", "url": "https://linear.app/acme/issue/ENG-1" },
        "activities": { "nodes": [
            { "id": "a1", "createdAt": "2026-01-02T03:04:05.000Z",
              "content": { "type": "thought", "body": "Thinking hard" } },
            { "id": "a2", "createdAt": "2026-01-02T03:04:05.000Z",
              "content": { "type": "action", "action": "run", "parameter": "tests", "result": "ok" } }
        ], "pageInfo": { "hasNextPage": false, "endCursor": null } }
    })
}

fn with_typenames(mut details: Value) -> Value {
    for (node, typename) in details["activities"]["nodes"]
        .as_array_mut()
        .expect("activity nodes")
        .iter_mut()
        .zip(["AgentActivityThoughtContent", "AgentActivityActionContent"])
    {
        node["content"]["__typename"] = json!(typename);
    }
    details
}

#[test]
fn agent_session_view_json_prints_the_session() {
    let api = MockLinear::start();
    api.on(
        "GetAgentSessionDetails",
        json!({ "agentSession": with_typenames(session_details()) }),
    );
    let json = Cli::for_api(&api)
        .run(&["issue", "agent-session", "view", SESSION_ID, "--json"])
        .success()
        .json();
    assert_eq!(json["id"], SESSION_ID);
    assert_eq!(json["status"], "awaitingInput");
    assert_eq!(json["issue"]["identifier"], "ENG-1");
    assert_eq!(
        nodes(&json["activities"])[0]["content"]["body"],
        "Thinking hard"
    );
    assert_eq!(nodes(&json["activities"])[1]["content"]["action"], "run");
    assert_eq!(
        api.variables("GetAgentSessionDetails"),
        json!({ "id": SESSION_ID, "first": 100, "after": null })
    );
}

#[test]
fn agent_session_view_follows_activity_pages() {
    let api = MockLinear::start();
    let mut first = with_typenames(session_details());
    let second_activity = first["activities"]["nodes"][1].take();
    first["activities"]["nodes"] = json!([first["activities"]["nodes"][0].take()]);
    first["activities"]["pageInfo"] = json!({ "hasNextPage": true, "endCursor": "c1" });
    let mut second = first.clone();
    second["activities"] = json!({
        "nodes": [second_activity],
        "pageInfo": { "hasNextPage": false, "endCursor": "c2" }
    });
    api.on("GetAgentSessionDetails", json!({ "agentSession": first }))
        .on("GetAgentSessionDetails", json!({ "agentSession": second }));
    let json = Cli::for_api(&api)
        .run(&["issue", "agent-session", "view", SESSION_ID, "--json"])
        .success()
        .json();
    assert_eq!(nodes(&json["activities"]).len(), 2);
    let afters: Vec<Value> = api
        .requests()
        .into_iter()
        .map(|r| r.variables["after"].clone())
        .collect();
    assert_eq!(afters, [Value::Null, json!("c1")]);
}

#[test]
fn agent_session_view_text_shows_summary_and_issue() {
    let api = MockLinear::start();
    api.on(
        "GetAgentSessionDetails",
        json!({ "agentSession": with_typenames(session_details()) }),
    );
    Cli::for_api(&api)
        .run(&["issue", "agent-session", "view", SESSION_ID])
        .success()
        .stdout_has("Investigating the bug")
        .stdout_has("ENG-1");
}

#[test]
fn agent_session_view_of_a_missing_session_fails() {
    let api = MockLinear::start();
    api.on("GetAgentSessionDetails", json!({ "agentSession": null }));
    Cli::for_api(&api)
        .run(&["issue", "agent-session", "view", SESSION_ID])
        .failure();
}

#[test]
fn relation_add_reports_uncertain_outcomes_without_retrying() {
    for uncertain in [false, true] {
        let api = MockLinear::start();
        api.on("GetIssueId", issue_id(ISSUE_1))
            .on("GetIssueId", issue_id(ISSUE_2));
        if uncertain {
            api.on_raw("CreateIssueRelation", 200, "not json");
        } else {
            api.on_error("CreateIssueRelation", "Relation rejected");
        }
        let run = Cli::for_api(&api).run(&["issue", "relation", "add", "ENG-1", "blocks", "ENG-2"]);
        run.failure().stderr_has(if uncertain {
            "relation may already exist"
        } else {
            "Relation rejected"
        });
        assert_eq!(run.stderr.contains("may already exist"), uncertain);
        assert_eq!(
            api.operations(),
            ["GetIssueId", "GetIssueId", "CreateIssueRelation"]
        );
        assert_eq!(
            api.variables("CreateIssueRelation"),
            json!({"input": {"issueId": ISSUE_1, "relatedIssueId": ISSUE_2, "type": "blocks"}})
        );
    }
}

#[test]
fn attach_reports_uncertain_outcomes_without_retrying() {
    for uncertain in [false, true] {
        let api = MockLinear::start();
        api.on("GetIssueId", issue_id(ISSUE_1))
            .on(
                "FileUpload",
                file_upload(&api, "notes.txt", "/signed/notes.txt"),
            )
            .on_http("PUT", "/signed/notes.txt", 200, b"");
        if uncertain {
            api.on_raw("AttachmentCreate", 200, "not json");
        } else {
            api.on_error("AttachmentCreate", "Attachment rejected");
        }
        let run = Cli::for_api(&api)
            .file("cwd/notes.txt", "some notes\n")
            .run(&["issue", "attach", "ENG-1", "notes.txt"]);
        run.failure().stderr_has(if uncertain {
            "attachment may already exist"
        } else {
            "Attachment rejected"
        });
        assert_eq!(run.stderr.contains("may already exist"), uncertain);
        assert_eq!(
            api.operations(),
            ["GetIssueId", "FileUpload", "", "AttachmentCreate"]
        );
        assert_eq!(
            api.variables("AttachmentCreate"),
            json!({"input": {"issueId": ISSUE_1, "title": "notes.txt", "url": "https://uploads.linear.app/acme/notes.txt"}})
        );
    }
}
