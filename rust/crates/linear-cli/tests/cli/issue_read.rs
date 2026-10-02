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
            "createdAt": "2024-01-01T00:00:00Z"
        }] },
        "documents": { "nodes": [{
            "id": "doc-1", "title": "Spec", "slugId": "spec-123",
            "url": "https://linear.app/acme/document/spec-123",
            "createdAt": "2024-01-01T00:00:00Z", "updatedAt": "2024-01-01T00:00:00Z"
        }] }
    });
    if comments {
        issue["comments"] = json!({ "nodes": [
            comment("root", "Root comment body", None, None),
            comment("reply", "Reply body", Some("root"), None),
            comment("resolved", "Resolved thread body", None, Some("2024-01-02T00:00:00Z")),
        ] });
    }
    issue
}

fn comment(id: &str, body: &str, parent: Option<&str>, resolved_at: Option<&str>) -> Value {
    json!({
        "id": id, "body": body, "quotedText": null, "createdAt": "2024-01-03T00:00:00Z",
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
    let api = MockLinear::start();
    api.on("GetIssueDetails", details(issue(false)))
        .on("GetIssueDetails", details(issue(false)));
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
        "createdAt": "2024-01-01T00:00:00Z", "updatedAt": "2024-01-01T00:00:00Z",
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
fn mine_lists_my_unstarted_issues_in_the_configured_team() {
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
fn mine_sends_filters() {
    let api = MockLinear::start();
    api.on("GetIssuesForState", issues(vec![], None));
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
            "2024-01-02T03:00:00Z",
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
            "0",
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
        "first": 50
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
                        "startsAt": "2024-01-01T00:00:00Z", "isNext": false, "isPrevious": false
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
fn mine_validation_fails_before_any_request() {
    let api = MockLinear::start();
    let cli = Cli::for_api(&api);
    cli.run(&["issue", "mine"]).failure().stderr_has("--team");
    let cli = cli.env("LINEAR_TEAM_ID", "ENG");
    cli.run(&["issue", "mine", "--created-after", "nope"])
        .failure()
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
            "0",
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
        json!({ "first": 250 })
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
        .failure()
        .stderr_has("--search");
    cli.run(&["issue", "query", "--search", "x", "--sort", "manual"])
        .failure()
        .stderr_has("--sort");
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
