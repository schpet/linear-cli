//! Read-only `issue` commands: view, title, url, mine and query.
use serde_json::{Value, json};

use crate::support::{Cli, MockLinear, assert_json, nodes};
use crate::team::{resolve_vars, resolved};

fn issue(comments: bool) -> Value {
    let mut issue = json!({
        "identifier": "ENG-1", "title": "Fix the widget",
        "description": "## Body\n\nPlain **bold** text.",
        "url": "https://linear.app/acme/issue/ENG-1/fix-the-widget",
        "branchName": "eng-1-fix-the-widget",
        "state": { "name": "Started", "color": "#123456" },
        "assignee": { "name": "alice", "displayName": "Alice Example" },
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
            "identifier": "ENG-99", "title": "Parent work",
            "state": { "name": "Done", "color": "#123456" }
        },
        "children": { "nodes": [{
            "identifier": "ENG-2", "title": "Child work",
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
        }] }
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

#[test]
fn view_json_prints_the_issue_with_comments() {
    let api = MockLinear::start();
    api.on("GetIssueDetailsWithComments", details(issue(true)));
    let json = Cli::for_api(&api)
        .run(&["issue", "view", "eng-1", "--json"])
        .success()
        .json();
    assert_json(&json, &issue(true));
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
    assert_json(&json, &issue(false));
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
fn view_rejects_an_unparseable_id_before_any_request() {
    let api = MockLinear::start();
    Cli::for_api(&api)
        .run(&["issue", "view", "not an id!"])
        .failure()
        .stderr_has("issue ID");
    assert!(api.requests().is_empty());
}

#[test]
fn view_reports_api_errors() {
    let api = MockLinear::start();
    api.on_error("GetIssueDetailsWithComments", "Entity not found");
    Cli::for_api(&api)
        .run(&["issue", "view", "ENG-1", "--json"])
        .failure()
        .stderr_has("Entity not found");
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
fn list_lists_my_unstarted_issues_in_the_configured_team() {
    let api = MockLinear::start();
    api.on(
        "GetIssuesForState",
        issues(vec![list_issue(1, "ENG", "unstarted")], None),
    );
    Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .run(&["issue", "list"])
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
fn list_json_matches_query_for_the_canonical_command_and_aliases() {
    let mut row = list_issue(1, "ENG", "unstarted");
    row["labels"]["nodes"] = json!([{ "id": "label-1", "name": "Bug", "color": "#123456" }]);
    row["inverseRelations"]["nodes"] = json!([{
        "id": "relation-1", "type": "blocks",
        "issue": { "id": "issue-2", "identifier": "ENG-2", "state": { "type": "started" } }
    }]);
    for (command, flag) in [("list", "--json"), ("mine", "-j"), ("l", "--json")] {
        let api = MockLinear::start();
        api.on("GetIssuesForState", issues(vec![row.clone()], None))
            .on("GetIssuesForQuery", issues(vec![row.clone()], None));
        let cli = Cli::for_api(&api).env("LINEAR_TEAM_ID", "ENG");
        let list = cli.run(&["issue", command, flag]);
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
fn list_json_prints_an_empty_array() {
    let api = MockLinear::start();
    api.on("GetIssuesForState", issues(vec![], None));
    let run =
        Cli::for_api(&api)
            .env("LINEAR_TEAM_ID", "ENG")
            .run(&["issue", "list", "-j", "--no-pager"]);
    run.success();
    assert_eq!(run.stdout, "[]\n");
}

#[test]
fn list_json_collects_and_orders_all_pages() {
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
        "list",
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
fn list_json_stops_at_the_limit() {
    let api = MockLinear::start();
    let row = list_issue(1, "ENG", "unstarted");
    api.on("GetIssuesForState", issues(vec![row.clone()], Some("next")));
    let run = Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .run(&["issue", "list", "--json", "--limit", "1"]);
    run.success();
    assert_json(&run.json(), &json!([row]));
    assert_eq!(api.requests().len(), 1);
    assert_eq!(api.variables("GetIssuesForState")["first"], json!(1));
}

#[test]
fn list_json_reports_api_errors_without_partial_output() {
    let api = MockLinear::start();
    api.on(
        "GetIssuesForState",
        issues(vec![list_issue(1, "ENG", "unstarted")], Some("next")),
    )
    .on_error("GetIssuesForState", "Access denied");
    let run = Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .run(&["issue", "list", "--json", "--limit", "all"]);
    run.failure();
    run.stderr_has("Access denied");
    assert!(run.stdout.is_empty(), "{run}");
}

#[test]
fn list_json_conflicts_with_opening_the_list() {
    for flag in ["--web", "--app"] {
        let api = MockLinear::start();
        Cli::for_api(&api)
            .run(&["issue", "list", "--json", flag])
            .usage_error()
            .stderr_has("cannot be used with");
        assert!(api.requests().is_empty());
    }
}

#[test]
fn list_sends_filters() {
    let api = MockLinear::start();
    api.on("GetIssuesForState", issues(vec![], None));
    let project = "f0000000-0000-4000-8000-000000000002";
    let milestone = "f0000000-0000-4000-8000-000000000003";
    Cli::for_api(&api)
        .env("LINEAR_TEAM_ID", "ENG")
        .run(&[
            "issue",
            "list",
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
}

#[test]
fn list_and_query_resolve_sort_flags_over_configuration() {
    let manual = json!([
        { "workflowState": { "order": "Ascending" } },
        { "manual": { "nulls": "last", "order": "Ascending" } }
    ]);
    for (command, operation) in [
        ("list", "GetIssuesForState"),
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
            if command == "list" {
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
fn list_unlimited_follows_pages() {
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
            "list",
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
fn list_resolves_team_cycle_and_state_names() {
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
                    "team": { "key": "ENG" }
                }],
                "pageInfo": { "hasNextPage": false, "endCursor": null }
            } }),
        )
        .on("GetIssuesForState", issues(vec![], None));
    Cli::for_api(&api)
        .run(&[
            "issue", "list", "--team", "eng", "--cycle", "active", "--state", "started", "--state",
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
fn list_filters_by_another_assignee_or_none() {
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
        let mut argv = vec!["issue", "list"];
        argv.extend(args);
        cli.run(&argv).success();
        let requests = api.requests();
        let last = requests
            .iter()
            .rev()
            .find(|r| r.operation.as_deref() == Some("GetIssuesForState"))
            .expect("an issue list request");
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
        &["issue", "list", "--assignee", url][..],
        &["issue", "update", "ENG-1", "-a", url],
        &["project", "create", "-n", "P", "--lead", url],
        &["initiative", "list", "--owner", url],
    ] {
        cli.run(args).usage_error().stderr_has("Linear URL");
    }
    assert!(api.requests().is_empty());
}

#[test]
fn list_assignee_filters_conflict_with_each_other() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api).env("LINEAR_TEAM_ID", "ENG");
    for args in [
        &["--assignee", "ada", "--unassigned"][..],
        &["--assignee", "ada", "-A"],
        &["-A", "-U"],
    ] {
        let mut argv = vec!["issue", "list"];
        argv.extend(args);
        cli.run(&argv).usage_error();
    }
    assert!(api.requests().is_empty());
}

#[test]
fn list_rejects_a_url_that_is_not_a_cycle() {
    let api = MockLinear::start();
    api.on("ResolveTeam", resolved("team-eng", "ENG", "Engineering"));
    Cli::for_api(&api)
        .run(&[
            "issue",
            "list",
            "--team",
            "eng",
            "--cycle",
            "https://linear.app/acme/issue/ENG-1/wrong",
        ])
        .failure()
        .stderr_has("cycle URL, number, or name");
}

#[test]
fn list_validation_fails_before_any_request() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api);
    cli.run(&["issue", "list"])
        .failure()
        .stderr_has("No team given and no default team configured")
        .stderr_has("--team");
    let cli = cli.env("LINEAR_TEAM_ID", "ENG");
    cli.run(&["issue", "list", "--created-after", "nope"])
        .usage_error()
        .stderr_has("--created-after");
    cli.run(&["issue", "list", "--sort", "bogus"]).usage_error();
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
fn query_unknown_state_fails() {
    let api = MockLinear::start();
    api.on(
        "GetWorkflowStatesInScope",
        json!({ "workflowStates": {
            "nodes": [{
                "id": "state-ready", "name": "Ready", "type": "unstarted",
                "team": { "key": "ENG" }
            }],
            "pageInfo": { "hasNextPage": false, "endCursor": null }
        } }),
    );
    Cli::for_api(&api)
        .run(&["issue", "query", "--all-teams", "--state", "Absent"])
        .failure()
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
        .failure();
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
        .failure()
        .stderr_has("Issue not found: ENG-404");
}

#[test]
fn list_limit_must_be_a_whole_number() {
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
fn list_orders_by_state_type_then_position_within_one_team() {
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
        "list",
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
    for command in ["list", "query"] {
        let api = MockLinear::start();
        if command == "list" {
            api.on("ResolveTeam", resolved("team-eng", "ENG", "Engineering"));
        }
        api.on("GetWorkflowStatesInScope", json!({ "workflowStates": {
            "nodes": [
                { "id": "say", "name": "Say \"hi\"", "type": "started", "team": { "key": if command == "list" { "ENG" } else { "OPS" } } },
                { "id": "bell", "name": "Bell\u{7}", "type": "unstarted", "team": { "key": "ENG" } },
            ],
            "pageInfo": { "hasNextPage": false, "endCursor": null }
        } }));
        let scope = if command == "list" {
            vec!["--team", "ENG"]
        } else {
            vec!["--all-teams"]
        };
        let mut argv = vec!["issue", command, "--state", "Absent"];
        argv.extend(scope);
        let run = Cli::for_api(&api).run(&argv);
        let expected = if command == "list" {
            r#"Valid states: "Bell\u0007" (unstarted), "Say \"hi\"" (started)."#
        } else {
            r#"Valid states: "Bell\u0007" (unstarted, ENG), "Say \"hi\"" (started, OPS)."#
        };
        run.failure().stderr_has(expected);
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
