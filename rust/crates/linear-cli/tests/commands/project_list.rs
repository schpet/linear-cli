use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::ready;
use std::rc::Rc;
use std::time::UNIX_EPOCH;

use cynic::QueryBuilder;
use linear_cli::commands::project::list::{
    Options, check_conflicting_flags, filter, opening, render_text, run_with,
};
use linear_cli::graphql::envelope::{GraphQlRequest, ResponseError, parse_response};
use linear_cli::graphql::operations::projects::{GetProjects, GetProjectsVariables};
use linear_cli::graphql::operations::viewer::GetViewer;
use serde_json::{Value, json};

fn project(id: &str, name: &str, sort_order: Value) -> Value {
    json!({
        "id": id,
        "name": name,
        "description": "Synthetic description",
        "slugId": "ALPHA",
        "icon": null,
        "color": "#cccccc",
        "sortOrder": sort_order,
        "status": {"id":"ps1", "name":"Planned", "color":"#ffcc00", "type":"planned"},
        "lead": {"name":"Ada Lovelace", "displayName":"Ada", "initials":"AL"},
        "priority": 2,
        "health": null,
        "startDate": "2026-01-01",
        "targetDate": null,
        "startedAt": null,
        "completedAt": null,
        "canceledAt": null,
        "createdAt": "2024-01-01T00:00:00.000Z",
        "updatedAt": "2024-01-02T00:00:00.000Z",
        "url": format!("https://linear.app/alpha/project/{id}"),
        "teams": {"nodes":[{"key":"ENG"}]}
    })
}

fn page(nodes: Vec<Value>, more: bool, cursor: Option<&str>) -> GetProjects {
    let body = json!({"data":{"projects":{
        "nodes": nodes,
        "pageInfo":{"hasNextPage":more,"endCursor":cursor}
    }}});
    parse_response(body.to_string().as_bytes()).expect("typed project page")
}

#[test]
fn selected_documents_and_nested_filter_are_public() {
    let request = GraphQlRequest::with_variables(GetProjects::build(GetProjectsVariables {
        filter: filter(Some("ENG"), Some("Planned")),
        first: Some(100),
        after: None,
    }));
    assert_eq!(request.operation_name.as_deref(), Some("GetProjects"));
    assert_eq!(
        serde_json::to_value(&request).unwrap()["variables"],
        json!({"filter":{"accessibleTeams":{"some":{"key":{"eq":"ENG"}}},"status":{"name":{"eq":"Planned"}}},"first":100})
    );
    assert_eq!(
        request.query.split_whitespace().collect::<String>(),
        "queryGetProjects($filter:ProjectFilter,$first:Int,$after:String){projects(filter:$filter,first:$first,after:$after){nodes{idnamedescriptionslugIdiconcolorsortOrderstatus{idnamecolortype}lead{namedisplayNameinitials}priorityhealthstartDatetargetDatestartedAtcompletedAtcanceledAtcreatedAtupdatedAturlteams{nodes{key}}}pageInfo{hasNextPageendCursor}}}"
    );
    let viewer = GraphQlRequest::without_variables(GetViewer::build(()));
    assert_eq!(viewer.operation_name.as_deref(), Some("GetViewer"));
    assert_eq!(
        viewer.query.split_whitespace().collect::<String>(),
        "queryGetViewer{viewer{organization{urlKey}}}"
    );
    assert!(
        serde_json::to_value(&viewer)
            .unwrap()
            .get("variables")
            .is_none()
    );
}

#[tokio::test]
async fn json_paginates_sorts_and_preserves_selected_connection_and_number_bytes() {
    let pages = Rc::new(RefCell::new(VecDeque::from([
        page(
            vec![
                project("z", "Zulu", json!(2)),
                project("b", "Same", json!(0.5)),
            ],
            true,
            Some("c1"),
        ),
        page(
            vec![
                project("a", "Same", json!(0.5)),
                project("neg", "Negative", json!(-2)),
            ],
            false,
            Some("c2"),
        ),
    ])));
    let requests = Rc::new(RefCell::new(Vec::new()));
    let output = run_with(
        {
            let pages = Rc::clone(&pages);
            let requests = Rc::clone(&requests);
            move |request| {
                requests
                    .borrow_mut()
                    .push(serde_json::to_value(&request).unwrap());
                ready(Ok(pages.borrow_mut().pop_front().expect("two pages")))
            }
        },
        None,
        None,
        true,
        || UNIX_EPOCH,
        120,
        false,
    )
    .await
    .expect("project JSON");
    let text = String::from_utf8(output).unwrap();
    let value: Value = serde_json::from_str(&text).unwrap();
    let ids = value["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| node["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(ids, ["neg", "a", "b", "z"]);
    assert_eq!(
        value["pageInfo"],
        json!({"hasNextPage":false,"endCursor":"c2"})
    );
    assert!(text.contains("\"sortOrder\": -2"));
    assert!(text.contains("\"sortOrder\": 0.5"));
    assert_eq!(value["nodes"][0]["status"]["type"], "planned");
    assert_eq!(value["nodes"][0]["teams"]["nodes"][0]["key"], "ENG");
    let requests = requests.borrow();
    assert_eq!(requests[0]["variables"], json!({"first":100}));
    assert_eq!(requests[1]["variables"], json!({"first":100,"after":"c1"}));
}

#[tokio::test]
async fn empty_cursor_is_sent_but_missing_and_repeated_cursors_stop() {
    let requests = Rc::new(RefCell::new(Vec::new()));
    let calls = Rc::new(RefCell::new(0));
    let output = run_with(
        {
            let requests = Rc::clone(&requests);
            let calls = Rc::clone(&calls);
            move |request| {
                requests
                    .borrow_mut()
                    .push(serde_json::to_value(&request).unwrap());
                let mut count = calls.borrow_mut();
                *count += 1;
                ready(Ok(page(vec![], *count == 1, Some(""))))
            }
        },
        None,
        None,
        true,
        || UNIX_EPOCH,
        120,
        false,
    )
    .await
    .expect("empty cursor is concrete");
    assert!(String::from_utf8(output).unwrap().contains("\"nodes\": []"));
    assert_eq!(
        requests.borrow()[1]["variables"],
        json!({"first":100,"after":""})
    );

    let error = run_with(
        |_| ready(Ok(page(vec![project("a", "Alpha", json!(1))], true, None))),
        None,
        None,
        true,
        || UNIX_EPOCH,
        120,
        false,
    )
    .await
    .expect_err("missing cursor");
    assert!(error.to_string().contains("no pagination cursor on page 1"));
    let calls = Rc::new(RefCell::new(0));
    let error = run_with(
        {
            let calls = Rc::clone(&calls);
            move |_| {
                *calls.borrow_mut() += 1;
                ready(Ok(page(vec![], true, Some("repeat"))))
            }
        },
        None,
        None,
        true,
        || UNIX_EPOCH,
        120,
        false,
    )
    .await
    .expect_err("repeated cursor");
    assert_eq!(*calls.borrow(), 2);
    assert!(
        error
            .to_string()
            .contains("repeated a project pagination cursor")
    );
}

#[test]
fn text_dates_unknown_status_and_browser_precedence_are_explicit() {
    let mut value = project("a", "A %s", json!(1));
    value["status"]["type"] = json!("started");
    value["startedAt"] = json!("9999-01-01T00:00:00.000Z");
    let parsed = page(vec![value.clone()], false, None);
    let text = render_text(&parsed.projects.nodes, UNIX_EPOCH, 120, false).unwrap();
    assert!(text.contains("A %s"));
    assert!(text.contains("Started just now"));
    assert!(text.starts_with("SLUG"));
    value["status"]["type"] = json!("futureStatus");
    let parsed = page(vec![value], false, None);
    let error = render_text(&parsed.projects.nodes, UNIX_EPOCH, 120, false)
        .expect_err("unknown text status");
    assert!(
        error
            .to_string()
            .contains("unknown project status type: futureStatus")
    );

    let (url, line) = opening("alpha", Some("ENG"), true);
    assert_eq!(url, "https://linear.app/alpha/team/ENG/projects/all");
    assert_eq!(
        line,
        b"Opening https://linear.app/alpha/team/ENG/projects/all in Linear.app\n"
    );
    let (url, _) = opening("alpha", None, false);
    assert_eq!(url, "https://linear.app/alpha/projects/all");
    assert!(
        check_conflicting_flags(&Options {
            team: Some("ENG".to_owned()),
            all_teams: true,
            ..Default::default()
        })
        .is_err()
    );
}

#[test]
fn text_uses_each_status_date_precedence_and_priority_health_fallbacks() {
    let cases = [
        (
            "started",
            Some(("startedAt", "2024-01-03T00:00:00.000Z")),
            "Started just now",
        ),
        (
            "completed",
            Some(("completedAt", "2024-01-03T00:00:00.000Z")),
            "Done just now",
        ),
        (
            "canceled",
            Some(("canceledAt", "2024-01-03T00:00:00.000Z")),
            "Canceled just now",
        ),
        (
            "planned",
            Some(("targetDate", "2027-08-09")),
            "Start: 2026-01-01",
        ),
        ("backlog", None, "Updated just now"),
        ("paused", None, "Updated just now"),
    ];
    for (status, timestamp, expected) in cases {
        let mut value = project("p", "A", json!(1));
        value["status"]["type"] = json!(status);
        value["priority"] = json!(9);
        value["health"] = json!("futureHealth");
        if let Some((field, date)) = timestamp {
            value[field] = json!(date);
        }
        let response = page(vec![value], false, None);
        let text = render_text(&response.projects.nodes, UNIX_EPOCH, 120, false).unwrap();
        assert!(text.contains(expected), "{status}: {text}");
        assert!(
            text.contains("9        futureHealth"),
            "unknown priority and health: {text}"
        );
    }
}

#[test]
fn schema_non_null_sort_and_raw_unknown_enums_are_typed() {
    let valid = json!({"data":{"projects":{"nodes":[project("a","A",json!(1))],"pageInfo":{"hasNextPage":false,"endCursor":null}}}});
    let invalid = valid
        .to_string()
        .replace("\"sortOrder\":1", "\"sortOrder\":null");
    assert!(matches!(
        parse_response::<GetProjects>(invalid.as_bytes()),
        Err(ResponseError::UnexpectedShape(_))
    ));
    let mut unknown = valid;
    unknown["data"]["projects"]["nodes"][0]["health"] = json!("futureHealth");
    unknown["data"]["projects"]["nodes"][0]["status"]["type"] = json!("futureStatus");
    let parsed: GetProjects =
        parse_response(unknown.to_string().as_bytes()).expect("raw future enum values");
    assert_eq!(
        parsed.projects.nodes[0].health.as_ref().unwrap().as_str(),
        "futureHealth"
    );
    assert_eq!(
        parsed.projects.nodes[0].status.status_type.as_str(),
        "futureStatus"
    );
}
