//! The `document` command group.
use serde_json::{Value, json};

use crate::support::{Cli, MockLinear, nodes};
use crate::team::{resolve_vars, resolved};

const DOC_ID: &str = "00000000-0000-4000-9000-000000000051";
const SLUG: &str = "d4b93e3b2695";
const PROJECT_ID: &str = "00000000-0000-4000-9000-000000000050";

fn page(nodes: Value, end_cursor: Value, has_next: bool) -> Value {
    json!({ "nodes": nodes, "pageInfo": { "hasNextPage": has_next, "endCursor": end_cursor } })
}

fn list_node(slug: &str, title: &str) -> Value {
    json!({
        "id": format!("doc-{slug}"), "title": title, "slugId": slug,
        "url": format!("https://linear.app/acme/document/{slug}"),
        "updatedAt": "2024-01-03T00:00:00.000Z",
        "project": { "name": "Roadmap", "slugId": "roadmap-slug" },
        "issue": null, "initiative": null, "team": null, "cycle": null, "release": null,
        "creator": { "name": "Ada" }
    })
}

fn document() -> Value {
    json!({
        "id": DOC_ID, "title": "Design notes", "slugId": SLUG,
        "content": "# Heading\n\nThe plan in **bold**.\n",
        "url": format!("https://linear.app/acme/document/design-notes-{SLUG}"),
        "createdAt": "2024-01-02T00:00:00.000Z", "updatedAt": "2024-01-03T00:00:00.000Z",
        "creator": { "name": "Ada", "email": "ada@example.com" },
        "project": { "name": "Roadmap", "slugId": "roadmap-slug" },
        "issue": null, "initiative": null, "team": null, "cycle": null, "release": null
    })
}

fn comment(id: &str, body: &str, parent: Option<&str>) -> Value {
    json!({
        "id": id, "body": body, "quotedText": null, "documentContentId": "content-1",
        "createdAt": "2024-01-02T00:00:00.000Z", "updatedAt": "2024-01-03T00:00:00.000Z",
        "archivedAt": null, "resolvedAt": null,
        "url": format!("https://linear.app/acme/comment/{id}"),
        "user": { "name": "Ada", "email": "ada@example.com" },
        "parent": parent.map(|id| json!({ "id": id }))
    })
}

fn written(operation: &str) -> Value {
    let key = match operation {
        "CreateDocument" => "documentCreate",
        "UpdateDocument" => "documentUpdate",
        other => panic!("not a document write: {other}"),
    };
    json!({ key: {
        "success": true,
        "document": {
            "id": DOC_ID, "slugId": SLUG, "title": "Server title",
            "url": format!("https://linear.app/acme/document/server-{SLUG}"),
            "updatedAt": "2024-01-04T00:00:00.000Z"
        }
    } })
}

fn guard(comments: Value) -> Value {
    json!({ "document": { "id": DOC_ID, "comments": page(comments, Value::Null, false) } })
}

#[test]
fn list_json_contains_documents() {
    let api = MockLinear::start();
    let nodes = json!([list_node("a1", "Alpha"), list_node("b2", "Beta")]);
    api.on(
        "ListDocuments",
        json!({ "documents": page(nodes.clone(), Value::Null, false) }),
    );
    let listed = Cli::for_api(&api)
        .run(&["document", "list", "--json"])
        .success()
        .json_nodes();
    assert_eq!(Value::Array(listed), nodes);
    assert_eq!(api.variables("ListDocuments"), json!({ "first": 50 }));
}

#[test]
fn list_filters_by_project_and_limit() {
    let api = MockLinear::start();
    api.on(
        "ListDocuments",
        json!({ "documents": page(json!([list_node("a1", "Alpha")]), Value::Null, false) }),
    );
    Cli::for_api(&api)
        .run(&["docs", "list", "--project", PROJECT_ID, "--limit", "2"])
        .success()
        .stdout_has("Alpha");
    assert_eq!(
        api.variables("ListDocuments"),
        json!({ "filter": { "project": { "id": { "eq": PROJECT_ID } } }, "first": 2 })
    );
}

fn releases(nodes: Value, end_cursor: Value) -> Value {
    let has_next = !end_cursor.is_null();
    json!({ "releases": page(nodes, end_cursor, has_next) })
}

#[test]
fn list_by_release_counts_a_release_on_two_pages_once() {
    const RELEASE_ID: &str = "00000000-0000-4000-9000-000000000060";
    let release = json!({ "id": RELEASE_ID, "name": "Summer", "version": "2026.8" });
    let api = MockLinear::start();
    api.on("ResolveReleases", releases(json!([release]), json!("next")))
        .on("ResolveReleases", releases(json!([release]), Value::Null))
        .on(
            "ListDocuments",
            json!({ "documents": page(json!([]), Value::Null, false) }),
        );
    Cli::for_api(&api)
        .run(&["document", "list", "--release", "2026.8", "--json"])
        .success();
    assert_eq!(
        api.variables("ListDocuments")["filter"],
        json!({ "release": { "id": { "eq": RELEASE_ID } } })
    );
}

#[test]
fn list_by_an_ambiguous_release_lists_the_matches() {
    let api = MockLinear::start();
    api.on(
        "ResolveReleases",
        releases(
            json!([
                { "id": "release-1", "name": "Summer", "version": "2026.8" },
                { "id": "release-2", "name": "2026.8", "version": null },
            ]),
            Value::Null,
        ),
    );
    Cli::for_api(&api)
        .run(&["document", "list", "--release", "2026.8"])
        .failure()
        .stderr_has("Release \"2026.8\" is ambiguous; it matches:\n  Summer (2026.8) — release-1\n  2026.8 — release-2")
        .stderr_has("Pass the release UUID instead.");
}

#[test]
fn view_raw_prints_markdown_from_a_url() {
    let api = MockLinear::start();
    api.on("GetDocument", json!({ "document": document() }));
    Cli::for_api(&api)
        .run(&[
            "document",
            "view",
            &format!("https://linear.app/acme/document/design-notes-{SLUG}"),
            "--raw",
            "--no-download",
        ])
        .success()
        .stdout_has("The plan in **bold**.");
    assert_eq!(api.variables("GetDocument"), json!({ "id": SLUG }));
}

#[test]
fn view_piped_prints_the_title_and_details_with_the_content() {
    let api = MockLinear::start();
    api.on("GetDocument", json!({ "document": document() }));
    Cli::for_api(&api)
        .run(&["document", "view", SLUG, "--no-download"])
        .success()
        .stdout_has("# Design notes")
        .stdout_has("**Project:** Roadmap")
        .stdout_has("The plan in **bold**.");
}

#[test]
fn view_missing_document_fails() {
    let api = MockLinear::start();
    api.on("GetDocument", json!({ "document": null }));
    Cli::for_api(&api)
        .run(&["document", "view", "gone123", "--raw"])
        .failure();
}

#[test]
fn view_json_collects_comment_pages() {
    let api = MockLinear::start();
    let first = comment("c1", "Looks good", None);
    let reply = comment("c2", "Thanks", Some("c1"));
    let with_comments = |nodes: Value, cursor: &str, more: bool| {
        let mut doc = document();
        doc["comments"] = page(nodes, json!(cursor), more);
        json!({ "document": doc })
    };
    api.on(
        "GetDocumentWithComments",
        with_comments(json!([first.clone()]), "cursor-1", true),
    )
    .on(
        "GetDocumentWithComments",
        with_comments(json!([reply.clone()]), "cursor-2", false),
    );
    let json = Cli::for_api(&api)
        .run(&["document", "view", SLUG, "--json"])
        .success()
        .json();
    assert_eq!(json["title"], "Design notes");
    assert_eq!(json["content"], document()["content"]);
    assert_eq!(nodes(&json["comments"]), [first, reply]);
    let variables: Vec<Value> = api.requests().into_iter().map(|r| r.variables).collect();
    assert_eq!(
        variables,
        [
            json!({ "id": SLUG, "commentsAfter": null }),
            json!({ "id": SLUG, "commentsAfter": "cursor-1" }),
        ]
    );
}

#[test]
fn view_json_stops_on_an_empty_comment_cursor() {
    let api = MockLinear::start();
    let mut doc = document();
    doc["comments"] = page(json!([comment("c1", "Looks good", None)]), json!(""), true);
    api.on("GetDocumentWithComments", json!({ "document": doc }));
    Cli::for_api(&api)
        .run(&["document", "view", SLUG, "--json"])
        .failure()
        .stderr_has("cursor");
    assert_eq!(api.operations(), ["GetDocumentWithComments"]);
}

#[test]
fn create_sends_title_content_and_project() {
    let api = MockLinear::start();
    api.on("CreateDocument", written("CreateDocument"));
    Cli::for_api(&api)
        .run(&[
            "document",
            "create",
            "--title",
            "Requested",
            "--content",
            "# Body\n\nText",
            "--project",
            PROJECT_ID,
            "--icon",
            "📄",
        ])
        .success()
        .stdout_has("Server title")
        .stdout_has(&format!("server-{SLUG}"));
    assert_eq!(
        api.variables("CreateDocument"),
        json!({ "input": {
            "title": "Requested", "content": "# Body\n\nText",
            "projectId": PROJECT_ID, "icon": "📄"
        } })
    );
}

#[test]
fn create_reads_content_from_a_file() {
    let api = MockLinear::start();
    api.on("CreateDocument", written("CreateDocument"));
    Cli::for_api(&api)
        .file(
            "cwd/body.md",
            "# From file\n\nMultiple words, with commas.\n",
        )
        .run(&[
            "document",
            "create",
            "-t",
            "Requested",
            "--content-file",
            "body.md",
            "--project",
            PROJECT_ID,
        ])
        .success();
    assert_eq!(
        api.variables("CreateDocument")["input"]["content"],
        "# From file\n\nMultiple words, with commas.\n"
    );
}

#[test]
fn create_reads_content_from_stdin() {
    let api = MockLinear::start();
    api.on("CreateDocument", written("CreateDocument"));
    Cli::for_api(&api)
        .stdin(b"Body")
        .run(&[
            "document",
            "create",
            "-t",
            "Requested",
            "--project",
            PROJECT_ID,
        ])
        .success();
    assert_eq!(api.variables("CreateDocument")["input"]["content"], "Body");
}

#[test]
fn create_attaches_to_a_team() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved("team-eng-id", "ENG", "Engineering"))
        .on("CreateDocument", written("CreateDocument"));
    Cli::for_api(&api)
        .run(&["document", "create", "-t", "Requested", "--team", "ENG"])
        .success();
    assert_eq!(api.variables("ResolveTeam"), resolve_vars("ENG"));
    assert_eq!(
        api.variables("CreateDocument"),
        json!({ "input": { "title": "Requested", "teamId": "team-eng-id" } })
    );
}

#[test]
fn create_without_title_fails_before_any_request() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&["document", "create", "--content", "Body"])
        .usage_error();
    Cli::for_api(&api)
        .run(&["document", "create", "-i"])
        .failure()
        .stderr_has("needs a terminal");
    assert!(api.requests().is_empty());
}

#[test]
fn create_and_update_accept_one_attachment_target() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api);
    cli.run(&[
        "document",
        "create",
        "-t",
        "T",
        "--project",
        PROJECT_ID,
        "--team",
        "ENG",
    ])
    .usage_error();
    cli.run(&[
        "document",
        "update",
        SLUG,
        "--issue",
        "ENG-1",
        "--release",
        "v1",
    ])
    .usage_error();
    cli.run(&["document", "create", "-t", "T", "--content", "Body"])
        .usage_error()
        .stderr_has("attachment target");
    assert!(api.requests().is_empty());
}

#[test]
fn update_without_fields_fails_before_any_request() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&["document", "update", SLUG])
        .usage_error()
        .stderr_has("No changes given");
    assert!(api.requests().is_empty());
}

#[test]
fn update_metadata_skips_the_inline_comment_check() {
    let api = MockLinear::start();
    api.on("UpdateDocument", written("UpdateDocument"));
    Cli::for_api(&api)
        .run(&[
            "document", "update", SLUG, "--title", "Renamed", "--icon", "📄",
        ])
        .success()
        .stdout_has("Server title");
    assert_eq!(
        api.variables("UpdateDocument"),
        json!({ "id": SLUG, "input": { "title": "Renamed", "icon": "📄" } })
    );
}

#[test]
fn update_content_checks_for_inline_comments_first() {
    let api = MockLinear::start();
    let resolved_quote = json!({
        "id": "old", "quotedText": "quote",
        "resolvedAt": "2024-01-01T00:00:00.000Z", "archivedAt": null
    });
    api.on("DocumentInlineCommentGuard", guard(json!([resolved_quote])))
        .on("UpdateDocument", written("UpdateDocument"));
    Cli::for_api(&api)
        .file("cwd/body.md", "new body\n")
        .run(&["document", "update", SLUG, "--content-file", "body.md"])
        .success();
    assert_eq!(
        api.operations(),
        ["DocumentInlineCommentGuard", "UpdateDocument"]
    );
    assert_eq!(
        api.variables("DocumentInlineCommentGuard"),
        json!({ "id": SLUG, "after": null })
    );
    assert_eq!(
        api.variables("UpdateDocument"),
        json!({ "id": SLUG, "input": { "content": "new body\n" } })
    );
}

#[test]
fn update_content_refuses_when_inline_comments_would_detach() {
    let api = MockLinear::start();
    let active = json!({
        "id": "active", "quotedText": "anchored text", "resolvedAt": null, "archivedAt": null
    });
    api.on("DocumentInlineCommentGuard", guard(json!([active])));
    Cli::for_api(&api)
        .run(&["document", "update", SLUG, "--content", "new body"])
        .failure()
        .stderr_has("--force");
    assert_eq!(api.operations(), ["DocumentInlineCommentGuard"]);
}

#[test]
fn update_content_with_force_skips_the_check() {
    let api = MockLinear::start();
    api.on("UpdateDocument", written("UpdateDocument"));
    Cli::for_api(&api)
        .run(&[
            "document",
            "update",
            SLUG,
            "--content",
            "new body",
            "--force",
        ])
        .success();
    assert_eq!(
        api.variables("UpdateDocument"),
        json!({ "id": SLUG, "input": { "content": "new body" } })
    );
}

#[test]
fn delete_resolves_a_url_then_deletes_by_id() {
    let api = MockLinear::start();
    api.on(
        "GetDocumentForDelete",
        json!({ "document": { "id": DOC_ID, "slugId": SLUG, "title": "Design notes" } }),
    )
    .on(
        "DeleteDocument",
        json!({ "documentDelete": { "success": true } }),
    );
    Cli::for_api(&api)
        .run(&[
            "document",
            "delete",
            &format!("https://linear.app/acme/document/design-notes-{SLUG}"),
            "-y",
        ])
        .success()
        .stdout_has("Design notes");
    assert_eq!(api.variables("GetDocumentForDelete"), json!({ "id": SLUG }));
    assert_eq!(api.variables("DeleteDocument"), json!({ "id": DOC_ID }));
}

#[test]
fn delete_without_yes_or_a_terminal_fails_before_any_request() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api);
    cli.run(&["document", "delete", SLUG])
        .failure()
        .stderr_has("--yes");
    cli.run(&["document", "delete", "--bulk", SLUG])
        .failure()
        .stderr_has("--yes");
    assert!(api.requests().is_empty());
}

#[test]
fn delete_bulk_deletes_each_document() {
    let api = MockLinear::start();
    for (id, slug) in [("doc-a", "aaa111"), ("doc-b", "bbb222")] {
        api.on(
            "GetDocumentForDelete",
            json!({ "document": { "id": id, "slugId": slug, "title": format!("Doc {slug}") } }),
        )
        .on(
            "DeleteDocument",
            json!({ "documentDelete": { "success": true } }),
        );
    }
    Cli::for_api(&api)
        .run(&["document", "delete", "--bulk", "aaa111", "bbb222", "-y"])
        .success();
    let mut lookups: Vec<Value> = api
        .requests()
        .into_iter()
        .filter(|r| r.operation.as_deref() == Some("GetDocumentForDelete"))
        .map(|r| r.variables)
        .collect();
    lookups.sort_by_key(Value::to_string);
    assert_eq!(
        lookups,
        [json!({ "id": "aaa111" }), json!({ "id": "bbb222" })]
    );
    let mut deletes: Vec<Value> = api
        .requests()
        .into_iter()
        .filter(|r| r.operation.as_deref() == Some("DeleteDocument"))
        .map(|r| r.variables)
        .collect();
    deletes.sort_by_key(Value::to_string);
    assert_eq!(
        deletes,
        [json!({ "id": "doc-a" }), json!({ "id": "doc-b" })]
    );
}

fn comment_target() -> Value {
    json!({ "document": { "id": DOC_ID, "title": "Design notes", "documentContentId": "content-1" } })
}

fn comment_created() -> Value {
    json!({ "commentCreate": {
        "success": true,
        "comment": { "id": "comment-new", "url": "https://linear.app/acme/comment/comment-new" }
    } })
}

#[test]
fn comment_add_replies_on_the_document_content() {
    let api = MockLinear::start();
    api.on("GetDocumentCommentTarget", comment_target())
        .on("AddComment", comment_created());
    Cli::for_api(&api)
        .run(&[
            "document",
            "comment",
            "add",
            &format!("https://linear.app/acme/document/design-notes-{SLUG}"),
            "--body",
            "Hi",
            "--parent",
            "c0000000-0000-4000-8000-0000000000a1",
        ])
        .success()
        .stdout_has("comment-new");
    assert_eq!(
        api.variables("GetDocumentCommentTarget"),
        json!({ "id": SLUG })
    );
    assert_eq!(
        api.variables("AddComment"),
        json!({ "input": {
            "body": "Hi", "parentId": "c0000000-0000-4000-8000-0000000000a1", "documentContentId": "content-1"
        } })
    );
}

#[test]
fn comment_add_reads_the_body_from_a_file() {
    let api = MockLinear::start();
    api.on("GetDocumentCommentTarget", comment_target())
        .on("AddComment", comment_created());
    Cli::for_api(&api)
        .file("cwd/comment.md", "**Bold** remark\n")
        .run(&[
            "document",
            "comment",
            "add",
            SLUG,
            "--body-file",
            "comment.md",
        ])
        .success();
    assert_eq!(
        api.variables("AddComment"),
        json!({ "input": { "body": "**Bold** remark\n", "documentContentId": "content-1" } })
    );
}

#[test]
fn comment_add_with_a_blank_body_fails_before_any_request() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&["document", "comment", "add", SLUG, "--body", "   "])
        .usage_error()
        .stderr_has("only whitespace");
    assert!(api.requests().is_empty());
}

#[test]
fn comment_add_without_a_body_or_terminal_fails_before_any_request() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&["document", "comment", "add", SLUG])
        .usage_error()
        .stderr_has("--body");
    assert!(api.requests().is_empty());
}

#[test]
fn comment_add_needs_the_document_content_record() {
    let api = MockLinear::start();
    api.on(
        "GetDocumentCommentTarget",
        json!({ "document": { "id": DOC_ID, "title": "Design notes", "documentContentId": null } }),
    );
    Cli::for_api(&api)
        .run(&["document", "comment", "add", SLUG, "--body", "Hi"])
        .failure()
        .stderr_has("Document \"Design notes\" has no content record to comment on");
    assert_eq!(api.operations(), ["GetDocumentCommentTarget"]);
}

#[test]
fn comment_add_reports_a_missing_document() {
    let api = MockLinear::start();
    api.on_error("GetDocumentCommentTarget", "Entity not found: Document");
    Cli::for_api(&api)
        .run(&["document", "comment", "add", "gone", "--body", "Hi"])
        .failure()
        .stderr_has("Document not found: gone");
    assert_eq!(api.operations(), ["GetDocumentCommentTarget"]);
}

fn listed_comment(id: &str, body: &str, parent: Option<&str>) -> Value {
    json!({
        "id": id, "body": body, "quotedText": null,
        "createdAt": "2024-01-02T12:00:00.000Z", "updatedAt": "2024-01-02T12:00:00.000Z",
        "editedAt": null, "url": format!("https://linear.app/acme/comment/{id}"),
        "user": { "id": "user-1", "name": "ada", "displayName": "Ada" },
        "externalUser": null, "botActor": null,
        "parent": parent.map(|id| json!({ "id": id }))
    })
}

fn document_comments() -> (Value, Value) {
    let nodes = json!([
        listed_comment("c1", "Root comment", None),
        listed_comment("c2", "Reply comment", Some("c1")),
    ]);
    let data = json!({ "document": {
        "id": DOC_ID, "comments": page(nodes.clone(), json!("end"), false)
    } });
    (nodes, data)
}

#[test]
fn comment_list_json_contains_comments() {
    let api = MockLinear::start();
    let (nodes, data) = document_comments();
    api.on("GetDocumentComments", data);
    let listed = Cli::for_api(&api)
        .run(&["document", "comment", "list", "--json", SLUG])
        .success()
        .json_nodes();
    assert_eq!(Value::Array(listed), nodes);
    assert_eq!(
        api.variables("GetDocumentComments"),
        json!({ "id": SLUG, "after": null, "first": 100 })
    );
}

#[test]
fn comment_list_text_shows_threads() {
    let api = MockLinear::start();
    api.on("GetDocumentComments", document_comments().1);
    Cli::for_api(&api)
        .run(&["document", "comment", "list", SLUG])
        .success()
        .stdout_has("Root comment")
        .stdout_has("Reply comment");
}

#[test]
fn comment_list_stops_on_an_empty_cursor() {
    let api = MockLinear::start();
    let (nodes, _) = document_comments();
    api.on(
        "GetDocumentComments",
        json!({ "document": { "id": DOC_ID, "comments": page(nodes, json!(""), true) } }),
    );
    Cli::for_api(&api)
        .run(&["document", "comment", "list", SLUG])
        .failure()
        .stderr_has("cursor");
    assert_eq!(api.operations(), ["GetDocumentComments"]);
}

fn for_edit(content: &str) -> Value {
    json!({ "document": { "id": DOC_ID, "title": "Design notes", "content": content } })
}

/// A sandbox whose `$EDITOR` is the stub `editor` running `script` on the file it is given.
fn with_editor(api: &MockLinear, script: &str) -> Cli {
    let cli = Cli::for_api(api).stub_bin("editor", script);
    let editor = cli.path("bin/editor").display().to_string();
    cli.env("EDITOR", &editor)
}

#[test]
fn update_edit_sends_the_edited_content() {
    let api = MockLinear::start();
    api.on("GetDocumentForEdit", for_edit("# Old\n"))
        .on("UpdateDocument", written("UpdateDocument"));
    let cli = with_editor(&api, "printf 'Added\\n' >> \"$1\"");
    cli.run(&["document", "update", SLUG, "--edit", "--force"])
        .success()
        .stdout_has("Design notes");
    assert_eq!(api.variables("GetDocumentForEdit"), json!({ "id": SLUG }));
    let content = api.variables("UpdateDocument")["input"]["content"].clone();
    assert_eq!(
        content.as_str().map(str::trim_end),
        Some("# Old\nAdded"),
        "{content}"
    );
    let calls = cli.calls("editor");
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].len(), 1, "{calls:?}");
}

#[test]
fn update_edit_without_changes_does_not_update() {
    for script in ["exit 0", ": > \"$1\""] {
        let api = MockLinear::start();
        api.on("GetDocumentForEdit", for_edit("# Old"));
        with_editor(&api, script)
            .run(&["document", "update", SLUG, "--edit", "--force"])
            .success()
            .stdout_has("No changes made.");
        assert_eq!(api.operations(), ["GetDocumentForEdit"]);
    }
}

#[test]
fn update_edit_fails_when_the_editor_fails() {
    for metadata in [Vec::new(), vec!["--title", "Renamed"]] {
        let api = MockLinear::start();
        api.on("GetDocumentForEdit", for_edit("# Old\n"));
        let mut args = vec!["document", "update", SLUG, "--edit", "--force"];
        args.extend(metadata);
        with_editor(&api, "exit 1")
            .run(&args)
            .failure()
            .stderr_has("ditor");
        assert_eq!(api.operations(), ["GetDocumentForEdit"]);
    }
}

#[test]
fn update_edit_without_an_editor_fails() {
    let api = MockLinear::start();
    api.on("GetDocumentForEdit", for_edit("# Old\n"));
    Cli::for_api(&api)
        .run(&["document", "update", SLUG, "--edit", "--force"])
        .failure()
        .stderr_has("EDITOR");
    assert_eq!(api.operations(), ["GetDocumentForEdit"]);
}

#[test]
fn create_warns_that_an_unreadable_reply_may_have_created_it() {
    let api = MockLinear::start();
    api.on_raw("CreateDocument", 200, "not json");
    Cli::for_api(&api)
        .run(&[
            "document",
            "create",
            "--title",
            "Notes",
            "--content",
            "Body",
            "--project",
            PROJECT_ID,
        ])
        .failure()
        .stderr_has("document may already exist");
}

#[test]
fn list_accepts_an_issue_url_and_normalizes_plain_issue_references() {
    for reference in [
        "https://linear.app/acme/issue/eng-1/title",
        "eng-1",
        "00000000-0000-4000-9000-000000000055",
    ] {
        let api = MockLinear::start();
        api.on(
            "GetIssueForDocumentTarget",
            json!({ "issue": { "id": "issue-id" } }),
        )
        .on(
            "ListDocuments",
            json!({ "documents": page(json!([]), Value::Null, false) }),
        );
        Cli::for_api(&api)
            .run(&["document", "list", "--issue", reference, "--json"])
            .success();
        let expected = if reference.starts_with("0000") {
            reference
        } else {
            "ENG-1"
        };
        assert_eq!(
            api.variables("GetIssueForDocumentTarget"),
            json!({ "id": expected })
        );
        assert_eq!(
            api.variables("ListDocuments"),
            json!({ "filter": { "issue": { "id": { "eq": "issue-id" } } }, "first": 50 })
        );
    }
}

#[test]
fn list_rejects_wrong_kind_issue_urls_after_workspace_checks() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api).env("LINEAR_WORKSPACE", "acme");
    cli.run(&[
        "document",
        "list",
        "--issue",
        "https://linear.app/acme/project/mobile-abcdef123456",
    ])
    .failure()
    .stderr_has("is a project URL, not an issue URL.")
    .stderr_has("Pass an issue URL, identifier like ENG-123, or UUID.");
    let run = cli.run(&[
        "document",
        "list",
        "--issue",
        "https://linear.app/foreign/project/mobile-abcdef123456",
    ]);
    run.failure().stderr_has("this is the \"acme\" workspace");
    assert!(!run.stderr.contains("not an issue URL"));
    assert!(api.requests().is_empty());
}

#[test]
fn update_edit_unchanged_still_updates_metadata() {
    for (flags, input) in [
        (["--title", "Renamed"], json!({"title": "Renamed"})),
        (["--icon", "📄"], json!({"icon": "📄"})),
        (["--project", PROJECT_ID], json!({"projectId": PROJECT_ID})),
    ] {
        let api = MockLinear::start();
        api.on("GetDocumentForEdit", for_edit("# Old"))
            .on("UpdateDocument", written("UpdateDocument"));
        let mut args = vec!["document", "update", SLUG, "--edit"];
        args.extend(flags);
        let run = with_editor(&api, "exit 0").run(&args);
        run.success();
        assert_eq!(api.operations(), ["GetDocumentForEdit", "UpdateDocument"]);
        assert_eq!(
            api.variables("UpdateDocument"),
            json!({"id": SLUG, "input": input})
        );
        run.stdout_has("Server title");
    }
}

#[test]
fn update_edit_blank_still_updates_metadata() {
    for (flags, input) in [
        (["--title", "Renamed"], json!({"title": "Renamed"})),
        (["--icon", "📄"], json!({"icon": "📄"})),
        (["--project", PROJECT_ID], json!({"projectId": PROJECT_ID})),
    ] {
        let api = MockLinear::start();
        api.on("GetDocumentForEdit", for_edit("# Old"))
            .on("UpdateDocument", written("UpdateDocument"));
        let mut args = vec!["document", "update", SLUG, "--edit"];
        args.extend(flags);
        let run = with_editor(&api, ": > \"$1\"").run(&args);
        run.success();
        assert_eq!(api.operations(), ["GetDocumentForEdit", "UpdateDocument"]);
        assert_eq!(
            api.variables("UpdateDocument"),
            json!({"id": SLUG, "input": input})
        );
        run.stdout_has("Server title");
    }
}

#[test]
fn update_edit_with_metadata_preserves_the_inline_comment_guard() {
    for active in [false, true] {
        let api = MockLinear::start();
        let comments = if active {
            json!([{"id": "active", "quotedText": "anchored text", "resolvedAt": null, "archivedAt": null}])
        } else {
            json!([])
        };
        api.on("GetDocumentForEdit", for_edit("# Old\n"))
            .on("DocumentInlineCommentGuard", guard(comments));
        if !active {
            api.on("UpdateDocument", written("UpdateDocument"));
        }
        let run = with_editor(&api, "printf 'Added\\n' >> \"$1\"")
            .run(&["document", "update", SLUG, "--edit", "--title", "Renamed"]);
        if active {
            run.failure().stderr_has("--force");
            assert_eq!(
                api.operations(),
                ["GetDocumentForEdit", "DocumentInlineCommentGuard"]
            );
        } else {
            run.success();
            assert_eq!(
                api.operations(),
                [
                    "GetDocumentForEdit",
                    "DocumentInlineCommentGuard",
                    "UpdateDocument"
                ]
            );
            assert_eq!(
                api.variables("UpdateDocument"),
                json!({"id": SLUG, "input": {"title": "Renamed", "content": "# Old\nAdded"}})
            );
        }
    }
}

#[test]
fn update_edit_on_a_terminal_saves_only_when_confirmed() {
    let api = MockLinear::start();
    api.on("GetDocumentForEdit", for_edit("# Old\n"));
    with_editor(&api, "printf 'Added\\n' >> \"$1\"")
        .run_tty(
            &["document", "update", SLUG, "--edit"],
            &[("Save the edited text of \"Design notes\"? (y/N)", "\r")],
        )
        .success()
        .stdout_has("Canceled.");
    assert_eq!(api.operations(), ["GetDocumentForEdit"]);
}

#[test]
fn update_reads_the_content_from_stdin_with_a_dash() {
    let api = MockLinear::start();
    api.on("DocumentInlineCommentGuard", guard(json!([])))
        .on("UpdateDocument", written("UpdateDocument"));
    Cli::for_api(&api)
        .stdin(b"piped body\n")
        .run(&[
            "document",
            "update",
            SLUG,
            "--content-file",
            "-",
            "--title",
            "New",
        ])
        .success();
    assert_eq!(
        api.variables("UpdateDocument"),
        json!({ "id": SLUG, "input": { "title": "New", "content": "piped body\n" } })
    );
}

#[test]
fn delete_bulk_lists_the_documents_before_deleting_and_skips_missing_ones() {
    let api = MockLinear::start();
    api.on(
        "GetDocumentForDelete",
        json!({ "document": { "id": "doc-a", "slugId": "aaa111", "title": "Design notes" } }),
    )
    .on(
        "DeleteDocument",
        json!({ "documentDelete": { "success": true } }),
    );
    let run = Cli::for_api(&api).run(&[
        "document",
        "delete",
        "--bulk",
        "aaa111",
        "https://linear.app/acme/issue/ENG-1",
        "-y",
    ]);
    run.failure()
        .stdout_has("1 document to delete:\n  Design notes\n")
        .stdout_has(
            "Skipping 1 document that could not be found:\n  https://linear.app/acme/issue/ENG-1: ",
        );
    assert_eq!(api.operations(), ["GetDocumentForDelete", "DeleteDocument"]);
}

#[test]
fn create_on_a_terminal_asks_for_the_attachment_again_until_it_is_found() {
    let api = MockLinear::start();
    let projects = |nodes: Value| json!({ "projects": { "nodes": nodes } });
    api.on("GetProjectIdByName", projects(json!([])))
        .on("GetProjectIdBySlugId", projects(json!([])))
        .on(
            "GetProjectIdByName",
            projects(json!([{ "id": PROJECT_ID }])),
        );
    let cli = with_editor(&api, "printf 'Body' > \"$1\"");
    let run = cli.run_tty(
        &["document", "create"],
        &[
            ("Document title", "Notes\r"),
            ("Attach the document to:", "\r"),
            ("Project (UUID, slug ID, or name)", "nope\r"),
            ("Project not found: nope", ""),
            ("Attach the document to:", "\r"),
            ("Project (UUID, slug ID, or name)", "Mobile\r"),
            ("Create document \"Notes\"? (y/N)", "\r"),
        ],
    );
    assert_eq!(run.code, 0, "{run}");
    assert_eq!(cli.calls("editor").len(), 1, "{run}");
    assert_eq!(
        api.operations(),
        [
            "GetProjectIdByName",
            "GetProjectIdBySlugId",
            "GetProjectIdByName"
        ]
    );
}

#[test]
fn create_on_a_terminal_checks_the_attachment_flag_before_the_editor() {
    let api = MockLinear::start();
    let projects = json!({ "projects": { "nodes": [] } });
    api.on("GetProjectIdByName", projects.clone())
        .on("GetProjectIdBySlugId", projects);
    let cli = with_editor(&api, "printf 'Body' > \"$1\"");
    let run = cli.run_tty(
        &["document", "create", "-t", "Notes", "--project", "nope"],
        &[],
    );
    assert_eq!(run.code, 1, "{run}");
    assert!(run.stdout.contains("Project not found: nope"), "{run}");
    assert!(cli.calls("editor").is_empty());
}
