//! `issue comment` add, update, delete and list.
use serde_json::{Value, json};

use crate::support::{Cli, MockLinear};

const COMMENT_ID: &str = "7d2e4f1a-3b5c-4d6e-8f90-a1b2c3d4e5f6";

fn created() -> Value {
    json!({
        "commentCreate": {
            "success": true,
            "comment": { "id": "comment-new", "url": "https://linear.app/acme/comment-new" }
        }
    })
}

/// A `FileUpload` reply whose signed upload target is `path` on the mock.
pub fn file_upload(api: &MockLinear, name: &str, path: &str) -> Value {
    json!({
        "fileUpload": {
            "success": true,
            "uploadFile": {
                "assetUrl": format!("https://uploads.linear.app/acme/{name}"),
                "uploadUrl": format!("{}{path}", api.base_url()),
                "headers": [{ "key": "x-upload-token", "value": "signed" }]
            }
        }
    })
}

#[test]
fn add_posts_the_body_to_the_issue() {
    let api = MockLinear::start();
    api.on("AddComment", created());
    Cli::for_api(&api)
        .run(&[
            "issue",
            "comment",
            "add",
            "eng-1",
            "--body",
            "Looks **good**",
        ])
        .success()
        .stdout_has("✓ Added comment to issue ENG-1\nhttps://linear.app/acme/comment-new\n");
    assert_eq!(
        api.variables("AddComment"),
        json!({ "input": { "body": "Looks **good**", "issueId": "ENG-1" } })
    );
}

#[test]
fn add_refuses_a_parent_url_before_uploading_or_looking_up() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .file("cwd/shot.png", "png")
        .run(&[
            "issue",
            "comment",
            "add",
            "eng-1",
            "--attach",
            "shot.png",
            "--parent",
            "https://linear.app/acme/issue/ENG-1/title#comment-abcdef12",
        ])
        .failure()
        .stderr_has("links to a comment");
    assert!(api.requests().is_empty());
    Cli::for_api(&api)
        .run(&[
            "project",
            "comment",
            "add",
            "Roadmap",
            "--body",
            "Hi",
            "--parent",
            "https://linear.app/acme/issue/ENG-1",
        ])
        .failure()
        .stderr_has("is a Linear URL");
    assert!(api.requests().is_empty());
}

#[test]
fn add_reads_the_body_file_and_replies_to_a_parent() {
    let api = MockLinear::start();
    api.on("AddComment", created());
    Cli::for_api(&api)
        .file("cwd/body.md", "# Notes\n\nFrom a file\n")
        .run(&[
            "issue",
            "comment",
            "add",
            "ENG-1",
            "--body-file",
            "body.md",
            "--parent",
            "comment-parent",
        ])
        .success();
    assert_eq!(
        api.variables("AddComment"),
        json!({
            "input": {
                "body": "# Notes\n\nFrom a file\n",
                "issueId": "ENG-1",
                "parentId": "comment-parent"
            }
        })
    );
}

#[test]
fn add_uploads_attachments_and_links_them_in_the_body() {
    let api = MockLinear::start();
    api.on(
        "FileUpload",
        file_upload(&api, "shot.png", "/signed/shot.png"),
    )
    .on_http("PUT", "/signed/shot.png", 200, b"")
    .on(
        "FileUpload",
        file_upload(&api, "notes.txt", "/signed/notes.txt"),
    )
    .on_http("PUT", "/signed/notes.txt", 200, b"")
    .on("AddComment", created());
    Cli::for_api(&api)
        .file("cwd/shot.png", "png-bytes")
        .file("cwd/notes.txt", "some notes\n")
        .run(&[
            "issue",
            "comment",
            "add",
            "ENG-1",
            "--body",
            "See attached",
            "--attach",
            "shot.png",
            "--attach",
            "notes.txt",
        ])
        .success()
        .stdout_has("shot.png")
        .stdout_has("notes.txt");

    let uploads: Vec<Value> = api
        .requests()
        .into_iter()
        .filter(|request| request.operation.as_deref() == Some("FileUpload"))
        .map(|request| request.variables)
        .collect();
    assert_eq!(
        uploads,
        [
            json!({ "contentType": "image/png", "filename": "shot.png", "size": 9, "makePublic": false }),
            json!({ "contentType": "text/plain", "filename": "notes.txt", "size": 11, "makePublic": false }),
        ]
    );
    let puts: Vec<_> = api
        .requests()
        .into_iter()
        .filter(|request| request.method == "PUT")
        .collect();
    assert_eq!(puts[0].body, b"png-bytes");
    assert_eq!(puts[0].header("x-upload-token"), Some("signed"));
    assert_eq!(puts[1].body, b"some notes\n");

    let body = api.variables("AddComment")["input"]["body"]
        .as_str()
        .expect("comment body")
        .to_owned();
    assert!(body.contains("See attached"), "{body}");
    assert!(
        body.contains("![shot.png](https://uploads.linear.app/acme/shot.png)"),
        "{body}"
    );
    assert!(
        body.contains("[notes.txt](https://uploads.linear.app/acme/notes.txt)"),
        "{body}"
    );
}

#[test]
fn add_stops_when_an_upload_fails() {
    let api = MockLinear::start();
    api.on(
        "FileUpload",
        file_upload(&api, "notes.txt", "/signed/notes.txt"),
    )
    .on_http("PUT", "/signed/notes.txt", 403, b"denied");
    Cli::for_api(&api)
        .file("cwd/notes.txt", "some notes\n")
        .run(&["issue", "comment", "add", "ENG-1", "--attach", "notes.txt"])
        .failure();
    assert!(!api.operations().contains(&"AddComment".to_owned()));
}

#[test]
fn add_with_a_missing_body_file_fails_before_any_request() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&[
            "issue",
            "comment",
            "add",
            "ENG-1",
            "--body-file",
            "missing.md",
        ])
        .failure();
    assert!(api.requests().is_empty());
}

fn updated() -> Value {
    json!({
        "commentUpdate": {
            "success": true,
            "comment": {
                "id": COMMENT_ID, "body": "New body", "updatedAt": "2026-01-01T00:00:00.000Z",
                "url": "https://linear.app/acme/comment-updated",
                "user": { "name": "alice", "displayName": "Alice" }
            }
        }
    })
}

#[test]
fn update_sends_the_new_body() {
    let api = MockLinear::start();
    api.on("UpdateComment", updated());
    Cli::for_api(&api)
        .run(&[
            "issue", "comment", "update", COMMENT_ID, "--body", "New body",
        ])
        .success()
        .stdout_has(&format!(
            "✓ Updated comment {COMMENT_ID}\nhttps://linear.app/acme/comment-updated\n"
        ));
    assert_eq!(
        api.variables("UpdateComment"),
        json!({ "id": COMMENT_ID, "input": { "body": "New body" } })
    );
}

#[test]
fn update_without_a_body_or_terminal_fails_before_any_request() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .stdin(b"Piped body\n")
        .run(&["issue", "comment", "update", COMMENT_ID])
        .usage_error()
        .stderr_has("--body");
    assert!(api.requests().is_empty());
}

#[test]
fn update_reads_the_body_file() {
    let api = MockLinear::start();
    api.on("UpdateComment", updated());
    Cli::for_api(&api)
        .file("cwd/body.md", "Edited\nin a file\n")
        .run(&[
            "issue",
            "comment",
            "update",
            COMMENT_ID,
            "--body-file",
            "body.md",
        ])
        .success();
    assert_eq!(
        api.variables("UpdateComment")["input"]["body"],
        "Edited\nin a file\n"
    );
}

#[test]
fn update_reports_api_errors() {
    let api = MockLinear::start();
    api.on_error("UpdateComment", "Comment is locked");
    Cli::for_api(&api)
        .run(&["issue", "comment", "update", COMMENT_ID, "-b", "x"])
        .failure()
        .stderr_has("Comment is locked");
}

fn comment_for_delete() -> Value {
    json!({ "comment": { "body": "Looks good\nShip it", "issue": { "identifier": "ENG-7" } } })
}

#[test]
fn delete_removes_the_comment() {
    let api = MockLinear::start();
    api.on("GetCommentForDelete", comment_for_delete()).on(
        "DeleteComment",
        json!({ "commentDelete": { "success": true } }),
    );
    Cli::for_api(&api)
        .run(&["issue", "comment", "delete", COMMENT_ID, "--yes"])
        .success()
        .stdout_has(&format!("✓ Deleted comment {COMMENT_ID}\n"));
    assert_eq!(api.variables("DeleteComment"), json!({ "id": COMMENT_ID }));
}

#[test]
fn delete_fails_when_the_api_reports_no_success() {
    let api = MockLinear::start();
    api.on("GetCommentForDelete", comment_for_delete()).on(
        "DeleteComment",
        json!({ "commentDelete": { "success": false } }),
    );
    Cli::for_api(&api)
        .run(&["issue", "comment", "delete", COMMENT_ID, "-y"])
        .failure();
}

#[test]
fn delete_needs_yes_without_a_terminal() {
    let api = MockLinear::start();
    api.on("GetCommentForDelete", comment_for_delete());
    Cli::for_api(&api)
        .run(&["issue", "comment", "delete", COMMENT_ID])
        .failure()
        .stderr_has("--yes");
    assert_eq!(api.operations(), ["GetCommentForDelete"]);
}

#[test]
fn delete_names_the_comment_and_defaults_to_no_on_a_terminal() {
    let api = MockLinear::start();
    api.on("GetCommentForDelete", comment_for_delete());
    Cli::for_api(&api)
        .run_tty(
            &["issue", "comment", "delete", COMMENT_ID],
            &[(
                "delete the comment on ENG-7 (\"Looks good Ship it\")? (y/N)",
                "\r",
            )],
        )
        .success()
        .stdout_has("Canceled.");
    assert_eq!(api.operations(), ["GetCommentForDelete"]);
}

#[test]
fn delete_reports_an_unknown_comment_before_asking() {
    let api = MockLinear::start();
    api.on_error("GetCommentForDelete", "Entity not found: Comment");
    Cli::for_api(&api)
        .run(&["issue", "comment", "delete", COMMENT_ID, "--yes"])
        .failure()
        .stderr_has(&format!("Comment not found: {COMMENT_ID}"));
    assert_eq!(api.operations(), ["GetCommentForDelete"]);
}

fn comment(id: &str, body: &str, user: &str, parent: Option<&str>) -> Value {
    json!({
        "id": id, "body": body, "quotedText": null,
        "createdAt": "2026-01-02T12:00:00.000Z", "updatedAt": "2026-01-02T12:00:00.000Z",
        "editedAt": null, "url": format!("https://linear.app/acme/issue/ENG-7#comment-{id}"),
        "user": { "id": format!("user-{user}"), "name": user, "displayName": user },
        "externalUser": null, "botActor": null,
        "parent": parent.map(|id| json!({ "id": id }))
    })
}

fn comments_page(nodes: Vec<Value>, end_cursor: Value, has_next: bool) -> Value {
    json!({
        "issue": {
            "comments": {
                "nodes": nodes,
                "pageInfo": { "hasNextPage": has_next, "endCursor": end_cursor }
            }
        }
    })
}

#[test]
fn list_json_follows_pages() {
    let api = MockLinear::start();
    let root = comment("c1", "Root comment", "alice", None);
    let reply = comment("c2", "A reply", "bob", Some("c1"));
    api.on(
        "GetIssueComments",
        comments_page(vec![root.clone()], json!("cursor-1"), true),
    )
    .on(
        "GetIssueComments",
        comments_page(vec![reply.clone()], json!("cursor-2"), false),
    );
    let listed = Cli::for_api(&api)
        .run(&["issue", "comment", "list", "eng-7", "--json"])
        .success()
        .json_nodes();
    assert_eq!(listed, [root, reply]);
    let variables: Vec<Value> = api.requests().into_iter().map(|r| r.variables).collect();
    assert_eq!(
        variables,
        [
            json!({ "id": "ENG-7", "after": null, "first": 100 }),
            json!({ "id": "ENG-7", "after": "cursor-1", "first": 100 })
        ]
    );
}

#[test]
fn list_limit_asks_linear_for_only_that_many() {
    let api = MockLinear::start();
    let root = comment("c1", "Root comment", "alice", None);
    api.on(
        "GetIssueComments",
        comments_page(vec![root.clone()], json!("cursor-1"), true),
    );
    let listed = Cli::for_api(&api)
        .run(&[
            "issue", "comment", "list", "eng-7", "--limit", "1", "--json",
        ])
        .success()
        .json_nodes();
    assert_eq!(listed, [root]);
    assert_eq!(
        api.variables("GetIssueComments"),
        json!({ "id": "ENG-7", "after": null, "first": 1 })
    );
}

#[test]
fn list_text_shows_threads() {
    let api = MockLinear::start();
    api.on(
        "GetIssueComments",
        comments_page(
            vec![
                comment("c1", "Root comment", "alice", None),
                comment("c2", "A reply", "bob", Some("c1")),
            ],
            Value::Null,
            false,
        ),
    );
    Cli::for_api(&api)
        .run(&["issue", "comment", "list", "ENG-7"])
        .success()
        .stdout_has("Root comment")
        .stdout_has("A reply")
        .stdout_has("alice")
        .stdout_has("bob");
}

#[test]
fn list_text_neutralizes_terminal_escape_sequences() {
    let api = MockLinear::start();
    api.on(
        "GetIssueComments",
        comments_page(
            vec![comment(
                "c1",
                "Looks fine\u{1b}]0;pwned\u{7}\u{1b}[2J",
                "eve\u{1b}[8m",
                None,
            )],
            Value::Null,
            false,
        ),
    );
    let run = Cli::for_api(&api).run(&["issue", "comment", "list", "ENG-7"]);
    run.success()
        .stdout_has("Looks fine\u{FFFD}]0;pwned\u{FFFD}\u{FFFD}[2J")
        .stdout_has("@eve\u{FFFD}[8m");
    assert!(!run.stdout.contains('\u{1b}'), "{run}");
}

#[test]
fn every_comment_command_refuses_a_non_utf8_body_file_before_any_request() {
    for target in [
        &["project", "comment", "add", "roadmap"][..],
        &["initiative", "comment", "add", "Growth"],
        &["document", "comment", "add", COMMENT_ID],
        &["issue", "comment", "add", "ENG-1"],
    ] {
        let api = MockLinear::start();
        let cli = Cli::for_api(&api);
        std::fs::write(cli.path("cwd/bad.md"), b"c\xc0\xafd").expect("write body file");
        let mut args = target.to_vec();
        args.extend(["--body-file", "bad.md"]);
        cli.run(&args)
            .failure()
            .stderr_has("Body file must be valid UTF-8");
        assert!(api.requests().is_empty(), "{target:?}");
    }
}
