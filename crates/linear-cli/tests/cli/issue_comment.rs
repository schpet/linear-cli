//! `issue comment` add, update, delete and list.
use serde_json::{Value, json};

use crate::support::{Cli, MockLinear};

const COMMENT_ID: &str = "7d2e4f1a-3b5c-4d6e-8f90-a1b2c3d4e5f6";

/// A top-level comment for `--reply-to` to find, on the entity whose
/// `field` (like `issueId`) is `value`.
fn reply_parent(field: &str, value: &str) -> Value {
    let mut comment = json!({
        "parentId": null, "issueId": null, "projectId": null,
        "initiativeId": null, "documentContentId": null
    });
    comment[field] = json!(value);
    json!({ "comment": comment })
}

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
    api.on("GetIssueId", json!({ "issue": { "id": "issue-1-id" } }))
        .on("GetReplyParent", reply_parent("issueId", "issue-1-id"))
        .on("AddComment", created());
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
            "c0000000-0000-4000-8000-0000000000a1",
        ])
        .success()
        .stdout_has("✓ Added reply to issue ENG-1");
    assert_eq!(
        api.variables("AddComment"),
        json!({
            "input": {
                "body": "# Notes\n\nFrom a file",
                "issueId": "ENG-1",
                "parentId": "c0000000-0000-4000-8000-0000000000a1"
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
        "Edited\nin a file"
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
        .usage_error()
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
                "Delete comment \"Looks good Ship it\" on ENG-7? (y/N)",
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
        .not_found()
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
        "parent": parent.map(|id| json!({ "id": id, "resolvedAt": null })),
        "resolvedAt": null, "resolvingCommentId": null, "resolvingUser": null
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
        .stdout_has("@alice commented")
        .stdout_has("@bob replied");
}

#[test]
fn list_on_a_terminal_renders_markdown_and_wraps_like_issue_view() {
    let api = MockLinear::start();
    let long = "word ".repeat(30);
    let mut reply = comment(
        "c2",
        &format!("Reply with *emphasis* {long}"),
        "bob",
        Some("c1"),
    );
    reply["quotedText"] = json!("the quoted part");
    api.on(
        "GetIssueComments",
        comments_page(
            vec![
                comment(
                    "c1",
                    &format!("Root with **bold** text {long}"),
                    "alice",
                    None,
                ),
                reply,
            ],
            Value::Null,
            false,
        ),
    );
    let run = Cli::for_api(&api).run_tty(&["issue", "comment", "list", "ENG-7", "--no-pager"], &[]);
    run.success()
        .stdout_has("@alice commented")
        .stdout_has("Root with bold text")
        .stdout_has("  @bob replied")
        .stdout_has("  Reply with emphasis")
        .stdout_has("[c2]");
    assert!(!run.stdout.contains("**"), "{run}");
    assert!(run.stdout.contains("│ the quoted part"), "{run}");
    let widest = run.stdout.lines().map(|line| line.chars().count()).max();
    assert!(widest.is_some_and(|width| width <= 80), "{run}");
    // Reply bodies stay indented under their header when they wrap.
    let reply_lines: Vec<&str> = run
        .stdout
        .lines()
        .skip_while(|line| !line.contains("@bob replied"))
        .skip(1)
        .filter(|line| !line.is_empty())
        .collect();
    assert!(reply_lines.len() > 1, "{run}");
    assert!(
        reply_lines.iter().all(|line| line.starts_with("  ")),
        "{run}"
    );
}

#[test]
fn list_text_shows_threads_oldest_first_like_issue_view() {
    let api = MockLinear::start();
    let mut newer = comment("c2", "Newer thread", "bob", None);
    newer["createdAt"] = json!("2026-01-03T12:00:00.000Z");
    api.on(
        "GetIssueComments",
        comments_page(
            vec![newer, comment("c1", "Older thread", "alice", None)],
            Value::Null,
            false,
        ),
    );
    let run = Cli::for_api(&api).run(&["issue", "comment", "list", "ENG-7"]);
    run.success();
    let older = run.stdout.find("Older thread").expect("older thread");
    let newer = run.stdout.find("Newer thread").expect("newer thread");
    assert!(older < newer, "{run}");
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

/// An editor that appends ` (edited)` to the file it is given.
const APPENDING_EDITOR: &str = "printf ' (edited)' >> \"$1\"";

#[test]
fn add_on_a_terminal_checks_the_issue_then_confirms_the_edited_body() {
    let api = MockLinear::start();
    api.on("GetIssueId", json!({ "issue": { "id": "issue-1-id" } }))
        .on("AddComment", created());
    let cli = Cli::for_api(&api)
        .stub_bin("editor", APPENDING_EDITOR)
        .env("VISUAL", "editor");
    cli.run_tty(
        &["issue", "comment", "add", "ENG-1"],
        &[("Post this comment on ENG-1? (y/N)", "y\r")],
    )
    .success()
    .stdout_has("✓ Added comment to issue ENG-1");
    assert_eq!(api.operations(), ["GetIssueId", "AddComment"]);
    assert_eq!(
        api.variables("AddComment"),
        json!({ "input": { "body": "(edited)", "issueId": "ENG-1" } })
    );
}

#[test]
fn add_on_a_terminal_does_not_open_the_editor_for_a_missing_issue() {
    let api = MockLinear::start();
    api.on_error("GetIssueId", "Entity not found: Issue");
    let cli = Cli::for_api(&api)
        .stub_bin("editor", APPENDING_EDITOR)
        .env("VISUAL", "editor");
    cli.run_tty(&["issue", "comment", "add", "ENG-404"], &[])
        .not_found()
        .stdout_has("Issue not found: ENG-404");
    assert!(cli.calls("editor").is_empty());
}

#[test]
fn add_on_a_terminal_posts_nothing_when_declined() {
    let api = MockLinear::start();
    api.on("GetIssueId", json!({ "issue": { "id": "issue-1-id" } }));
    Cli::for_api(&api)
        .stub_bin("editor", APPENDING_EDITOR)
        .env("VISUAL", "editor")
        .run_tty(&["issue", "comment", "add", "ENG-1"], &[("(y/N)", "\r")])
        .success()
        .stdout_has("Canceled.");
    assert_eq!(api.operations(), ["GetIssueId"]);
}

#[test]
fn update_on_a_terminal_edits_the_existing_body() {
    let api = MockLinear::start();
    api.on("GetComment", json!({ "comment": { "body": "Old body" } }))
        .on("UpdateComment", updated());
    Cli::for_api(&api)
        .stub_bin("editor", APPENDING_EDITOR)
        .env("VISUAL", "editor")
        .run_tty(
            &["issue", "comment", "update", COMMENT_ID],
            &[("Save the edited comment? (y/N)", "y\r")],
        )
        .success();
    assert_eq!(
        api.variables("UpdateComment")["input"]["body"],
        "Old body (edited)"
    );
}

#[test]
fn add_reads_the_body_from_stdin_with_a_dash() {
    let api = MockLinear::start();
    api.on("AddComment", created());
    Cli::for_api(&api)
        .stdin(b"Piped comment\n")
        .run(&["issue", "comment", "add", "ENG-1", "--body-file", "-"])
        .success();
    assert_eq!(
        api.variables("AddComment")["input"]["body"],
        "Piped comment"
    );
    Cli::for_api(&api)
        .stdin(b"\n")
        .run(&["issue", "comment", "add", "ENG-1", "--body-file", "-"])
        .usage_error()
        .stderr_has("Body file is empty: stdin");
}

const PARENT_ID: &str = "c0000000-0000-4000-8000-0000000000a1";

#[test]
fn add_refuses_a_reply_to_that_is_not_a_uuid_before_any_request() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&[
            "issue",
            "comment",
            "add",
            "ENG-1",
            "--body",
            "Hi",
            "--reply-to",
            "comment-1",
        ])
        .usage_error()
        .stderr_has("Not a comment UUID: comment-1");
    assert!(api.requests().is_empty());
}

#[test]
fn add_on_a_terminal_checks_the_parent_before_the_editor() {
    let api = MockLinear::start();
    api.on("GetIssueId", json!({ "issue": { "id": "issue-1-id" } }))
        .on_error("GetReplyParent", "Entity not found: Comment");
    let cli = Cli::for_api(&api)
        .stub_bin("editor", APPENDING_EDITOR)
        .env("VISUAL", "editor");
    cli.run_tty(
        &["issue", "comment", "add", "ENG-1", "--reply-to", PARENT_ID],
        &[],
    )
    .not_found()
    .stdout_has(&format!("Comment not found: {PARENT_ID}"));
    assert!(cli.calls("editor").is_empty());
}

#[test]
fn add_on_a_terminal_asks_to_post_a_reply() {
    let api = MockLinear::start();
    api.on("GetIssueId", json!({ "issue": { "id": "issue-1-id" } }))
        .on("GetReplyParent", reply_parent("issueId", "issue-1-id"))
        .on("AddComment", created());
    Cli::for_api(&api)
        .stub_bin("editor", APPENDING_EDITOR)
        .env("VISUAL", "editor")
        .run_tty(
            &["issue", "comment", "add", "ENG-1", "-p", PARENT_ID],
            &[("Post this reply on ENG-1? (y/N)", "y\r")],
        )
        .success();
    assert_eq!(api.variables("AddComment")["input"]["parentId"], PARENT_ID);
}

#[test]
fn add_refuses_to_reply_to_a_reply_naming_its_thread() {
    let api = MockLinear::start();
    let mut parent = reply_parent("issueId", "issue-1-id");
    parent["comment"]["parentId"] = json!("thread-root-id");
    api.on("GetIssueId", json!({ "issue": { "id": "issue-1-id" } }))
        .on("GetReplyParent", parent);
    Cli::for_api(&api)
        .run(&[
            "issue",
            "comment",
            "add",
            "ENG-1",
            "--body",
            "Hi",
            "--reply-to",
            PARENT_ID,
        ])
        .failure()
        .stderr_has("only a top-level comment can be replied to")
        .stderr_has("--reply-to thread-root-id");
    assert!(!api.operations().contains(&"AddComment".to_owned()));
}

#[test]
fn add_refuses_to_reply_to_a_comment_on_another_entity() {
    for (target, field) in [
        (&["issue", "comment", "add", "ENG-1"][..], "issueId"),
        (
            &[
                "project",
                "comment",
                "add",
                "a0000000-0000-4000-8000-000000000001",
            ],
            "projectId",
        ),
    ] {
        let api = MockLinear::start();
        if field == "issueId" {
            api.on("GetIssueId", json!({ "issue": { "id": "issue-1-id" } }));
        }
        api.on("GetReplyParent", reply_parent(field, "someone-else"));
        let mut args = target.to_vec();
        args.extend(["--body", "Hi", "--reply-to", PARENT_ID]);
        Cli::for_api(&api)
            .run(&args)
            .failure()
            .stderr_has(&format!("Comment {PARENT_ID} is not on"));
        assert!(
            !api.operations().contains(&"AddComment".to_owned()),
            "{target:?}"
        );
    }
}

#[test]
fn update_on_a_terminal_with_the_text_unchanged_saves_nothing() {
    let api = MockLinear::start();
    api.on("GetComment", json!({ "comment": { "body": "Old body\n" } }));
    let run = Cli::for_api(&api)
        .stub_bin("editor", "exit 0")
        .env("VISUAL", "editor")
        .run_tty(&["issue", "comment", "update", COMMENT_ID], &[]);
    assert_eq!(run.code, 0, "{run}");
    assert!(run.stdout.contains("No changes made."), "{run}");
    assert!(!run.stdout.contains("(y/N)"), "{run}");
    assert_eq!(api.operations(), ["GetComment"]);
}

const THREAD_A: &str = "a1111111-1111-4111-8111-111111111111";
const THREAD_B: &str = "b2222222-2222-4222-8222-222222222222";
const THREAD_C: &str = "c3333333-3333-4333-8333-333333333333";
const REPLY: &str = "d4444444-4444-4444-8444-444444444444";

/// What `resolve` and `unresolve` look up for a comment.
fn lookup_of(id: &str) -> Value {
    json!({ "id": id })
}

/// A top-level comment on ENG-7, resolved by `resolving` when `resolved`.
fn thread(id: &str, resolved: bool, resolving: Option<&str>) -> Value {
    json!({
        "comment": {
            "url": format!("https://linear.app/acme/issue/ENG-7#comment-{id}"),
            "parentId": null,
            "resolvedAt": resolved.then_some("2026-01-03T00:00:00.000Z"),
            "resolvingCommentId": resolving,
            "issue": { "identifier": "ENG-7" }
        }
    })
}

/// A reply in thread `parent` on ENG-7.
fn reply_in(id: &str, parent: &str) -> Value {
    let mut reply = thread(id, false, None);
    reply["comment"]["parentId"] = json!(parent);
    reply
}

/// What `commentResolve` (`field`) or `commentUnresolve` answers for thread `id`.
fn changed(field: &str, id: &str, resolved: bool, resolving: Option<&str>) -> Value {
    json!({
        field: {
            "success": true,
            "comment": {
                "id": id,
                "resolvedAt": resolved.then_some("2026-01-04T00:00:00.000Z"),
                "resolvingCommentId": resolving
            }
        }
    })
}

fn mutations(api: &MockLinear) -> Vec<Value> {
    api.requests()
        .into_iter()
        .filter(|r| {
            matches!(
                r.operation.as_deref(),
                Some("ResolveComment" | "UnresolveComment")
            )
        })
        .map(|r| r.variables)
        .collect()
}

#[test]
fn resolve_resolves_a_thread_and_prints_it() {
    let api = MockLinear::start();
    api.on("GetCommentForResolution", thread(THREAD_A, false, None))
        .on(
            "ResolveComment",
            changed("commentResolve", THREAD_A, true, None),
        );
    Cli::for_api(&api)
        .run(&["issue", "comment", "resolve", THREAD_A])
        .success()
        .stdout_has(&format!(
            "✓ Resolved comment thread {THREAD_A} on ENG-7\nhttps://linear.app/acme/issue/ENG-7#comment-{THREAD_A}\n"
        ));
    assert_eq!(api.variables("ResolveComment"), json!({ "id": THREAD_A }));
}

#[test]
fn resolve_with_records_the_reply_that_resolved_the_thread() {
    for flag in ["--with", "--resolving-comment"] {
        let api = MockLinear::start();
        api.on_variables(
            "GetCommentForResolution",
            lookup_of(THREAD_A),
            thread(THREAD_A, false, None),
        )
        .on_variables(
            "GetCommentForResolution",
            lookup_of(REPLY),
            reply_in(REPLY, THREAD_A),
        )
        .on(
            "ResolveComment",
            changed("commentResolve", THREAD_A, true, Some(REPLY)),
        );
        Cli::for_api(&api)
            .run(&["issue", "comment", "resolve", THREAD_A, flag, REPLY])
            .success()
            .stdout_has("✓ Resolved comment thread");
        assert_eq!(
            api.variables("ResolveComment"),
            json!({ "id": THREAD_A, "resolvingCommentId": REPLY }),
            "{flag}"
        );
    }
}

#[test]
fn resolve_with_a_comment_outside_the_thread_changes_nothing() {
    // The thread is already resolved: a bad --with still fails rather than
    // passing as "already resolved".
    for (with, message) in [
        (
            reply_in(REPLY, THREAD_B),
            format!("Comment {REPLY} is a reply in thread {THREAD_B}, not {THREAD_A}"),
        ),
        (
            thread(REPLY, false, None),
            format!("Comment {REPLY} is not a reply"),
        ),
    ] {
        let api = MockLinear::start();
        api.on_variables(
            "GetCommentForResolution",
            lookup_of(THREAD_A),
            thread(THREAD_A, true, None),
        )
        .on_variables("GetCommentForResolution", lookup_of(REPLY), with);
        Cli::for_api(&api)
            .run(&["issue", "comment", "resolve", THREAD_A, "--with", REPLY])
            .failure()
            .stderr_has(&message);
        assert!(mutations(&api).is_empty());
    }
}

#[test]
fn resolve_with_another_reply_resolves_a_resolved_thread_again() {
    let api = MockLinear::start();
    api.on_variables(
        "GetCommentForResolution",
        lookup_of(THREAD_A),
        thread(THREAD_A, true, Some(THREAD_C)),
    )
    .on_variables(
        "GetCommentForResolution",
        lookup_of(REPLY),
        reply_in(REPLY, THREAD_A),
    )
    .on(
        "ResolveComment",
        changed("commentResolve", THREAD_A, true, Some(REPLY)),
    );
    Cli::for_api(&api)
        .run(&["issue", "comment", "resolve", THREAD_A, "--with", REPLY])
        .success()
        .stdout_has("✓ Resolved comment thread");
}

#[test]
fn resolve_with_needs_exactly_one_thread() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&[
            "issue", "comment", "resolve", THREAD_A, THREAD_B, "--with", REPLY,
        ])
        .usage_error()
        .stderr_has("--with takes exactly one thread");
    assert!(api.requests().is_empty());
}

#[test]
fn resolve_and_unresolve_refuse_a_reply_naming_its_thread() {
    for (command, fix) in [
        (
            "resolve",
            format!("`linear issue comment resolve {THREAD_B}`, adding `--with {THREAD_A}`"),
        ),
        (
            "unresolve",
            format!("`linear issue comment unresolve {THREAD_B}`"),
        ),
    ] {
        let api = MockLinear::start();
        api.on("GetCommentForResolution", reply_in(THREAD_A, THREAD_B));
        Cli::for_api(&api)
            .run(&["issue", "comment", command, THREAD_A])
            .failure()
            .stderr_has(&format!("Comment {THREAD_A} is a reply"))
            .stderr_has(&fix);
        assert!(mutations(&api).is_empty(), "{command}");
    }
}

#[test]
fn resolve_refuses_a_comment_that_is_not_on_an_issue() {
    let api = MockLinear::start();
    let mut comment = thread(THREAD_A, false, None);
    comment["comment"]["issue"] = Value::Null;
    api.on("GetCommentForResolution", comment);
    Cli::for_api(&api)
        .run(&["issue", "comment", "resolve", THREAD_A])
        .failure()
        .stderr_has(&format!("Comment {THREAD_A} is not on an issue"));
    assert!(mutations(&api).is_empty());
}

#[test]
fn resolve_and_unresolve_leave_a_thread_already_in_that_state() {
    for (command, resolved, state) in [("resolve", true, "resolved"), ("unresolve", false, "open")]
    {
        let api = MockLinear::start();
        api.on("GetCommentForResolution", thread(THREAD_A, resolved, None));
        Cli::for_api(&api)
            .run(&["issue", "comment", command, THREAD_A])
            .success()
            .stdout_has(&format!(
                "Comment thread {THREAD_A} on ENG-7 is already {state}.\n"
            ));
        assert_eq!(api.operations(), ["GetCommentForResolution"], "{command}");
    }
}

#[test]
fn resolve_refuses_urls_and_non_uuids_before_any_request() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api);
    cli.run(&["issue", "comment", "resolve", "not-a-uuid"])
        .usage_error()
        .stderr_has("Not a comment UUID: not-a-uuid");
    cli.run(&["issue", "comment", "resolve", THREAD_A, THREAD_B, "nope"])
        .usage_error()
        .stderr_has("Not a comment UUID: nope");
    cli.run(&["issue", "comment", "resolve", THREAD_A, "--with", "nope"])
        .usage_error();
    cli.run(&[
        "issue",
        "comment",
        "resolve",
        "https://linear.app/acme/issue/ENG-7/title#comment-a1111111",
    ])
    .failure()
    .stderr_has("only carries the first eight characters");
    cli.run(&["issue", "comment", "unresolve"])
        .usage_error()
        .stderr_has("No comment IDs given");
    assert!(api.requests().is_empty());
}

#[test]
fn resolve_names_an_unknown_comment() {
    let api = MockLinear::start();
    api.on_error("GetCommentForResolution", "Entity not found: Comment");
    Cli::for_api(&api)
        .run(&["issue", "comment", "resolve", THREAD_A])
        .not_found()
        .stderr_has(&format!("Comment not found: {THREAD_A}"));
}

#[test]
fn resolve_fails_when_linear_leaves_the_thread_open() {
    for payload in [
        changed("commentResolve", THREAD_A, false, None),
        json!({ "commentResolve": {
            "success": false,
            "comment": { "id": THREAD_A, "resolvedAt": null, "resolvingCommentId": null }
        } }),
    ] {
        let api = MockLinear::start();
        api.on("GetCommentForResolution", thread(THREAD_A, false, None))
            .on("ResolveComment", payload);
        Cli::for_api(&api)
            .run(&["issue", "comment", "resolve", THREAD_A])
            .failure()
            .stderr_has("Linear did not resolve the comment thread");
    }
}

#[test]
fn resolve_several_resolves_each_thread_once() {
    let api = MockLinear::start();
    for id in [THREAD_A, THREAD_B, THREAD_C] {
        api.on_variables(
            "GetCommentForResolution",
            lookup_of(id),
            thread(id, false, None),
        )
        .on_variables(
            "ResolveComment",
            json!({ "id": id }),
            changed("commentResolve", id, true, None),
        );
    }
    // The same thread in capitals is listed once.
    let shouting = THREAD_A.to_ascii_uppercase();
    Cli::for_api(&api)
        .stdin(THREAD_C.as_bytes())
        .run(&[
            "issue",
            "comment",
            "resolve",
            THREAD_A,
            THREAD_B,
            &shouting,
            "--bulk-stdin",
        ])
        .success()
        .stderr_has(&format!(
            "3 comment threads to resolve:\n  {THREAD_A} on ENG-7\n"
        ))
        .stdout_has("✓ Successfully resolved 3 comment threads");
    assert_eq!(mutations(&api).len(), 3);
}

#[test]
fn resolve_several_keeps_going_and_fails_at_the_end() {
    let api = MockLinear::start();
    api.on_variables(
        "GetCommentForResolution",
        lookup_of(THREAD_A),
        thread(THREAD_A, false, None),
    )
    .on_variables(
        "ResolveComment",
        json!({ "id": THREAD_A }),
        changed("commentResolve", THREAD_A, true, None),
    )
    .on_variables_error(
        "GetCommentForResolution",
        lookup_of(THREAD_B),
        "Entity not found: Comment",
    )
    .on_variables(
        "GetCommentForResolution",
        lookup_of(THREAD_C),
        thread(THREAD_C, false, None),
    )
    .on_variables_error(
        "ResolveComment",
        json!({ "id": THREAD_C }),
        "Comment is locked",
    );
    Cli::for_api(&api)
        .run(&["issue", "comment", "resolve", THREAD_A, THREAD_B, THREAD_C])
        .failure()
        .stdout_has("Completed: 1/3 comment threads resolved")
        .stdout_has(&format!("  - {THREAD_B}: Comment not found: {THREAD_B}"))
        .stdout_has(&format!(
            "  - {THREAD_C} ({THREAD_C} on ENG-7): Comment is locked"
        ));
}

#[test]
fn unresolve_reopens_a_thread() {
    for command in ["unresolve", "reopen"] {
        let api = MockLinear::start();
        api.on("GetCommentForResolution", thread(THREAD_A, true, None))
            .on(
                "UnresolveComment",
                changed("commentUnresolve", THREAD_A, false, None),
            );
        Cli::for_api(&api)
            .run(&["issue", "comment", command, THREAD_A])
            .success()
            .stdout_has(&format!("✓ Reopened comment thread {THREAD_A} on ENG-7\n"));
        assert_eq!(api.variables("UnresolveComment"), json!({ "id": THREAD_A }));
    }
}

/// A top-level comment by alice whose thread bob resolved.
fn resolved_root(id: &str, body: &str) -> Value {
    let mut root = comment(id, body, "alice", None);
    root["resolvedAt"] = json!("2026-01-03T00:00:00.000Z");
    root["resolvingUser"] = json!({ "id": "user-bob", "name": "bob", "displayName": "bob" });
    root
}

/// A reply in a thread whose top-level comment is resolved.
fn reply_in_resolved(id: &str, body: &str, parent: &str) -> Value {
    let mut reply = comment(id, body, "bob", Some(parent));
    reply["parent"]["resolvedAt"] = json!("2026-01-03T00:00:00.000Z");
    reply
}

/// A resolved thread (c1, c2), an open one (c3, c4), and a reply (c5) whose
/// resolved thread is not in the list.
fn mixed_threads() -> Vec<Value> {
    vec![
        resolved_root("c1", "Resolved root"),
        reply_in_resolved("c2", "Resolved reply", "c1"),
        comment("c3", "Open root", "alice", None),
        comment("c4", "Open reply", "bob", Some("c3")),
        reply_in_resolved("c5", "Orphan reply", "c9"),
    ]
}

fn ids(nodes: &[Value]) -> Vec<&str> {
    nodes
        .iter()
        .filter_map(|node| node["id"].as_str())
        .collect()
}

#[test]
fn list_json_shows_how_each_thread_was_resolved() {
    let api = MockLinear::start();
    let nodes = mixed_threads();
    api.on(
        "GetIssueComments",
        comments_page(nodes.clone(), Value::Null, false),
    );
    let listed = Cli::for_api(&api)
        .run(&["issue", "comment", "list", "ENG-7", "--json"])
        .success()
        .json_nodes();
    assert_eq!(listed, nodes);
    assert_eq!(listed[0]["resolvingUser"]["name"], "bob");
    assert_eq!(
        listed[1]["parent"]["resolvedAt"],
        "2026-01-03T00:00:00.000Z"
    );
}

#[test]
fn list_marks_resolved_threads() {
    for tty in [false, true] {
        let api = MockLinear::start();
        api.on(
            "GetIssueComments",
            comments_page(mixed_threads(), Value::Null, false),
        );
        let args = ["issue", "comment", "list", "ENG-7", "--no-pager"];
        let cli = Cli::for_api(&api);
        let run = if tty {
            cli.run_tty(&args, &[])
        } else {
            cli.run(&args)
        };
        run.success().stdout_has("[c1] [resolved]");
        assert!(!run.stdout.contains("[c3] [resolved]"), "{run}");
        assert!(!run.stdout.contains("[c2] [resolved]"), "{run}");
    }
}

#[test]
fn list_filters_whole_threads_by_resolution() {
    for (flag, expected) in [
        ("--resolved", vec!["c1", "c2", "c5"]),
        ("--unresolved", vec!["c3", "c4"]),
    ] {
        let api = MockLinear::start();
        api.on(
            "GetIssueComments",
            comments_page(mixed_threads(), Value::Null, false),
        );
        let listed = Cli::for_api(&api)
            .run(&["issue", "comment", "list", "ENG-7", flag, "--json"])
            .success()
            .json_nodes();
        assert_eq!(ids(&listed), expected, "{flag}");
    }
}

#[test]
fn list_filter_reads_every_page_before_applying_the_limit() {
    let api = MockLinear::start();
    api.on(
        "GetIssueComments",
        comments_page(
            vec![resolved_root("c1", "Resolved")],
            json!("cursor-1"),
            true,
        ),
    )
    .on(
        "GetIssueComments",
        comments_page(
            vec![
                comment("c3", "Open root", "alice", None),
                comment("c4", "Open reply", "bob", Some("c3")),
            ],
            Value::Null,
            false,
        ),
    );
    let listed = Cli::for_api(&api)
        .run(&[
            "issue",
            "comment",
            "list",
            "ENG-7",
            "--unresolved",
            "--limit",
            "1",
            "--json",
        ])
        .success()
        .json_nodes();
    assert_eq!(ids(&listed), ["c3"]);
    assert_eq!(api.requests()[0].variables["first"], 100);
}

#[test]
fn list_filter_says_when_no_thread_matches() {
    let api = MockLinear::start();
    api.on(
        "GetIssueComments",
        comments_page(vec![resolved_root("c1", "Resolved")], Value::Null, false),
    );
    Cli::for_api(&api)
        .run(&["issue", "comment", "list", "ENG-7", "--unresolved"])
        .success()
        .stdout_has("No open threads found for this issue\n");
}

#[test]
fn list_resolved_and_unresolved_conflict() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&[
            "issue",
            "comment",
            "list",
            "ENG-7",
            "--resolved",
            "--unresolved",
        ])
        .usage_error();
    assert!(api.requests().is_empty());
}
