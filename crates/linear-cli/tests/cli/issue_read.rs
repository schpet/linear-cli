//! Read-only `issue` commands: view, title, url, mine and query.
use serde_json::{Value, json};

use crate::support::{Cli, MockLinear, assert_json, nodes, plain_text, png};
use crate::team::{resolve_vars, resolved};

fn issue(comments: bool) -> Value {
    let mut issue = json!({
        "trashed": null, "archivedAt": null,
        "identifier": "ENG-1", "title": "Fix the widget",
        "description": "## Body\n\nPlain **bold** text.",
        "url": "https://linear.app/acme/issue/ENG-1/fix-the-widget",
        "branchName": "eng-1-fix-the-widget",
        "state": { "name": "Started", "color": "#123456" },
        "assignee": { "name": "alice", "displayName": "Alice Example" },
        "delegate": null,
        "priority": 2,
        "project": { "name": "Launch" },
        "projectMilestone": { "name": "Beta" },
        "cycle": {
            "id": "cycle-7", "number": 7, "name": "Sprint 7", "isActive": true,
            "isNext": false, "isPrevious": false, "isFuture": false, "isPast": false
        },
        "team": { "activeCycle": { "number": 7 } },
        "labels": { "nodes": [{ "id": "label-bug", "name": "Bug", "color": "#ff00ff" }] },
        "parent": {
            "trashed": null, "archivedAt": null,
            "identifier": "ENG-99", "title": "Parent work",
            "url": "https://linear.app/acme/issue/ENG-99/parent-work",
            "state": { "name": "Done", "color": "#123456" }
        },
        "children": { "nodes": [{
            "trashed": null, "archivedAt": null,
            "identifier": "ENG-2", "title": "Child work",
            "url": "https://linear.app/acme/issue/ENG-2/child-work",
            "state": { "name": "Started", "color": "#112233" }
        }] },
        "attachments": { "nodes": [{
            "id": "att-1", "title": "Design doc", "url": "https://example.com/docs",
            "subtitle": "Reference", "sourceType": "github", "metadata": { "nested": true },
            "createdAt": "2024-01-01T00:00:00.000Z"
        }] },
        "documents": { "nodes": [{
            "id": "doc-1", "title": "Spec", "slugId": "spec-123",
            "url": "https://linear.app/acme/document/spec-123",
            "createdAt": "2024-01-01T00:00:00.000Z", "updatedAt": "2024-01-01T00:00:00.000Z"
        }] },
        "creator": { "name": "sam", "displayName": "Sam Example" },
        "createdAt": "2024-01-01T00:00:00.000Z",
        "updatedAt": "2024-01-02T00:00:00.000Z"
    });
    if comments {
        issue["comments"] = json!({ "nodes": [
            comment("root", "Root comment body", None, None),
            comment("reply", "Reply body", Some("root"), None),
            comment("resolved", "Resolved thread body", None, Some("2024-01-02T00:00:00.000Z")),
        ] });
    }
    for key in ["labels", "children", "attachments", "documents", "comments"] {
        if let Some(connection) = issue.get_mut(key) {
            connection["pageInfo"] = json!({ "hasNextPage": false, "endCursor": null });
        }
    }
    issue
}

fn comment(id: &str, body: &str, parent: Option<&str>, resolved_at: Option<&str>) -> Value {
    json!({
        "id": id, "body": body, "quotedText": null, "createdAt": "2024-01-03T00:00:00.000Z",
        "url": format!("https://linear.app/acme/issue/ENG-1#comment-{id}"),
        "resolvedAt": resolved_at, "resolvingCommentId": null, "resolvingUser": null,
        "user": { "name": "alice", "displayName": "Alice Example" },
        "externalUser": null,
        "parent": parent.map(|id| json!({ "id": id }))
    })
}

fn details(issue: Value) -> Value {
    json!({ "issue": issue })
}

fn view_json(mut issue: Value) -> Value {
    issue["parent"].as_object_mut().unwrap().remove("url");
    for child in issue["children"]["nodes"].as_array_mut().unwrap() {
        child.as_object_mut().unwrap().remove("url");
    }
    issue
}

fn numbered_issue(number: u32, comments: bool) -> Value {
    let mut value = issue(comments);
    value["identifier"] = json!(format!("ENG-{number}"));
    value["title"] = json!(format!("Widget task {number}"));
    value["url"] = json!(format!("https://linear.app/acme/issue/ENG-{number}"));
    value["branchName"] = json!(format!("eng-{number}-widget-task"));
    value
}

#[test]
fn view_multiple_json_preserves_input_order_and_comment_selection() {
    for comments in [true, false] {
        let api = MockLinear::start();
        let expected: Vec<_> = [26, 21, 25, 22, 24, 23]
            .into_iter()
            .map(|number| numbered_issue(number, comments))
            .collect();
        let operation = if comments {
            "GetIssueDetailsWithComments"
        } else {
            "GetIssueDetails"
        };
        for value in expected.iter().rev() {
            api.on_variables(
                operation,
                json!({ "id": value["identifier"] }),
                details(value.clone()),
            );
        }
        let mut args = vec![
            "issue", "view", "ENG-26", "ENG-21", "ENG-25", "ENG-22", "ENG-24", "ENG-23", "--json",
        ];
        if !comments {
            args.push("--no-comments");
        }
        let run = Cli::for_api(&api).run(&args);
        let expected: Vec<_> = expected.into_iter().map(view_json).collect();
        assert_json(&run.success().json(), &json!(expected));
        assert!(run.stderr.is_empty(), "{run}");
        assert_eq!(api.operations().len(), expected.len());
    }
}

#[test]
fn view_multiple_text_separates_issues_and_applies_comment_options_to_each() {
    for (comments, resolved) in [(true, false), (true, true), (false, false)] {
        let api = MockLinear::start();
        let operation = if comments {
            "GetIssueDetailsWithComments"
        } else {
            "GetIssueDetails"
        };
        for number in [21, 22] {
            api.on_variables(
                operation,
                json!({ "id": format!("ENG-{number}") }),
                details(numbered_issue(number, comments)),
            );
        }
        let mut args = vec![
            "issue",
            "view",
            "ENG-22",
            "ENG-21",
            "--no-download",
            "--no-pager",
        ];
        if !comments {
            args.push("--no-comments");
        }
        if resolved {
            args.push("--show-resolved-threads");
        }
        let run = Cli::for_api(&api).run(&args);
        run.success();
        let first = run
            .stdout
            .find("# ENG-22: Widget task 22")
            .expect("first issue heading");
        let second = run
            .stdout
            .find("# ENG-21: Widget task 21")
            .expect("second issue heading");
        assert!(first < second, "{run}");
        assert!(run.stdout[first..second].contains("\n\n---\n\n"), "{run}");
        assert_eq!(
            run.stdout.matches("Root comment body").count(),
            if comments { 2 } else { 0 }
        );
        assert_eq!(
            run.stdout.matches("Resolved thread body").count(),
            if resolved { 2 } else { 0 }
        );
    }
}

#[test]
fn view_multiple_normalizes_mixed_references_and_retains_duplicates() {
    let api = MockLinear::start();
    let expected: Vec<_> = [21, 22, 21, 23]
        .into_iter()
        .map(|n| numbered_issue(n, false))
        .collect();
    for value in &expected {
        api.on_variables(
            "GetIssueDetails",
            json!({ "id": value["identifier"] }),
            details(value.clone()),
        );
    }
    let run = Cli::for_api(&api).env("LINEAR_TEAM_ID", "ENG").run(&[
        "issue",
        "v",
        "eng-21",
        "https://linear.app/acme/issue/eng-22/widget-task",
        "ENG-21",
        "23",
        "--json",
        "--no-comments",
    ]);
    let expected: Vec<_> = expected.into_iter().map(view_json).collect();
    assert_json(&run.success().json(), &json!(expected));
}

#[test]
fn view_multiple_rejects_invalid_later_input_before_requests() {
    for invalid in ["not an id!", "", " "] {
        let api = MockLinear::start();
        let run = Cli::for_api(&api).run(&["issue", "view", "ENG-21", invalid, "--json"]);
        run.usage_error();
        if !invalid.trim().is_empty() {
            run.stderr_has(invalid);
        }
        assert!(run.stdout.is_empty(), "{run}");
        assert!(api.requests().is_empty());
    }
}

#[test]
fn view_multiple_errors_name_the_issue_without_partial_output() {
    for json in [true, false] {
        for missing in [true, false] {
            let api = MockLinear::start();
            api.on_variables(
                "GetIssueDetailsWithComments",
                json!({ "id": "ENG-21" }),
                details(numbered_issue(21, true)),
            );
            if missing {
                api.on_variables(
                    "GetIssueDetailsWithComments",
                    json!({ "id": "ENG-22" }),
                    json!({ "issue": null }),
                );
            } else {
                api.on_variables_error(
                    "GetIssueDetailsWithComments",
                    json!({ "id": "ENG-22" }),
                    "Something went wrong",
                );
            }
            let mut args = vec!["issue", "view", "ENG-21", "ENG-22", "--no-download"];
            if json {
                args.push("--json");
            }
            let run = Cli::for_api(&api).run(&args);
            if missing {
                run.not_found().stderr_has("Issue not found: ENG-22");
            } else {
                run.failure().stderr_has("Something went wrong");
            }
            run.stderr_has("ENG-22");
            assert!(run.stdout.is_empty(), "{run}");
        }
    }
}

#[test]
fn view_multiple_finishes_each_issues_nested_pages() {
    let api = MockLinear::start();
    for number in [21, 22] {
        let mut value = numbered_issue(number, true);
        value["comments"]["pageInfo"] = json!({ "hasNextPage": true, "endCursor": "next" });
        api.on_variables(
            "GetIssueDetailsWithComments",
            json!({ "id": format!("ENG-{number}") }),
            details(value),
        );
        api.on_variables(
            "GetIssueCommentsPage",
            json!({ "id": format!("ENG-{number}"), "first": 100, "after": "next" }),
            json!({ "issue": { "comments": {
            "nodes": [comment("late", &format!("Late comment for {number}"), None, None)],
            "pageInfo": { "hasNextPage": false, "endCursor": null }
        } } }),
        );
    }
    let run = Cli::for_api(&api).run(&["issue", "view", "ENG-22", "ENG-21", "--json"]);
    let values = run.success().json_nodes();
    assert_eq!(
        values[0]["comments"]
            .as_array()
            .expect("comments")
            .last()
            .expect("late comment")["body"],
        "Late comment for 22"
    );
    assert_eq!(
        values[1]["comments"]
            .as_array()
            .expect("comments")
            .last()
            .expect("late comment")["body"],
        "Late comment for 21"
    );
}

#[test]
fn view_multiple_nested_page_failure_has_no_partial_output() {
    let api = MockLinear::start();
    api.on_variables(
        "GetIssueDetailsWithComments",
        json!({ "id": "ENG-21" }),
        details(numbered_issue(21, true)),
    );
    let mut value = numbered_issue(22, true);
    value["comments"]["pageInfo"] = json!({ "hasNextPage": true, "endCursor": "next" });
    api.on_variables(
        "GetIssueDetailsWithComments",
        json!({ "id": "ENG-22" }),
        details(value),
    );
    api.on_error("GetIssueCommentsPage", "Could not load comments");
    let run = Cli::for_api(&api).run(&["issue", "view", "ENG-21", "ENG-22", "--json"]);
    run.failure()
        .stderr_has("ENG-22")
        .stderr_has("Could not load comments");
    assert!(run.stdout.is_empty(), "{run}");
}

#[test]
fn view_multiple_terminal_output_uses_one_pager_unless_disabled() {
    for paging in [true, false] {
        let api = MockLinear::start();
        for number in [21, 22] {
            let mut value = numbered_issue(number, false);
            value["description"] = json!("A long widget description.\n\n".repeat(60));
            api.on_variables(
                "GetIssueDetails",
                json!({ "id": format!("ENG-{number}") }),
                details(value),
            );
        }
        let cli = Cli::for_api(&api)
            .stub_bin("sh", "exec /bin/sh \"$@\"")
            .stub_bin("test-pager", "cat");
        let pager = cli.path("bin/test-pager").display().to_string();
        let cli = cli.env("PAGER", &pager);
        let mut args = vec![
            "issue",
            "view",
            "ENG-22",
            "ENG-21",
            "--no-comments",
            "--no-download",
        ];
        if !paging {
            args.push("--no-pager");
        }
        let run = cli.run_tty(&args, &[]);
        run.success()
            .stdout_has("ENG-22: Widget task 22")
            .stdout_has("ENG-21: Widget task 21");
        assert_eq!(cli.calls("test-pager").len(), usize::from(paging));
        assert!(
            run.stdout.find("ENG-22: Widget task 22") < run.stdout.find("ENG-21: Widget task 21"),
            "{run}"
        );
    }
}

#[test]
fn view_multiple_downloads_each_issues_images_only_in_text_download_mode() {
    for mode in ["download", "--no-download", "--json"] {
        let api = MockLinear::start();
        let mut urls = Vec::new();
        for number in [21, 22] {
            let path = format!("/img/widget-{number}.png");
            let url = format!("{}{path}", api.base_url());
            let mut value = numbered_issue(number, false);
            value["description"] = json!(format!("![widget]({url})"));
            urls.push(url);
            api.on_variables(
                "GetIssueDetails",
                json!({ "id": format!("ENG-{number}") }),
                details(value),
            );
            if mode == "download" {
                api.on_http("GET", &path, 200, format!("IMAGE-{number}").as_bytes());
            }
        }
        let cli = Cli::for_api(&api);
        let mut args = vec![
            "issue",
            "view",
            "ENG-21",
            "ENG-22",
            "--no-comments",
            "--no-pager",
        ];
        if mode != "download" {
            args.push(mode);
        }
        let run = cli.run(&args);
        run.success();
        let files = files_under(&cli.path("home/.cache/linear-cli/images"));
        if mode == "download" {
            assert_eq!(files.len(), 2);
            for file in files {
                run.stdout_has(&file.display().to_string());
            }
            for url in urls {
                assert!(!run.stdout.contains(&url), "{run}");
            }
        } else {
            assert!(files.is_empty());
            for url in urls {
                run.stdout_has(&url);
            }
            assert!(api.requests().iter().all(|request| request.method != "GET"));
        }
    }
}

#[test]
fn view_json_prints_the_issue_with_comments() {
    let api = MockLinear::start();
    api.on("GetIssueDetailsWithComments", details(issue(true)));
    let json = Cli::for_api(&api)
        .run(&["issue", "view", "eng-1", "--json"])
        .success()
        .json();
    assert_json(&json, &view_json(issue(true)));
    assert_eq!(
        api.variables("GetIssueDetailsWithComments"),
        json!({ "id": "ENG-1" })
    );
}

#[test]
fn view_bare_number_uses_the_configured_team() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details(issue(false)));
    let json = Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .run(&["issue", "view", "1", "--json", "--no-comments"])
        .success()
        .json();
    assert_json(&json, &view_json(issue(false)));
    assert_eq!(api.variables("GetIssueDetails"), json!({ "id": "ENG-1" }));
}

#[test]
fn view_alias_accepts_an_issue_url() {
    let api = MockLinear::start();
    api.on("GetIssueDetailsWithComments", details(issue(true)));
    Cli::for_api(&api)
        .run(&[
            "issue",
            "v",
            "https://linear.app/acme/issue/eng-1/fix-the-widget",
            "--json",
        ])
        .success();
    assert_eq!(
        api.variables("GetIssueDetailsWithComments"),
        json!({ "id": "ENG-1" })
    );
}

#[test]
fn view_text_shows_issue_and_open_threads() {
    let api = MockLinear::start();
    api.on("GetIssueDetailsWithComments", details(issue(true)));
    let run = Cli::for_api(&api).run(&["issue", "view", "ENG-1", "--no-download", "--no-pager"]);
    run.success()
        .stdout_has("ENG-1")
        .stdout_has("Fix the widget")
        .stdout_has("Plain")
        .stdout_has("Root comment body")
        .stdout_has("Reply body");
    assert!(!run.stdout.contains("Resolved thread body"), "{run}");
}

/// An issue whose one thread is anchored to a passage of its description,
/// with a reply that quotes nothing.
fn issue_with_anchored_thread() -> Value {
    let mut issue = issue(true);
    let mut root = comment("anchored", "Worth doing?", None, None);
    root["quotedText"] = json!("First quoted line\nsecond quoted line.");
    issue["comments"]["nodes"] = json!([root, comment("reply", "Agreed", Some("anchored"), None)]);
    issue
}

#[test]
fn view_text_quotes_the_description_passage_a_thread_is_anchored_to() {
    let api = MockLinear::start();
    api.on(
        "GetIssueDetailsWithComments",
        details(issue_with_anchored_thread()),
    );
    let run = Cli::for_api(&api).run(&["issue", "view", "ENG-1", "--no-download", "--no-pager"]);
    run.success().stdout_has(
        "[anchored]\n\n  > First quoted line\n  > second quoted line.\n  \n  Worth doing?\n\n  - **@Alice Example** replied",
    );
    assert!(!run.stdout.contains("> Agreed"), "{run}");
}

#[test]
fn view_tty_quotes_the_description_passage_a_thread_is_anchored_to() {
    let api = MockLinear::start();
    api.on(
        "GetIssueDetailsWithComments",
        details(issue_with_anchored_thread()),
    );
    let run = Cli::for_api(&api).run_tty(
        &["issue", "view", "ENG-1", "--no-download", "--no-pager"],
        &[],
    );
    run.success()
        .stdout_has("First quoted line")
        .stdout_has("second quoted line.");
    let quote = run.stdout.find("First quoted line").expect("quote shown");
    let body = run.stdout.find("Worth doing?").expect("body shown");
    assert!(quote < body, "the quote comes before the comment: {run}");
}

#[test]
fn view_text_can_include_resolved_threads() {
    let api = MockLinear::start();
    api.on("GetIssueDetailsWithComments", details(issue(true)));
    Cli::for_api(&api)
        .run(&[
            "issue",
            "view",
            "ENG-1",
            "--no-download",
            "--no-pager",
            "--show-resolved-threads",
        ])
        .success()
        .stdout_has("Resolved thread body");
}

#[test]
fn view_text_without_comments() {
    let api = MockLinear::start();
    api.on("GetIssueDetails", details(issue(false)));
    Cli::for_api(&api)
        .run(&[
            "issue",
            "view",
            "ENG-1",
            "--no-download",
            "--no-comments",
            "--no-pager",
        ])
        .success()
        .stdout_has("Fix the widget");
    assert_eq!(api.operations(), ["GetIssueDetails"]);
}

#[test]
fn view_shows_the_delegate_only_when_there_is_one() {
    for (operation, comments) in [
        ("GetIssueDetailsWithComments", true),
        ("GetIssueDetails", false),
    ] {
        let mut delegated = issue(comments);
        delegated["delegate"] = json!({ "name": "Linear", "displayName": "linear" });
        let api = MockLinear::start();
        api.on(operation, details(delegated.clone()))
            .on(operation, details(delegated.clone()))
            .on(operation, details(issue(comments)));
        let cli = Cli::for_api(&api);
        let mut args = vec!["issue", "view", "ENG-1", "--no-download", "--no-pager"];
        if !comments {
            args.push("--no-comments");
        }
        let json = cli
            .run(&[args.as_slice(), &["--json"]].concat())
            .success()
            .json();
        assert_json(&json, &view_json(delegated));
        cli.run(&args)
            .success()
            .stdout_has("**Assignee:** @Alice Example\n**Delegate:** @linear\n**Project:**");
        let plain = cli.run(&args);
        assert!(!plain.success().stdout.contains("Delegate"), "{plain}");
        for request in api.requests() {
            assert!(request.query.contains("delegate {"), "{}", request.query);
        }
    }
}

#[test]
fn view_puts_each_metadata_field_on_its_own_line() {
    for (operation, comments) in [
        ("GetIssueDetailsWithComments", true),
        ("GetIssueDetails", false),
    ] {
        let api = MockLinear::start();
        api.on(operation, details(issue(comments)));
        let mut args = vec!["issue", "view", "ENG-1", "--no-download"];
        if !comments {
            args.push("--no-comments");
        }
        let run = Cli::for_api(&api).run(&args);
        run.success().stdout_has(
            "# ENG-1: Fix the widget\n\n\
             **State:** Started\n\
             **Priority:** ▄▆█\n\
             **Assignee:** @Alice Example\n\
             **Project:** Launch\n\
             **Milestone:** Beta\n\
             **Cycle:** #7 Sprint 7 (now)\n\
             **Creator:** @Sam Example\n\
             **Created:** 2024-01-01\n\
             **Updated:** 2024-01-02\n\n\
             ## Body",
        );
        let query = &api.requests()[0].query;
        for field in ["creator {", "createdAt", "updatedAt"] {
            assert!(query.contains(field), "{field} missing from {query}");
        }
    }
}

#[test]
fn view_names_the_creator_by_display_name_then_name() {
    for (creator, line) in [
        (
            json!({ "name": "sam", "displayName": "" }),
            Some("**Creator:** @sam\n"),
        ),
        (json!({ "name": "", "displayName": "" }), None),
        (Value::Null, None),
    ] {
        let mut created = issue(false);
        created["creator"] = creator;
        let api = MockLinear::start();
        api.on("GetIssueDetails", details(created));
        let run = Cli::for_api(&api).run(&["issue", "view", "ENG-1", "--no-comments"]);
        match line {
            Some(line) => run.success().stdout_has(line),
            None => {
                run.success()
                    .stdout_has("**Cycle:** #7 Sprint 7 (now)\n**Created:** 2024-01-01\n");
                assert!(!run.stdout.contains("Creator"), "{run}");
                &run
            }
        };
    }
}

#[test]
fn view_tty_shows_metadata_on_separate_lines() {
    let api = MockLinear::start();
    api.on("GetIssueDetailsWithComments", details(issue(true)));
    let run = Cli::for_api(&api).run_tty(
        &["issue", "view", "ENG-1", "--no-download", "--no-pager"],
        &[],
    );
    run.success()
        .stdout_has("State: Started\n")
        .stdout_has("Assignee: @Alice Example\n")
        .stdout_has("Creator: @Sam Example\nCreated: 2024-01-01\nUpdated: 2024-01-02\n");
    assert!(!run.stdout.contains(" | "), "{run}");
}

#[test]
fn view_rejects_an_unparseable_id_before_any_request() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&["issue", "view", "not an id!"])
        .usage_error()
        .stderr_has("issue ID");
    assert!(api.requests().is_empty());
}

#[test]
fn view_reports_api_errors() {
    let api = MockLinear::start();
    api.on_error("GetIssueDetailsWithComments", "Rate limit exceeded");
    Cli::for_api(&api)
        .run(&["issue", "view", "ENG-1", "--json"])
        .failure()
        .stderr_has("Rate limit exceeded");
}

const ISSUE_NOT_FOUND: &str = r#"{"errors":[{"message":"Entity not found: Issue","extensions":{"userPresentableMessage":"Could not find referenced Issue."}}]}"#;

#[test]
fn view_and_title_name_a_missing_issue() {
    let api = MockLinear::start();
    api.on_raw("GetIssueDetailsWithComments", 200, ISSUE_NOT_FOUND)
        .on_raw("GetIssueDetails", 200, ISSUE_NOT_FOUND);
    let cli = Cli::for_api(&api);
    cli.run(&["issue", "view", "ENG-9999"])
        .not_found()
        .stderr_has("Issue not found: ENG-9999");
    let run = cli.run(&["issue", "title", "ENG-9999"]);
    run.not_found().stderr_has("Issue not found: ENG-9999");
    assert!(!run.stderr.contains("Could not find"), "{run}");
}

#[test]
fn title_and_url_print_single_fields() {
    let mut issue = issue(false);
    issue["team"]["key"] = json!("ENG");
    let api = MockLinear::start();
    api.on("GetIssueDetails", details(issue.clone()))
        .on("GetIssueDetails", details(issue));
    let cli = Cli::for_api(&api).env("LINEAR_TEAM_ID", "eng");
    assert_eq!(
        cli.run(&["issue", "title", "1"])
            .success()
            .stdout
            .trim_end(),
        "Fix the widget"
    );
    assert_eq!(
        cli.run(&["issue", "url", "eng-1"])
            .success()
            .stdout
            .trim_end(),
        "https://linear.app/acme/issue/ENG-1/fix-the-widget"
    );
    let ids: Vec<Value> = api.requests().into_iter().map(|r| r.variables).collect();
    assert_eq!(ids, [json!({ "id": "ENG-1" }), json!({ "id": "ENG-1" })]);
}

/// A node with the fields both `issue mine` and `issue query` select.
fn list_issue(number: u32, team: &str, state_type: &str) -> Value {
    json!({
        "id": format!("issue-{team}-{number}"),
        "trashed": null, "archivedAt": null,
        "identifier": format!("{team}-{number}"),
        "title": format!("Issue {number} title"),
        "url": format!("https://linear.app/acme/issue/{team}-{number}"),
        "priority": 2, "priorityLabel": "High", "estimate": null,
        "createdAt": "2024-01-01T00:00:00.000Z", "updatedAt": "2024-01-01T00:00:00.000Z",
        "state": {
            "id": format!("state-{state_type}"), "name": state_type, "color": "#112233",
            "type": state_type, "position": 1
        },
        "assignee": null,
        "team": {
            "id": format!("team-{team}"), "key": team, "name": format!("{team} Team"),
            "cyclesEnabled": false, "activeCycle": null
        },
        "project": null, "projectMilestone": null, "cycle": null,
        "labels": { "nodes": [] },
        "inverseRelations": { "nodes": [] }
    })
}

fn issues(nodes: Vec<Value>, end_cursor: Option<&str>) -> Value {
    json!({ "issues": {
        "nodes": nodes,
        "pageInfo": { "hasNextPage": end_cursor.is_some(), "endCursor": end_cursor }
    } })
}

fn default_sort() -> Value {
    json!([
        { "workflowState": { "order": "Ascending" } },
        { "priority": { "nulls": "last", "order": "Descending" } },
        { "manual": { "nulls": "last", "order": "Ascending" } }
    ])
}

fn identifiers(json: &Value) -> Vec<String> {
    let mut ids: Vec<String> = nodes(json)
        .iter()
        .map(|node| node["identifier"].as_str().expect("identifier").to_owned())
        .collect();
    ids.sort();
    ids
}

#[test]
fn removed_listing_command_explains_the_replacements_without_requests() {
    let api = MockLinear::start();
    let cli = Cli::new().endpoint(&api);
    for args in [
        &["issue", "list"][..],
        &["issue", "list", "--help"],
        &["issue", "list", "-h"],
        &[
            "issue", "list", "--json", "--team", "ENG", "--state", "started",
        ],
        &["issue", "list", "-A", "-w"],
        &["i", "list", "--assignee", "@me"],
        &["issue", "l"],
        &["i", "l", "-A", "--sort", "priority"],
    ] {
        let run = cli.run(args);
        run.usage_error()
            .stderr_has("`issue list` was removed in 4.0")
            .stderr_has("linear issue mine")
            .stderr_has("linear issue query");
        assert!(run.stdout.is_empty(), "{run}");
    }
    assert!(api.requests().is_empty());
    let help = cli.run(&["issue", "--help"]);
    help.success();
    let names: Vec<_> = help
        .stdout
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .collect();
    for name in ["mine", "query"] {
        assert!(names.contains(&name), "{help}");
    }
    for name in ["list", "l"] {
        assert!(!names.contains(&name), "{help}");
    }
}

#[test]
fn mine_shows_my_unstarted_issues_in_the_configured_team() {
    let api = MockLinear::start();
    api.on(
        "GetIssuesForState",
        issues(vec![list_issue(1, "ENG", "unstarted")], None),
    );
    Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .run(&["issue", "mine"])
        .success()
        .stdout_has("ENG-1")
        .stdout_has("Issue 1 title");
    assert_eq!(
        api.variables("GetIssuesForState"),
        json!({
            "sort": default_sort(),
            "filter": {
                "team": { "key": { "eq": "ENG" } },
                "state": { "type": { "in": ["unstarted"] } },
                "assignee": { "isMe": { "eq": true } }
            },
            "first": 50
        })
    );
}

#[test]
fn mine_json_matches_query_with_both_json_flags() {
    let mut row = list_issue(1, "ENG", "unstarted");
    row["labels"]["nodes"] = json!([{ "id": "label-1", "name": "Bug", "color": "#123456" }]);
    row["inverseRelations"]["nodes"] = json!([{
        "id": "relation-1", "type": "blocks",
        "issue": { "id": "issue-2", "identifier": "ENG-2", "state": { "type": "started" } }
    }]);
    for flag in ["--json", "-j"] {
        let api = MockLinear::start();
        api.on("GetIssuesForState", issues(vec![row.clone()], None))
            .on("GetIssuesForQuery", issues(vec![row.clone()], None));
        let cli = Cli::for_api(&api).env("LINEAR_TEAM_ID", "ENG");
        let list = cli.run(&["issue", "mine", flag]);
        list.success();
        assert_json(&list.json(), &json!([row.clone()]));
        assert!(list.stderr.is_empty(), "{list}");
        assert!(list.stdout.ends_with('\n'), "{list}");
        let query = cli.run(&["issue", "query", "--json"]);
        query.success();
        assert_eq!(list.stdout, query.stdout);
        assert_eq!(
            api.variables("GetIssuesForState")["filter"],
            json!({
                "team": { "key": { "eq": "ENG" } },
                "state": { "type": { "in": ["unstarted"] } },
                "assignee": { "isMe": { "eq": true } }
            })
        );
    }
}

#[test]
fn mine_json_prints_an_empty_array() {
    let api = MockLinear::start();
    api.on("GetIssuesForState", issues(vec![], None));
    let run =
        Cli::for_api(&api)
            .env("LINEAR_TEAM_ID", "ENG")
            .run(&["issue", "mine", "-j", "--no-pager"]);
    run.success();
    assert_eq!(run.stdout, "[]\n");
}

#[test]
fn mine_json_collects_and_orders_all_pages() {
    let api = MockLinear::start();
    let first = list_issue(1, "ENG", "unstarted");
    let second = list_issue(2, "ENG", "started");
    api.on(
        "GetIssuesForState",
        issues(vec![first.clone()], Some("next")),
    )
    .on("GetIssuesForState", issues(vec![second.clone()], None));
    let run = Cli::for_api(&api).env("LINEAR_TEAM_ID", "ENG").run(&[
        "issue",
        "mine",
        "--json",
        "--all-states",
        "--limit",
        "all",
    ]);
    run.success();
    assert_json(&run.json(), &json!([second, first]));
    let requests = api.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests.last().expect("second page").variables["after"],
        json!("next")
    );
}

#[test]
fn mine_json_stops_at_the_limit() {
    let api = MockLinear::start();
    let row = list_issue(1, "ENG", "unstarted");
    api.on("GetIssuesForState", issues(vec![row.clone()], Some("next")));
    let run = Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .run(&["issue", "mine", "--json", "--limit", "1"]);
    run.success();
    assert_json(&run.json(), &json!([row]));
    assert_eq!(api.requests().len(), 1);
    assert_eq!(api.variables("GetIssuesForState")["first"], json!(1));
}

#[test]
fn mine_json_reports_api_errors_without_partial_output() {
    let api = MockLinear::start();
    api.on(
        "GetIssuesForState",
        issues(vec![list_issue(1, "ENG", "unstarted")], Some("next")),
    )
    .on_error("GetIssuesForState", "Access denied");
    let run = Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .run(&["issue", "mine", "--json", "--limit", "all"]);
    run.failure();
    run.stderr_has("Access denied");
    assert!(run.stdout.is_empty(), "{run}");
}

#[test]
fn mine_json_conflicts_with_opening_the_list() {
    for flag in ["--web", "--app"] {
        let api = MockLinear::start();
        Cli::for_api(&api)
            .run(&["issue", "mine", "--json", flag])
            .usage_error()
            .stderr_has("cannot be used with");
        assert!(api.requests().is_empty());
    }
}

fn label_nodes(team: Option<&str>) -> Value {
    let team = team.map(|key| json!({ "key": key, "name": key }));
    json!({ "issueLabels": { "nodes": [
        { "id": "label-1", "name": "Bug", "color": "#ff0000", "team": team }
    ] } })
}

#[test]
fn mine_sends_filters() {
    let api = MockLinear::start();
    api.on("GetLabelByName", label_nodes(Some("ENG")))
        .on("GetLabelByName", label_nodes(None))
        .on("GetIssuesForState", issues(vec![], None));
    let project = "f0000000-0000-4000-8000-000000000002";
    let milestone = "f0000000-0000-4000-8000-000000000003";
    Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .run(&[
            "issue",
            "mine",
            "--project",
            project,
            "--milestone",
            milestone,
            "--label",
            "Bug",
            "-l",
            "UI",
            "--created-after",
            "2024-01-01",
            "--updated-after",
            "2024-01-02T03:00:00.000Z",
            "--limit",
            "1",
        ])
        .success();
    assert_eq!(
        api.variables("GetIssuesForState"),
        json!({
            "sort": default_sort(),
            "filter": {
                "team": { "key": { "eq": "ENG" } },
                "state": { "type": { "in": ["unstarted"] } },
                "assignee": { "isMe": { "eq": true } },
                "project": { "id": { "eq": project } },
                "projectMilestone": { "id": { "eq": milestone } },
                "labels": { "and": [
                    { "some": { "name": { "eqIgnoreCase": "Bug" } } },
                    { "some": { "name": { "eqIgnoreCase": "UI" } } }
                ] },
                "createdAt": { "gte": "2024-01-01T00:00:00.000Z" },
                "updatedAt": { "gte": "2024-01-02T03:00:00.000Z" }
            },
            "first": 1
        })
    );
    let names: Vec<Value> = api
        .requests()
        .into_iter()
        .filter(|request| request.operation.as_deref() == Some("GetLabelByName"))
        .map(|request| request.variables["name"].clone())
        .collect();
    assert_eq!(names, [json!("Bug"), json!("UI")]);
}

#[test]
fn mine_and_query_reject_a_label_the_issues_cannot_have() {
    let api = MockLinear::start();
    api.on("GetLabelByName", json!({ "issueLabels": { "nodes": [] } }))
        .on("GetLabelByName", label_nodes(Some("OPS")))
        .on("GetLabelByName", json!({ "issueLabels": { "nodes": [] } }));
    let cli = Cli::for_api(&api).env("LINEAR_TEAM_ID", "ENG");
    cli.run(&["issue", "mine", "--label", "nope"])
        .not_found()
        .stderr_has("Issue label not found: nope")
        .stderr_has("linear label list --team ENG");
    cli.run(&["issue", "mine", "--label", "Bug"])
        .not_found()
        .stderr_has("Issue label not found: Bug");
    cli.run(&["issue", "query", "--all-teams", "--label", "nope"])
        .not_found()
        .stderr_has("Issue label not found: nope")
        .stderr_has("linear label list --all-teams");
    assert_eq!(
        api.operations(),
        ["GetLabelByName", "GetLabelByName", "GetLabelByName"]
    );
}

#[test]
fn mine_and_query_resolve_sort_flags_over_configuration() {
    let manual = json!([
        { "workflowState": { "order": "Ascending" } },
        { "manual": { "nulls": "last", "order": "Ascending" } }
    ]);
    for (command, operation) in [
        ("mine", "GetIssuesForState"),
        ("query", "GetIssuesForQuery"),
    ] {
        for flag in [None, Some("manual"), Some("priority")] {
            let api = MockLinear::start();
            api.on(operation, issues(vec![], None));
            let cli = Cli::for_api(&api)
                .env("LINEAR_TEAM_ID", "ENG")
                .file("cwd/.linear.toml", "issue_sort = 'manual'\n");
            let mut args = vec!["issue", command, "--json"];
            if let Some(flag) = flag {
                args.extend(["--sort", flag]);
            }
            cli.run(&args).success();
            let mut filter = json!({ "team": { "key": { "eq": "ENG" } } });
            if command == "mine" {
                filter["state"] = json!({ "type": { "in": ["unstarted"] } });
                filter["assignee"] = json!({ "isMe": { "eq": true } });
            }
            assert_eq!(
                api.variables(operation),
                json!({
                    "sort": if flag == Some("priority") { default_sort() } else { manual.clone() },
                    "filter": filter,
                    "first": 50
                }),
                "{command} with {flag:?}"
            );
        }
    }
}

#[test]
fn mine_unlimited_follows_pages() {
    let api = MockLinear::start();
    api.on(
        "GetIssuesForState",
        issues(vec![list_issue(1, "ENG", "started")], Some("cursor-1")),
    )
    .on(
        "GetIssuesForState",
        issues(vec![list_issue(2, "ENG", "backlog")], None),
    );
    Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .run(&[
            "issue",
            "mine",
            "--all-states",
            "--limit",
            "all",
            "--sort",
            "manual",
        ])
        .success()
        .stdout_has("ENG-1")
        .stdout_has("ENG-2");
    let base = json!({
        "sort": [
            { "workflowState": { "order": "Ascending" } },
            { "manual": { "nulls": "last", "order": "Ascending" } }
        ],
        "filter": {
            "team": { "key": { "eq": "ENG" } },
            "assignee": { "isMe": { "eq": true } }
        },
        "first": 100
    });
    let mut second = base.clone();
    second["after"] = json!("cursor-1");
    let variables: Vec<Value> = api.requests().into_iter().map(|r| r.variables).collect();
    assert_eq!(variables, [base, second]);
}

#[test]
fn mine_resolves_team_cycle_and_state_names() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved("team-eng", "ENG", "Engineering"))
        .on(
            "GetTeamCyclesForLookup",
            json!({ "team": {
                "key": "ENG", "cyclesEnabled": true,
                "cycles": {
                    "nodes": [{
                        "id": "cycle-active", "number": 7, "name": "Sprint",
                        "startsAt": "2024-01-01T00:00:00.000Z", "isNext": false, "isPrevious": false
                    }],
                    "pageInfo": { "hasNextPage": false, "endCursor": null }
                },
                "activeCycle": { "id": "cycle-active", "number": 7, "name": "Sprint" }
            } }),
        )
        .on(
            "GetWorkflowStatesInScope",
            json!({ "workflowStates": {
                "nodes": [{
                    "id": "state-ready", "name": "Ready", "type": "unstarted",
                    "position": 1, "team": { "key": "ENG" }
                }],
                "pageInfo": { "hasNextPage": false, "endCursor": null }
            } }),
        )
        .on("GetIssuesForState", issues(vec![], None));
    Cli::for_api(&api)
        .run(&[
            "issue", "mine", "--team", "eng", "--cycle", "active", "--state", "started", "--state",
            "Ready",
        ])
        .success();
    assert_eq!(api.variables("ResolveTeam"), resolve_vars("eng"));
    assert_eq!(
        api.variables("GetTeamCyclesForLookup")["teamId"],
        json!("team-eng")
    );
    assert_eq!(
        api.variables("GetWorkflowStatesInScope")["filter"],
        json!({ "team": { "key": { "in": ["ENG"] } } })
    );
    assert_eq!(
        api.variables("GetIssuesForState")["filter"],
        json!({
            "team": { "key": { "eq": "ENG" } },
            "state": { "or": [
                { "type": { "in": ["started"] } },
                { "id": { "in": ["state-ready"] } }
            ] },
            "assignee": { "isMe": { "eq": true } },
            "cycle": { "id": { "eq": "cycle-active" } }
        })
    );
}

#[test]
fn mine_filters_by_another_assignee_or_none() {
    let api = MockLinear::start();
    api.on(
        "LookupUser",
        json!({ "users": { "nodes": [{
            "id": "user-ada", "email": "ada@example.com", "displayName": "ada", "name": "Ada Lovelace"
        }] } }),
    )
    .on("GetIssuesForState", issues(vec![], None))
    .on("GetIssuesForState", issues(vec![], None))
    .on("GetIssuesForState", issues(vec![], None));
    let cli = Cli::for_api(&api).env("LINEAR_TEAM_ID", "ENG");
    let assignee = |args: &[&str]| {
        let mut argv = vec!["issue", "mine"];
        argv.extend(args);
        cli.run(&argv).success();
        let requests = api.requests();
        let last = requests
            .iter()
            .rev()
            .find(|r| r.operation.as_deref() == Some("GetIssuesForState"))
            .expect("an issue mine request");
        last.variables["filter"].get("assignee").cloned()
    };
    assert_eq!(
        assignee(&["--assignee", "ada"]),
        Some(json!({ "id": { "eq": "user-ada" } }))
    );
    assert_eq!(assignee(&["-U"]), Some(json!({ "null": true })));
    assert_eq!(assignee(&["--all-assignees"]), None);
    assert_eq!(api.variables("LookupUser"), json!({ "input": "ada" }));
}

#[test]
fn query_takes_yourself_as_at_me_or_self_without_a_lookup() {
    let api = MockLinear::start();
    api.on("GetIssuesForQuery", issues(vec![], None))
        .on("GetIssuesForQuery", issues(vec![], None));
    let cli = Cli::for_api(&api).env("LINEAR_TEAM_ID", "ENG");
    for me in ["@me", "self"] {
        cli.run(&["issue", "query", "--assignee", me, "--json"])
            .success();
    }
    for request in api.requests() {
        assert_eq!(
            request.variables["filter"]["assignee"],
            json!({ "isMe": { "eq": true } })
        );
    }
}

#[test]
fn user_flags_refuse_linear_urls_before_any_request() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api).env("LINEAR_TEAM_ID", "ENG");
    let url = "https://linear.app/acme/profiles/ada";
    for args in [
        &["issue", "mine", "--assignee", url][..],
        &["issue", "update", "ENG-1", "-a", url],
        &["project", "create", "-n", "P", "--lead", url],
        &["initiative", "list", "--owner", url],
    ] {
        cli.run(args).usage_error().stderr_has("Linear URL");
    }
    assert!(api.requests().is_empty());
}

#[test]
fn mine_assignee_filters_conflict_with_each_other() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api).env("LINEAR_TEAM_ID", "ENG");
    for args in [
        &["--assignee", "ada", "--unassigned"][..],
        &["--assignee", "ada", "-A"],
        &["-A", "-U"],
    ] {
        let mut argv = vec!["issue", "mine"];
        argv.extend(args);
        cli.run(&argv).usage_error();
    }
    assert!(api.requests().is_empty());
}

#[test]
fn mine_rejects_a_url_that_is_not_a_cycle() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved("team-eng", "ENG", "Engineering"));
    Cli::for_api(&api)
        .run(&[
            "issue",
            "mine",
            "--team",
            "eng",
            "--cycle",
            "https://linear.app/acme/issue/ENG-1/wrong",
        ])
        .failure()
        .stderr_has("cycle URL, number, or name");
}

#[test]
fn mine_validation_fails_before_any_request() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api);
    cli.run(&["issue", "mine"])
        .usage_error()
        .stderr_has("No team given and no default team configured")
        .stderr_has("--team");
    let cli = cli.env("LINEAR_TEAM_ID", "ENG");
    cli.run(&["issue", "mine", "--created-after", "nope"])
        .usage_error()
        .stderr_has("--created-after");
    cli.run(&["issue", "mine", "--sort", "bogus"]).usage_error();
    assert!(api.requests().is_empty());
}

#[test]
fn query_json_defaults_to_the_configured_team() {
    let api = MockLinear::start();
    api.on(
        "GetIssuesForQuery",
        issues(vec![list_issue(1, "ENG", "started")], None),
    );
    let run = Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .run(&["issue", "query", "--json"]);
    let json = run.success().json();
    assert_eq!(identifiers(&json), ["ENG-1"]);
    assert_eq!(nodes(&json)[0]["title"], "Issue 1 title");
    run.stderr_has("ENG");
    assert_eq!(
        api.variables("GetIssuesForQuery"),
        json!({
            "sort": default_sort(),
            "first": 50,
            "filter": { "team": { "key": { "eq": "ENG" } } }
        })
    );
}

#[test]
fn query_all_teams_unlimited_follows_pages() {
    let api = MockLinear::start();
    api.on(
        "GetIssuesForQuery",
        issues(
            vec![
                list_issue(1, "OPS", "completed"),
                list_issue(2, "ENG", "started"),
            ],
            Some("cursor-1"),
        ),
    )
    .on(
        "GetIssuesForQuery",
        issues(vec![list_issue(3, "OPS", "started")], None),
    );
    let json = Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .run(&[
            "issue",
            "query",
            "--all-teams",
            "--all-states",
            "--json",
            "--limit",
            "all",
            "--include-archived",
        ])
        .success()
        .json();
    assert_eq!(identifiers(&json), ["ENG-2", "OPS-1", "OPS-3"]);
    let base = json!({ "sort": default_sort(), "first": 100, "includeArchived": true });
    let mut second = base.clone();
    second["after"] = json!("cursor-1");
    let variables: Vec<Value> = api.requests().into_iter().map(|r| r.variables).collect();
    assert_eq!(variables, [base, second]);
}

#[test]
fn query_search_pages_until_the_limit() {
    let api = MockLinear::start();
    let search = |nodes: Vec<Value>, end_cursor: Option<&str>| {
        // Search results also carry match metadata.
        let nodes: Vec<Value> = nodes
            .into_iter()
            .map(|mut node| {
                node["metadata"] = json!({ "source": "title" });
                node
            })
            .collect();
        json!({ "searchIssues": {
            "nodes": nodes,
            "pageInfo": { "hasNextPage": end_cursor.is_some(), "endCursor": end_cursor },
            "totalCount": 9
        } })
    };
    api.on(
        "SearchIssues",
        search(vec![list_issue(1, "ENG", "completed")], Some("cursor-1")),
    )
    .on(
        "SearchIssues",
        search(
            vec![
                list_issue(2, "ENG", "started"),
                list_issue(3, "ENG", "triage"),
            ],
            None,
        ),
    );
    let json = Cli::for_api(&api)
        .run(&[
            "issue",
            "query",
            "--all-teams",
            "--search",
            " oauth ",
            "--search-comments",
            "--json",
            "--include-archived",
            "--limit",
            "3",
        ])
        .success()
        .json();
    assert_eq!(identifiers(&json), ["ENG-1", "ENG-2", "ENG-3"]);
    let variables: Vec<Value> = api.requests().into_iter().map(|r| r.variables).collect();
    assert_eq!(
        variables,
        [
            json!({ "term": "oauth", "first": 3, "includeArchived": true, "includeComments": true }),
            json!({
                "term": "oauth", "first": 2, "includeArchived": true, "includeComments": true,
                "after": "cursor-1"
            }),
        ]
    );
}

#[test]
fn query_text_lists_issues() {
    let api = MockLinear::start();
    api.on(
        "GetIssuesForQuery",
        issues(vec![list_issue(4, "ENG", "started")], None),
    );
    Cli::for_api(&api)
        .run(&["issue", "query", "--all-teams", "--no-pager"])
        .success()
        .stdout_has("ENG-4")
        .stdout_has("Issue 4 title");
}

#[test]
fn mine_explicit_columns_keep_their_order_and_show_optional_columns() {
    let api = MockLinear::start();
    let mut row = list_issue(42, "DEMO", "unstarted");
    row["url"] = json!("https://linear.app/workspace-one/issue/DEMO-42");
    api.on("GetIssuesForState", issues(vec![row.clone()], None))
        .on("GetIssuesForState", issues(vec![row], None));
    let cli = Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "DEMO")
        .env_remove("NO_COLOR")
        .file(
            "cwd/.linear.toml",
            r#"[templates]
format_issue_id = '{{ hyperlink(issue.url, "界", issue.workspace_url_key ~ "/" ~ issue.identifier) }}'
"#,
        );
    let args = [
        "issue",
        "mine",
        "--columns",
        "title,cycle,assignee,id,team",
        "--no-pager",
    ];
    let run = cli.run(&args);
    run.success();
    let mut lines = run.stdout.lines();
    assert_eq!(
        lines.next(),
        Some("TITLE           CYC  A  ID                     TEAM")
    );
    assert_eq!(
        lines.next(),
        Some("Issue 42 title  -    -  workspace-one/DEMO-42  DEMO")
    );
    assert_eq!(lines.next(), None);

    let terminal = cli.run_tty(&args, &[]);
    terminal
        .success()
        .stdout_has("Issue 42 title  -    -  界  DEMO");
    assert!(
        terminal.terminal.contains(
            "\x1b]8;;https://linear.app/workspace-one/issue/DEMO-42\x1b\\界\x1b]8;;\x1b\\  DEMO"
        ),
        "{terminal:?}"
    );
    assert_eq!(api.operations(), ["GetIssuesForState", "GetIssuesForState"]);
}

#[test]
fn narrow_issue_table_clips_long_ids_before_their_lifecycle_marker() {
    use unicode_width::UnicodeWidthStr;

    const URL: &str = "https://linear.app/workspace-one/issue/DEMO-42";
    const WIDTH: u16 = 32;
    let api = MockLinear::start();
    let mut archived = list_issue(42, "DEMO", "unstarted");
    archived["url"] = json!(URL);
    archived["archivedAt"] = json!("2026-11-09T20:50:36.574Z");
    archived["state"]["name"] = json!("Todo");
    let mut live = list_issue(7, "DEMO", "unstarted");
    live["url"] = json!("https://linear.app/workspace-one/issue/DEMO-7");
    live["state"]["name"] = json!("Todo");
    let response = issues(vec![archived, live], None);
    for _ in 0..5 {
        api.on("GetIssuesForQuery", response.clone());
    }
    let cli = Cli::for_api(&api).env_remove("NO_COLOR").file(
        "cwd/.linear.toml",
        r#"[templates]
format_issue_id = '''{% if issue.identifier == "DEMO-42" %}{{ hyperlink(issue.url, "Workspace one demonstration issue display", "workspace-one/reference/DEMO-42 full fallback") }}{% else %}{{ hyperlink(issue.url, issue.identifier) }}{% endif %}'''
"#,
    );
    let args = [
        "issue",
        "query",
        "--all-teams",
        "--include-archived",
        "--columns",
        "id,title",
        "--no-pager",
    ];
    let assert_fits = |output: &str| {
        assert_eq!(output.lines().count(), 3, "{output}");
        assert!(
            output
                .lines()
                .all(|line| line.width() <= usize::from(WIDTH)),
            "{output}"
        );
        assert!(output.contains("DEMO-7"), "{output}");
    };
    let styled = cli.run_tty_with_width(&args, &[], Some(WIDTH));
    assert_fits(&styled.success().stdout);
    let archived = styled
        .terminal
        .lines()
        .find(|line| line.contains("[archived]"))
        .expect("archived row");
    let (label, after) = archived
        .strip_prefix(&format!("\x1b]8;;{URL}\x1b\\"))
        .and_then(|row| row.split_once("\x1b]8;;\x1b\\"))
        .expect("archived row starts with a complete hyperlink");
    assert!(
        label.starts_with("Work") && !label.contains('…'),
        "{archived:?}"
    );
    assert!(after.starts_with("… [archived]  "), "{archived:?}");

    let cli = cli.env("NO_COLOR", "1");
    let plain = cli.run_tty_with_width(&args, &[], Some(WIDTH));
    assert_fits(&plain.success().stdout);
    let fallback = plain
        .stdout
        .lines()
        .find(|line| line.contains("[archived]"))
        .expect("archived row");
    assert!(
        fallback.starts_with("works") && fallback.contains("… [archived]  "),
        "{fallback}"
    );
    assert!(!plain.terminal.contains('\x1b'), "{plain:?}");

    let squeezed = cli.run_tty_with_width(&args, &[], Some(22));
    let squeezed = &squeezed.success().stdout;
    assert!(
        squeezed.lines().all(|line| line.width() <= 22),
        "{squeezed}"
    );
    assert!(squeezed.contains("… [archived]  "), "{squeezed}");

    cli.run(&args)
        .success()
        .stdout_has("workspace-one/reference/DEMO-42 full fallback [archived]  ");

    let default_columns = cli.env_remove("NO_COLOR").run_tty_with_width(
        &[
            "issue",
            "query",
            "--all-teams",
            "--include-archived",
            "--no-pager",
        ],
        &[],
        Some(34),
    );
    let output = &default_columns.success().stdout;
    assert!(output.lines().all(|line| line.width() <= 34), "{output}");
    assert!(output.contains("… [archived]  "), "{output}");
    assert!(
        default_columns
            .terminal
            .contains(&format!("\x1b]8;;{URL}\x1b\\W")),
        "{default_columns:?}"
    );
}

#[test]
fn query_format_includes_the_id_template_and_preserves_configured_tabs() {
    let api = MockLinear::start();
    let mut first = list_issue(42, "DEMO", "started");
    first["url"] = json!("https://linear.app/workspace-one/issue/DEMO-42");
    first["title"] = json!("One\tfake column\nfake row\u{1b}[2J");
    let mut second = list_issue(42, "DEMO", "started");
    second["url"] = json!("https://linear.app/workspace-two/issue/DEMO-42");
    api.on("GetIssuesForQuery", issues(vec![first, second], None));
    let run = Cli::for_api(&api)
        .file(
            "cwd/.linear.toml",
            concat!(
                "[templates]\n",
                "format_issue_id = '{{ issue.workspace_url_key }}/{{ issue.identifier }}'\n",
                "[template-aliases]\n",
                "fields = '{% macro title(issue) %}{{ issue.title }}{% endmacro %}'\n",
                "pick = '{% import \"fields\" as fields %}{% include \"format_issue_id\" %}\t{{ issue.priorityLabel }}\t{{ fields.title(issue) }}'\n",
            ),
        )
        .run(&["issue", "query", "--all-teams", "--format", "pick"]);
    run.success();
    assert_eq!(
        run.stdout,
        "workspace-one/DEMO-42\tHigh\tOne fake column fake row�[2J\n\
         workspace-two/DEMO-42\tHigh\tIssue 42 title\n"
    );
    assert_eq!(api.operations(), ["GetIssuesForQuery"]);
}

#[test]
fn query_search_format_keeps_search_metadata() {
    let api = MockLinear::start();
    let mut row = list_issue(42, "DEMO", "started");
    row["url"] = json!("");
    row["metadata"] = json!({ "source": "title" });
    api.on(
        "SearchIssues",
        json!({ "searchIssues": {
        "nodes": [row],
        "pageInfo": { "hasNextPage": false, "endCursor": null }
    } }),
    );
    let run = Cli::for_api(&api)
        .file(
            "cwd/.linear.toml",
            r#"[template-aliases]
match = '{% include "format_issue_id" %} {{ issue.metadata.source }} {{ issue.workspace_url_key or "unavailable" }}'
"#,
        )
        .run(&["issue", "query", "--all-teams", "--search", "widget", "--format", "match"]);
    run.success();
    assert_eq!(run.stdout, "DEMO-42 title unavailable\n");
    assert_eq!(api.operations(), ["SearchIssues"]);
}

#[test]
fn mine_format_pages_only_on_a_terminal_when_enabled() {
    let api = MockLinear::start();
    let rows: Vec<_> = (1..=60)
        .map(|number| list_issue(number, "DEMO", "unstarted"))
        .collect();
    for _ in 0..3 {
        api.on("GetIssuesForState", issues(rows.clone(), None));
    }
    api.on("GetIssuesForState", issues(vec![], None));
    let cli = Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "DEMO")
        .file(
            "cwd/.linear.toml",
            "[template-aliases]\nids = '{{ issue.identifier }}'\n",
        )
        .stub_bin("sh", "exec /bin/sh \"$@\"")
        .stub_bin("test-pager", "cat");
    let pager = cli.path("bin/test-pager").display().to_string();
    let cli = cli.env("PAGER", &pager);
    let args = ["issue", "mine", "--format", "ids", "--limit", "all"];
    let piped = cli.run(&args);
    assert_eq!(piped.success().stdout.lines().count(), 60);
    assert!(cli.calls("test-pager").is_empty());

    cli.run_tty(&args, &[]).success().stdout_has("DEMO-60");
    assert_eq!(cli.calls("test-pager").len(), 1);

    let mut unpaged = args.to_vec();
    unpaged.push("--no-pager");
    cli.run_tty(&unpaged, &[]).success().stdout_has("DEMO-60");
    assert_eq!(cli.calls("test-pager").len(), 1);

    let empty = cli.run(&args);
    assert!(empty.success().stdout.is_empty(), "{empty}");
}

#[test]
fn mine_and_query_validate_output_options_before_any_requests() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api).env("LINEAR_TEAM_ID", "DEMO");
    for command in ["mine", "query"] {
        cli.run(&["issue", command, "--columns", "title,id,title"])
            .usage_error()
            .stderr_has("--columns lists title more than once");
        cli.run(&["issue", command, "--columns", "title,unknown"])
            .usage_error()
            .stderr_has("unknown");
        for name in ["unconfigured", "format_issue_id", "builtin/format_issue_id"] {
            cli.run(&["issue", command, "--format", name])
                .usage_error()
                .stderr_has(name);
        }
        for flags in [
            ["--format", "pick", "--json"],
            ["--columns", "id", "--json"],
        ] {
            cli.run(&["issue", command, flags[0], flags[1], flags[2]])
                .usage_error();
        }
        cli.run(&["issue", command, "--format", "pick", "--columns", "id"])
            .usage_error();
    }
    for open in ["--web", "--app"] {
        cli.run(&["issue", "mine", "--format", "pick", open])
            .usage_error();
        cli.run(&["issue", "mine", "--columns", "id", open])
            .usage_error();
    }
    assert!(api.requests().is_empty());
}

#[test]
fn query_unknown_state_fails() {
    let api = MockLinear::start();
    api.on(
        "GetWorkflowStatesInScope",
        json!({ "workflowStates": {
            "nodes": [{
                "id": "state-ready", "name": "Ready", "type": "unstarted",
                "position": 1, "team": { "key": "ENG" }
            }],
            "pageInfo": { "hasNextPage": false, "endCursor": null }
        } }),
    );
    Cli::for_api(&api)
        .run(&["issue", "query", "--all-teams", "--state", "Absent"])
        .not_found()
        .stderr_has("Absent");
    assert_eq!(
        api.variables("GetWorkflowStatesInScope"),
        json!({ "first": 100 })
    );
}

#[test]
fn query_validation_fails_before_any_request() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api);
    cli.run(&["issue", "query", "--json"])
        .failure()
        .stderr_has("--all-teams");
    let cli = cli.env("LINEAR_TEAM_ID", "ENG");
    cli.run(&["issue", "query", "--search-comments"])
        .usage_error()
        .stderr_has("--search");
    cli.run(&["issue", "query", "--search", "x", "--sort", "manual"])
        .usage_error()
        .stderr_has("--sort");
    let milestone = "f0000000-0000-4000-8000-000000000003";
    cli.run(&["issue", "query", "--search", "x", "--milestone", milestone])
        .usage_error()
        .stderr_has("--milestone");
    assert!(api.requests().is_empty());
}

#[test]
fn query_reports_api_errors() {
    let api = MockLinear::start();
    api.on_raw(
        "GetIssuesForQuery",
        500,
        r#"{"errors":[{"message":"boom"}]}"#,
    );
    Cli::for_api(&api)
        .run(&["issue", "query", "--all-teams", "--json"])
        .unavailable();
}

/// An issue whose description embeds an image served by `api`.
fn with_image(api: &MockLinear) -> (Value, String) {
    let url = format!("{}/img/diagram.png", api.base_url());
    let mut issue = issue(false);
    issue["description"] = json!(format!("See ![diagram]({url}) for details."));
    (details(issue), url)
}

/// Every file under `dir`, recursively.
fn files_under(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => panic!("read directory {}: {error}", dir.display()),
    };
    let mut files = Vec::new();
    for entry in entries {
        let path = entry
            .unwrap_or_else(|error| panic!("read directory entry in {}: {error}", dir.display()))
            .path();
        let is_dir = match path.metadata() {
            Ok(metadata) => metadata.is_dir(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => panic!("stat {}: {error}", path.display()),
        };
        if is_dir {
            files.extend(files_under(&path));
        } else {
            files.push(path);
        }
    }
    files
}

#[test]
fn view_downloads_images_into_the_private_user_cache() {
    let api = MockLinear::start();
    let (reply, url) = with_image(&api);
    for _ in 0..2 {
        api.on("GetIssueDetails", reply.clone()).on_http(
            "GET",
            "/img/diagram.png",
            200,
            b"PNGDATA",
        );
    }
    let cli = Cli::for_api(&api).env("TMPDIR", "/nonexistent");
    let run = cli.run(&["issue", "view", "ENG-1", "--no-comments", "--no-pager"]);
    run.success();
    let cache = cli.path("home/.cache/linear-cli/images");
    assert_eq!(files_under(&cache).len(), 1);
    // XDG_CACHE_HOME takes precedence over ~/.cache.
    let xdg = cli.path("xdg-cache");
    let cli = cli.env("XDG_CACHE_HOME", &xdg.display().to_string());
    let run = cli.run(&["issue", "view", "ENG-1", "--no-comments", "--no-pager"]);
    run.success();
    let cache = xdg.join("linear-cli/images");
    let files = files_under(&cache);
    assert_eq!(files.len(), 1, "{files:?}");
    assert_eq!(std::fs::read(&files[0]).expect("cached image"), b"PNGDATA");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = |path: &std::path::Path| {
            std::fs::metadata(path).expect("stat").permissions().mode() & 0o777
        };
        assert_eq!(mode(&cache), 0o700);
        assert_eq!(mode(&files[0]), 0o600);
    }
    assert!(
        run.stdout.contains(&files[0].display().to_string()),
        "{run}"
    );
    assert!(!run.stdout.contains(&url), "{run}");
    let download = api
        .requests()
        .into_iter()
        .find(|r| r.method == "GET")
        .expect("image request");
    assert_eq!(download.header("authorization"), None);
}

#[test]
fn view_no_download_keeps_remote_image_urls() {
    let api = MockLinear::start();
    let (reply, url) = with_image(&api);
    api.on("GetIssueDetails", reply);
    let cli = Cli::for_api(&api);
    let run = cli.run(&[
        "issue",
        "view",
        "ENG-1",
        "--no-comments",
        "--no-download",
        "--no-pager",
    ]);
    run.success().stdout_has(&url);
    assert!(files_under(&cli.path("home/.cache")).is_empty());
    assert_eq!(api.operations(), ["GetIssueDetails"]);
}

#[test]
fn view_leaves_images_on_other_hosts_alone() {
    let api = MockLinear::start();
    let mut issue = issue(false);
    issue["description"] = json!("![tracker](https://tracker.example/pixel.png)");
    api.on("GetIssueDetails", details(issue));
    let cli = Cli::for_api(&api);
    cli.run(&["issue", "view", "ENG-1", "--no-comments", "--no-pager"])
        .success()
        .stdout_has("![tracker](https://tracker.example/pixel.png)");
    assert!(files_under(&cli.path("home/.cache")).is_empty());
    assert_eq!(api.operations(), ["GetIssueDetails"]);
}

#[test]
fn view_reports_failed_image_downloads_and_keeps_the_url() {
    let api = MockLinear::start();
    let (reply, url) = with_image(&api);
    api.on("GetIssueDetails", reply)
        .on_http("GET", "/img/diagram.png", 404, b"missing");
    Cli::for_api(&api)
        .run(&["issue", "view", "ENG-1", "--no-comments", "--no-pager"])
        .success()
        .stdout_has(&url)
        .stderr_has(&url);
}

#[test]
fn view_without_a_cache_directory_fails_only_when_there_is_something_to_download() {
    let api = MockLinear::start();
    let (reply, _) = with_image(&api);
    api.on("GetIssueDetails", reply);
    Cli::for_api(&api)
        .env_remove("HOME")
        .run(&["issue", "view", "ENG-1", "--no-comments", "--no-pager"])
        .failure()
        .stderr_has("Could not find a cache directory for downloads")
        .stderr_has("--no-download");
    let api = MockLinear::start();
    api.on("GetIssueDetails", details(issue(false)));
    Cli::for_api(&api)
        .env_remove("HOME")
        .run(&["issue", "view", "ENG-1", "--no-comments", "--no-pager"])
        .success();
}

#[test]
fn view_of_a_missing_issue_is_not_found() {
    let api = MockLinear::start();
    api.on("GetIssueDetailsWithComments", json!({ "issue": null }));
    Cli::for_api(&api)
        .run(&["issue", "view", "ENG-404", "--json"])
        .not_found()
        .stderr_has("Issue not found: ENG-404");
}

#[test]
fn query_limit_must_be_a_whole_number() {
    for limit in ["-1", "1.5"] {
        Cli::new()
            .run(&["issue", "query", "--all-teams", "--limit", limit])
            .usage_error();
    }
}

#[test]
fn query_orders_types_but_preserves_same_type_server_order_across_teams() {
    let api = MockLinear::start();
    let issue_at = |number, team, kind, position: i32| {
        let mut row = list_issue(number, team, kind);
        row["state"]["position"] = json!(position);
        row
    };
    api.on(
        "GetIssuesForQuery",
        issues(
            vec![
                issue_at(1, "ENG", "unstarted", 1),
                issue_at(2, "ENG", "unstarted", 9),
                issue_at(3, "OPS", "unstarted", 5),
                issue_at(4, "ENG", "zulu", 9),
                issue_at(5, "OPS", "Écart", 1),
                issue_at(6, "ENG", "echo", 1),
                issue_at(7, "ENG", "echo", 9),
                issue_at(8, "ENG", "duplicate", 1),
                issue_at(9, "OPS", "triage", 1),
                issue_at(10, "ENG", "started", 0),
            ],
            None,
        ),
    );
    let rows = Cli::for_api(&api)
        .run(&["issue", "query", "--all-teams", "--json"])
        .success()
        .json_nodes();
    let order: Vec<_> = rows
        .iter()
        .map(|row| row["identifier"].as_str().expect("identifier"))
        .collect();
    assert_eq!(
        order,
        [
            "OPS-9", "ENG-10", "ENG-1", "ENG-2", "OPS-3", "ENG-8", "OPS-5", "ENG-6", "ENG-7",
            "ENG-4"
        ]
    );
}

#[test]
fn mine_orders_by_state_type_then_position_within_one_team() {
    let api = MockLinear::start();
    let issue_at = |number: u32, state_type: &str, position: f64| {
        let mut issue = list_issue(number, "ENG", state_type);
        issue["state"]["position"] = json!(position);
        issue
    };
    api.on(
        "GetIssuesForState",
        issues(
            vec![
                issue_at(1, "unstarted", 1.0),
                issue_at(2, "unstarted", 9.0),
                issue_at(3, "unstarted", 9.0),
                issue_at(4, "started", 0.0),
            ],
            None,
        ),
    );
    let run = Cli::for_api(&api).env("LINEAR_TEAM_ID", "ENG").run(&[
        "issue",
        "mine",
        "--all-states",
        "--no-pager",
    ]);
    let stdout = &run.success().stdout;
    let position = |id: &str| {
        stdout
            .find(id)
            .unwrap_or_else(|| panic!("{id} in {stdout}"))
    };
    let order = ["ENG-4", "ENG-2", "ENG-3", "ENG-1"].map(position);
    assert!(order.is_sorted(), "{stdout}");
}

#[test]
fn query_over_several_teams_matches_any_of_them() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved("team-eng", "ENG", "Engineering"))
        .on("ResolveTeam", resolved("team-ops", "OPS", "Operations"))
        .on("GetIssuesForQuery", issues(vec![], None));
    Cli::for_api(&api)
        .run(&["issue", "query", "--team", "eng", "--team", "ops", "--json"])
        .success();
    assert_eq!(
        api.variables("GetIssuesForQuery")["filter"],
        json!({ "team": { "or": [
            { "key": { "eq": "ENG" } },
            { "key": { "eq": "OPS" } }
        ] } })
    );
}

#[test]
fn view_fetches_every_page_of_long_collections() {
    let mut first = issue(true);
    first["comments"]["pageInfo"] = json!({ "hasNextPage": true, "endCursor": "comment-cursor" });
    first["labels"]["pageInfo"] = json!({ "hasNextPage": true, "endCursor": "label-cursor" });
    let api = MockLinear::start();
    api.on("GetIssueDetailsWithComments", details(first))
        .on(
            "GetIssueLabelsPage",
            json!({ "issue": { "labels": {
                "nodes": [{ "id": "label-ui", "name": "UI", "color": "#00ff00" }],
                "pageInfo": { "hasNextPage": false, "endCursor": null }
            } } }),
        )
        .on(
            "GetIssueCommentsPage",
            json!({ "issue": { "comments": {
                "nodes": [comment("late", "Late comment", None, None)],
                "pageInfo": { "hasNextPage": false, "endCursor": null }
            } } }),
        );
    let json = Cli::for_api(&api)
        .run(&["issue", "view", "ENG-1", "--json"])
        .success()
        .json();
    let ids = |key: &str| {
        nodes(&json[key])
            .iter()
            .map(|node| node["id"].as_str().expect("id").to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(ids("labels"), ["label-bug", "label-ui"]);
    assert_eq!(ids("comments"), ["root", "reply", "resolved", "late"]);
    assert_eq!(
        api.variables("GetIssueLabelsPage"),
        json!({ "id": "ENG-1", "first": 100, "after": "label-cursor" })
    );
    assert_eq!(
        api.variables("GetIssueCommentsPage"),
        json!({ "id": "ENG-1", "first": 100, "after": "comment-cursor" })
    );
}

#[test]
fn cache_file_reads_report_errors_instead_of_returning_no_files() {
    let cli = Cli::new().file("ordinary-file", "x");
    let failure = std::panic::catch_unwind(|| files_under(&cli.path("ordinary-file")))
        .expect_err("not a directory must not become an empty file list");
    let message = failure.downcast_ref::<String>().expect("panic message");
    assert!(message.contains("ordinary-file"), "{message}");
}

#[test]
fn cache_file_reads_report_symlink_metadata_errors() {
    let cli = Cli::new();
    std::fs::create_dir(cli.path("directory")).expect("cache directory");
    std::os::unix::fs::symlink("loop", cli.path("directory/loop")).expect("looping symlink");
    let failure = std::panic::catch_unwind(|| files_under(&cli.path("directory")))
        .expect_err("a symlink loop must not be listed as a file");
    let message = failure.downcast_ref::<String>().expect("panic message");
    assert!(message.contains("directory/loop"), "{message}");
}

#[test]
fn cache_file_reads_preserve_missing_paths_and_follow_directory_symlinks() {
    let cli = Cli::new().file("directory/real/a", "a");
    assert!(files_under(&cli.path("missing")).is_empty());
    std::os::unix::fs::symlink("real", cli.path("directory/link")).expect("directory symlink");
    std::os::unix::fs::symlink("missing", cli.path("directory/broken")).expect("dangling symlink");
    let mut files = files_under(&cli.path("directory"));
    files.sort();
    assert_eq!(
        files,
        [
            cli.path("directory/broken"),
            cli.path("directory/link/a"),
            cli.path("directory/real/a")
        ]
    );
}

#[test]
fn missing_state_hints_quote_names_in_single_and_multiple_team_scopes() {
    for command in ["mine", "query"] {
        let api = MockLinear::start();
        if command == "mine" {
            api.on("ResolveTeam", resolved("team-eng", "ENG", "Engineering"));
        }
        api.on("GetWorkflowStatesInScope", json!({ "workflowStates": {
            "nodes": [
                { "id": "say", "name": "Say \"hi\"", "type": "started", "position": 1, "team": { "key": if command == "mine" { "ENG" } else { "OPS" } } },
                { "id": "bell", "name": "Bell\u{7}", "type": "unstarted", "position": 2, "team": { "key": "ENG" } },
            ],
            "pageInfo": { "hasNextPage": false, "endCursor": null }
        } }));
        let scope = if command == "mine" {
            vec!["--team", "ENG"]
        } else {
            vec!["--all-teams"]
        };
        let mut argv = vec!["issue", command, "--state", "Absent"];
        argv.extend(scope);
        let run = Cli::for_api(&api).run(&argv);
        if command == "mine" {
            run.not_found()
                .stderr_has("Workflow state not found: 'Absent' in team ENG");
        }
        let expected = if command == "mine" {
            r#"Valid states: "Bell\u0007" (unstarted), "Say \"hi\"" (started)."#
        } else {
            r#"Valid states: "Bell\u0007" (unstarted, ENG), "Say \"hi\"" (started, OPS)."#
        };
        run.not_found().stderr_has(expected);
        assert!(!run.stderr.contains('\u{7}'));
        assert!(
            api.operations()
                .iter()
                .all(|op| op == "ResolveTeam" || op == "GetWorkflowStatesInScope")
        );
    }
}

#[test]
fn title_preserves_exact_remote_text_when_piped() {
    for title in [
        "Plain café",
        "ok café\u{1b}]0;owned\u{7}\u{1b}[2J\u{9b}31mred\u{8}\u{7f}\r\n\tend",
    ] {
        let api = MockLinear::start();
        let mut details = issue(false);
        details["team"]["key"] = json!("ENG");
        details["title"] = json!(title);
        api.on("GetIssueDetails", json!({"issue": details}));
        let run = Cli::for_api(&api).run(&["issue", "title", "ENG-1"]);
        run.success();
        assert_eq!(run.stdout, format!("{title}\n"));
        assert_eq!(api.operations(), ["GetIssueDetails"]);
        assert_eq!(api.variables("GetIssueDetails"), json!({"id": "ENG-1"}));
    }
}

#[test]
fn mine_prints_the_rows_itself_when_the_pager_fails() {
    let api = MockLinear::start();
    let rows = (1..=60)
        .map(|n| list_issue(n, "ENG", "unstarted"))
        .collect();
    api.on("GetIssuesForState", issues(rows, None));
    // The pager runs through `sh`, which the sandbox PATH lacks.
    let cli = Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .stub_bin("sh", "exec /bin/sh \"$@\"")
        .stub_bin("failing-pager", "cat > /dev/null; exit 1");
    let pager = cli.path("bin/failing-pager").display().to_string();
    let run = cli.env("PAGER", &pager).run_tty(&["issue", "mine"], &[]);
    run.success()
        .stdout_has("Warning: The pager exited with status 1, so the output is printed without it")
        .stdout_has("ENG-50");
}

#[test]
fn view_tty_ends_with_the_issue_url_after_the_comments() {
    for extra in [&[][..], &["--show-resolved-threads"], &["--no-comments"]] {
        let comments = !extra.contains(&"--no-comments");
        let api = MockLinear::start();
        let operation = if comments {
            "GetIssueDetailsWithComments"
        } else {
            "GetIssueDetails"
        };
        api.on(operation, details(issue(comments)));
        let args = [
            &["issue", "view", "ENG-1", "--no-download", "--no-pager"][..],
            extra,
        ]
        .concat();
        let run = Cli::for_api(&api).run_tty(&args, &[]);
        run.success().ends_with_footer(
            "View this issue on Linear: https://linear.app/acme/issue/ENG-1/fix-the-widget",
        );
        if comments {
            let footer = run.stdout.find("View this issue").expect("footer");
            let last_comment = run.stdout.find("Reply body").expect("reply");
            assert!(last_comment < footer, "{run}");
        }
    }
}

#[test]
fn view_piped_has_no_footer() {
    let api = MockLinear::start();
    api.on("GetIssueDetailsWithComments", details(issue(true)));
    let run = Cli::for_api(&api).run(&["issue", "view", "ENG-1", "--no-download"]);
    assert!(!run.success().stdout.contains("on Linear:"), "{run}");
}

#[test]
fn view_footer_is_paged_with_the_issue_and_survives_a_failing_pager() {
    let mut long = issue(true);
    long["description"] = json!(
        (1..=80)
            .map(|n| format!("line {n}"))
            .collect::<Vec<_>>()
            .join("\n\n")
    );
    let footer = "View this issue on Linear: https://linear.app/acme/issue/ENG-1/fix-the-widget";
    for (script, failing) in [
        ("cat > \"$PAGED\"", false),
        ("cat > /dev/null; exit 1", true),
    ] {
        let api = MockLinear::start();
        api.on("GetIssueDetailsWithComments", details(long.clone()));
        let cli = Cli::for_api(&api)
            .stub_bin("sh", "exec /bin/sh \"$@\"")
            .stub_bin("pager", script);
        let pager = cli.path("bin/pager").display().to_string();
        let paged = cli.path("paged.txt").display().to_string();
        let cli = cli.env("PAGER", &pager).env("PAGED", &paged);
        let run = cli.run_tty(&["issue", "view", "ENG-1", "--no-download"], &[]);
        if failing {
            run.success()
                .stdout_has("Warning: The pager exited with status 1")
                .ends_with_footer(footer);
        } else {
            run.success();
            let text = cli.read("paged.txt");
            assert!(
                text.trim_end().ends_with(&format!("\n\n{footer}")),
                "{text}"
            );
            assert_eq!(text.matches("on Linear:").count(), 1, "{text}");
        }
    }
}

/// A sandbox on a Ghostty terminal with color, viewing an issue whose
/// description shows `/img/diagram.png`, served as `image` when there is one.
fn on_ghostty(api: &MockLinear, image: Option<&[u8]>, lines: usize) -> Cli {
    let (mut reply, _) = with_image(api);
    let filler: Vec<String> = (1..=lines).map(|n| format!("Line {n}.")).collect();
    let description = reply["issue"]["description"]
        .as_str()
        .expect("description")
        .to_owned();
    reply["issue"]["description"] = json!(format!("{description}\n\n{}", filler.join("\n\n")));
    api.on("GetIssueDetails", reply);
    if let Some(image) = image {
        api.on_http("GET", "/img/diagram.png", 200, image);
    }
    Cli::for_api(api)
        .env_remove("NO_COLOR")
        .env("TERM_PROGRAM", "ghostty")
}

const VIEW_IMAGE: [&str; 5] = ["issue", "view", "ENG-1", "--no-comments", "--no-pager"];

#[test]
fn view_on_a_kitty_terminal_draws_downloaded_pngs_under_their_link() {
    let api = MockLinear::start();
    let run = on_ghostty(&api, Some(&png(64, 32)), 0).run_tty(&VIEW_IMAGE, &[]);
    run.success().stdout_has("Image: diagram");
    let terminal = &run.terminal;
    let sent = terminal
        .find("\x1b_Ga=t,f=100,t=d,")
        .expect("image transfer");
    let placed = terminal.find("\x1b_Ga=p,U=1,").expect("virtual placement");
    let cell = terminal.find('\u{10eeee}').expect("placeholder cells");
    assert!(sent < placed && placed < cell, "{run}");
    assert!(
        run.stdout.find("Image: diagram") < run.stdout.find('\u{10eeee}'),
        "{run}"
    );
}

#[test]
fn view_pages_only_placeholders_and_lets_less_print_them() {
    for (inherited, expected) in [(None, "10EEEE:p"), (Some("e000:w"), "e000:w,10EEEE:p")] {
        let api = MockLinear::start();
        let mut cli = on_ghostty(&api, Some(&png(64, 32)), 80)
            .stub_bin("sh", "exec /bin/sh \"$@\"")
            .stub_bin(
                "pager",
                "cat > \"$PAGED\"; printf %s \"$LESSUTFCHARDEF\" > \"$PAGED.env\"",
            );
        let pager = cli.path("bin/pager").display().to_string();
        let paged = cli.path("paged.txt").display().to_string();
        cli = cli.env("PAGER", &pager).env("PAGED", &paged);
        if let Some(value) = inherited {
            cli = cli.env("LESSUTFCHARDEF", value);
        }
        let run = cli.run_tty(&["issue", "view", "ENG-1", "--no-comments"], &[]);
        run.success();
        assert!(run.terminal.contains("\x1b_Ga=t,"), "{run}");
        let text = cli.read("paged.txt");
        assert!(text.contains('\u{10eeee}'), "{text}");
        assert!(!text.contains("\x1b_G"), "{text:?}");
        assert_eq!(cli.read("paged.txt.env"), expected);
    }
}

#[test]
fn view_without_a_kitty_terminal_or_with_images_off_shows_only_links() {
    type Case<'a> = (&'a str, &'a [(&'a str, &'a str)], &'a [&'a str]);
    let cases: [Case; 5] = [
        (
            "inline_images off",
            &[("LINEAR_INLINE_IMAGES", "false")],
            &[],
        ),
        ("NO_COLOR", &[("NO_COLOR", "1")], &[]),
        (
            "another terminal",
            &[("TERM_PROGRAM", "Apple_Terminal")],
            &[],
        ),
        ("inside tmux", &[("TMUX", "/tmp/tmux-1/default,1,0")], &[]),
        ("--no-download", &[], &["--no-download"]),
    ];
    for (name, env, flags) in cases {
        let api = MockLinear::start();
        let image = png(64, 32);
        let served = (!flags.contains(&"--no-download")).then_some(&image[..]);
        let mut cli = on_ghostty(&api, served, 0);
        for (key, value) in env {
            cli = cli.env(key, value);
        }
        let run = cli.run_tty(&[&VIEW_IMAGE[..], flags].concat(), &[]);
        run.success().stdout_has("diagram");
        assert!(!run.terminal.contains("\x1b_G"), "{name}: {run}");
        assert!(!run.terminal.contains('\u{10eeee}'), "{name}: {run}");
    }
}

#[test]
fn view_shows_other_formats_as_links_and_reports_broken_pngs() {
    let api = MockLinear::start();
    let run = on_ghostty(&api, Some(b"GIF89a not a png"), 0).run_tty(&VIEW_IMAGE, &[]);
    run.success().stdout_has("Image: diagram");
    assert!(!run.terminal.contains("\x1b_G"), "{run}");
    assert!(!run.stdout.contains("Warning"), "{run}");

    let api = MockLinear::start();
    let run = on_ghostty(&api, Some(b"\x89PNG\r\n\x1a\ntruncated"), 0).run_tty(&VIEW_IMAGE, &[]);
    run.success()
        .stdout_has("Image: diagram")
        .stdout_has("inline: malformed PNG");
    assert!(!run.terminal.contains("\x1b_G"), "{run}");
}

#[test]
fn view_piped_never_sends_images() {
    let api = MockLinear::start();
    let run = on_ghostty(&api, Some(&png(64, 32)), 0).run(&VIEW_IMAGE);
    run.success();
    assert!(!run.stdout.contains('\x1b'), "{run}");
}

#[test]
fn view_multiple_ends_each_issue_with_its_own_url() {
    let api = MockLinear::start();
    for number in [21, 22] {
        api.on_variables(
            "GetIssueDetails",
            json!({ "id": format!("ENG-{number}") }),
            details(numbered_issue(number, false)),
        );
    }
    let run = Cli::for_api(&api).run_tty(
        &[
            "issue",
            "view",
            "ENG-21",
            "ENG-22",
            "--no-comments",
            "--no-download",
            "--no-pager",
        ],
        &[],
    );
    run.success();
    let footers: Vec<&str> = run
        .stdout
        .lines()
        .filter(|line| line.contains("on Linear:"))
        .collect();
    assert_eq!(footers.len(), 2, "{run}");
    assert!(
        footers[0].contains("ENG-21") && footers[1].contains("ENG-22"),
        "{run}"
    );
    let second = run
        .stdout
        .find("ENG-22: Widget task 22")
        .expect("second issue");
    assert!(
        run.stdout
            .find("ENG-21")
            .is_some_and(|first| first < second),
        "{run}"
    );
    assert!(
        run.stdout
            .find(footers[0])
            .is_some_and(|footer| footer < second),
        "{run}"
    );
}

#[test]
fn view_multiple_sends_every_issues_images_before_the_text() {
    let api = MockLinear::start();
    for number in [21, 22] {
        let mut value = numbered_issue(number, false);
        value["description"] = json!(format!(
            "![shot {number}]({}/img/{number}.png)",
            api.base_url()
        ));
        api.on_variables(
            "GetIssueDetails",
            json!({ "id": format!("ENG-{number}") }),
            details(value),
        )
        .on_http(
            "GET",
            &format!("/img/{number}.png"),
            200,
            &png(10 * number, 20),
        );
    }
    let run = Cli::for_api(&api)
        .env_remove("NO_COLOR")
        .env("TERM_PROGRAM", "kitty")
        .run_tty(
            &[
                "issue",
                "view",
                "ENG-21",
                "ENG-22",
                "--no-comments",
                "--no-pager",
            ],
            &[],
        );
    run.success();
    assert_eq!(run.terminal.matches("\x1b_Ga=t,").count(), 2, "{run}");
    let last_transfer = run.terminal.rfind("\x1b_Ga=p,").expect("placements");
    let first_text = run.terminal.find("ENG-21").expect("first issue");
    assert!(last_transfer < first_text, "{run}");
}

#[test]
fn view_never_draws_a_local_file_the_markdown_names_itself() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api)
        .env_remove("NO_COLOR")
        .env("TERM_PROGRAM", "ghostty");
    std::fs::write(cli.path("secret.png"), png(8, 8)).expect("write png");
    let mut value = issue(false);
    value["description"] = json!(format!("![x]({})", cli.path("secret.png").display()));
    api.on("GetIssueDetails", details(value));
    let run = cli.run_tty(&VIEW_IMAGE, &[]);
    run.success().stdout_has("Image: x");
    assert!(!run.terminal.contains("\x1b_G"), "{run}");
}

#[test]
fn view_surfaces_lifecycle_without_replacing_workflow_state() {
    let at = json!("2026-11-09T20:50:36.574Z");
    for comments in [false, true] {
        for (trashed, archived, banner, state) in [
            (
                json!(true),
                at.clone(),
                "**Deleted:** 2026-11-09 (in trash; restore it in Linear to edit or comment)",
                "Started (deleted)",
            ),
            (
                json!(true),
                Value::Null,
                "**Deleted:** In trash; restore it in Linear to edit or comment",
                "Started (deleted)",
            ),
            (
                json!(false),
                at.clone(),
                "**Archived:** 2026-11-09",
                "Started (archived)",
            ),
            (
                Value::Null,
                at.clone(),
                "**Archived:** 2026-11-09",
                "Started (archived)",
            ),
            (Value::Null, Value::Null, "", "Started"),
            (json!(false), Value::Null, "", "Started"),
        ] {
            let api = MockLinear::start();
            let mut value = issue(comments);
            value["trashed"] = trashed.clone();
            value["archivedAt"] = archived.clone();
            let operation = if comments {
                "GetIssueDetailsWithComments"
            } else {
                "GetIssueDetails"
            };
            api.on(operation, details(value.clone()))
                .on(operation, details(value));
            let cli = Cli::for_api(&api);
            let mut args = vec!["issue", "view", "ENG-1", "--no-download"];
            if !comments {
                args.push("--no-comments");
            }
            let text = cli.run(&args);
            text.success().stdout_has(&format!("**State:** {state}\n"));
            if banner.is_empty() {
                assert!(!text.stdout.contains("**Deleted:**"), "{text}");
                assert!(!text.stdout.contains("**Archived:**"), "{text}");
            } else {
                text.stdout_has(&format!("# ENG-1: Fix the widget\n\n{banner}\n"));
            }
            args.push("--json");
            let output = cli.run(&args).success().json();
            assert_eq!(output.get("trashed"), Some(&trashed));
            assert_eq!(output.get("archivedAt"), Some(&archived));
            assert_eq!(output["state"]["name"], "Started");
            for request in api.requests() {
                assert!(request.query.contains("trashed"));
                assert!(request.query.contains("archivedAt"));
            }
        }
    }
}

#[test]
fn query_and_search_preserve_lifecycle_in_json_and_tables() {
    for search in [false, true] {
        let operation = if search {
            "SearchIssues"
        } else {
            "GetIssuesForQuery"
        };
        let api = MockLinear::start();
        let mut deleted = list_issue(1, "ENG", "unstarted");
        deleted["trashed"] = json!(true);
        deleted["archivedAt"] = json!("2026-11-09T20:50:36.574Z");
        let mut rows = vec![deleted, list_issue(2, "ENG", "started")];
        let mut response = issues(rows.clone(), None);
        if search {
            for row in &mut rows {
                row["metadata"] = json!({});
            }
            response = json!({ "searchIssues": { "nodes": rows, "pageInfo": { "hasNextPage": false, "endCursor": null } } });
        }
        api.on(operation, response.clone()).on(operation, response);
        let cli = Cli::for_api(&api);
        let mut args = vec!["issue", "query", "--all-teams", "--include-archived"];
        if search {
            args.extend(["--search", "widget"]);
        }
        let text = cli.run(&args);
        text.success().stdout_has("[deleted]");
        args.push("--json");
        let output = cli.run(&args).success().json_nodes();
        let deleted = output
            .iter()
            .find(|row| row["identifier"] == "ENG-1")
            .expect("deleted row");
        assert_eq!(deleted.get("trashed"), Some(&json!(true)));
        assert_eq!(deleted["archivedAt"], "2026-11-09T20:50:36.574Z");
        let live = output
            .iter()
            .find(|row| row["identifier"] == "ENG-2")
            .expect("live row");
        assert_eq!(live.get("trashed"), Some(&Value::Null));
        assert_eq!(live.get("archivedAt"), Some(&Value::Null));
        for request in api.requests() {
            assert!(request.query.contains("trashed"));
            assert!(request.query.contains("archivedAt"));
            assert_eq!(request.variables["includeArchived"], true);
        }
    }
}

#[test]
fn view_marks_parent_and_paginated_child_lifecycle() {
    let api = MockLinear::start();
    let mut value = issue(false);
    value["parent"]["trashed"] = json!(true);
    let mut child = value["children"]["nodes"][0].clone();
    child["archivedAt"] = json!("2026-11-09T20:50:36.574Z");
    value["children"]["pageInfo"] = json!({ "hasNextPage": true, "endCursor": "children-next" });
    api.on("GetIssueDetails", details(value)).on(
        "GetIssueChildrenPage",
        json!({ "issue": { "children": {
        "nodes": [child], "pageInfo": { "hasNextPage": false, "endCursor": null }
    } } }),
    );
    Cli::for_api(&api)
        .run(&["issue", "view", "ENG-1", "--no-comments", "--no-download"])
        .success()
        .stdout_has("ENG-99 [deleted]")
        .stdout_has("ENG-2 [archived]");
}

#[test]
fn view_formats_heading_and_hierarchy_issue_ids_for_each_output() {
    let issue_url = "https://linear.app/workspace-one/issue/DEMO-42/example";
    let parent_url = "https://linear.app/workspace-one/issue/DEMO-41/parent";
    let child_url = "https://linear.app/workspace-one/issue/DEMO-43/child";
    let mut value = issue(false);
    value["identifier"] = json!("DEMO-42");
    value["url"] = json!(issue_url);
    value["parent"]["identifier"] = json!("DEMO-41");
    value["parent"]["url"] = json!(parent_url);
    value["parent"]["trashed"] = json!(true);
    value["children"]["nodes"][0]["identifier"] = json!("DEMO-43");
    value["children"]["nodes"][0]["url"] = json!(child_url);
    let api = MockLinear::start();
    for _ in 0..4 {
        api.on("GetIssueDetails", details(value.clone()));
    }
    let cli = Cli::for_api(&api).env_remove("NO_COLOR").file(
        "cwd/linear.toml",
        r#"[templates]
format_issue_id = '''{{ hyperlink(issue.url, "[<" ~ issue.workspace_url_key ~ ">](" ~ issue.identifier ~ ")", issue.workspace_url_key ~ "/" ~ issue.identifier) }}'''
"#,
    );
    let args = [
        "issue",
        "view",
        "DEMO-42",
        "--no-comments",
        "--no-download",
        "--no-pager",
    ];
    let tty = cli.run_tty(&args, &[]);
    tty.success()
        .stdout_has("[<workspace-one>](DEMO-42): Fix the widget")
        .stdout_has("[<workspace-one>](DEMO-41) [deleted]: Parent work")
        .stdout_has("[<workspace-one>](DEMO-43): Child work");
    for (url, identifier) in [
        (issue_url, "DEMO-42"),
        (parent_url, "DEMO-41"),
        (child_url, "DEMO-43"),
    ] {
        let (_, link) = tty
            .terminal
            .split_once(&format!("\x1b]8;;{url}\x1b\\"))
            .expect("the issue URL opens a link");
        let (label, _) = link
            .split_once("\x1b]8;;\x1b\\")
            .expect("the issue link ends");
        assert_eq!(
            plain_text(label.as_bytes()),
            format!("[<workspace-one>]({identifier})")
        );
    }
    let piped = cli.run(&args);
    piped
        .success()
        .stdout_has("# workspace-one/DEMO-42: Fix the widget")
        .stdout_has("- **workspace-one/DEMO-41 [deleted]**: Parent work")
        .stdout_has("- **workspace-one/DEMO-43**: Child work");
    assert!(!piped.stdout.contains("\x1b]8;;"), "{piped}");
    let args_json = [&args[..], &["--json"]].concat();
    let json = cli.run(&args_json).success().json();
    assert_json(&json, &view_json(value));
    let plain = cli.env("NO_COLOR", "1").run_tty(&args, &[]);
    plain
        .success()
        .stdout_has("# workspace-one/DEMO-42: Fix the widget")
        .stdout_has("workspace-one/DEMO-41 [deleted]: Parent work")
        .stdout_has("workspace-one/DEMO-43: Child work");
    assert!(!plain.terminal.contains("\x1b]8;;"), "{plain}");
    assert_eq!(api.operations(), ["GetIssueDetails"; 4]);
}

#[test]
fn terminal_view_shows_the_deleted_banner_and_state_suffix() {
    let api = MockLinear::start();
    let mut value = issue(false);
    value["trashed"] = json!(true);
    value["archivedAt"] = json!("2026-11-09T20:50:36.574Z");
    api.on("GetIssueDetails", details(value));
    Cli::for_api(&api)
        .run_tty(
            &[
                "issue",
                "view",
                "ENG-1",
                "--no-comments",
                "--no-download",
                "--no-pager",
            ],
            &[],
        )
        .success()
        .stdout_has("Deleted: 2026-11-09")
        .stdout_has("State: Started (deleted)");
}
