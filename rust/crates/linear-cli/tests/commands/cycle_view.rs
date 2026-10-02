use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::ready;
use std::rc::Rc;

use linear_cli::commands::cycle_view::{
    detail_request, json, lookup_request, markdown, resolve_id_with,
};
use linear_cli::graphql::envelope::parse_response;
use linear_cli::graphql::operations::cycle_view::{GetCycleDetails, GetTeamCyclesForLookup};
use serde_json::{Value, json};

fn node(id: &str, number: Value, name: Value) -> Value {
    json!({"id":id,"number":number,"name":name,"startsAt":"2026-09-01T00:00:00.000Z","isNext":false,"isPrevious":false})
}
fn page(
    nodes: Vec<Value>,
    next: bool,
    cursor: Option<&str>,
    enabled: bool,
) -> GetTeamCyclesForLookup {
    let body = json!({"data":{"team":{"key":"ENG","cyclesEnabled":enabled,
        "cycles":{"nodes":nodes,"pageInfo":{"hasNextPage":next,"endCursor":cursor}},
        "activeCycle":{"id":"active","number":5,"name":"Current"}}}});
    parse_response(body.to_string().as_bytes()).expect("typed lookup page")
}

#[tokio::test]
async fn sends_explicit_null_then_empty_cursor_and_searches_all_pages_in_order() {
    let pages = Rc::new(RefCell::new(VecDeque::from([
        page(
            vec![node("first-number", json!(5), json!("Other"))],
            true,
            Some(""),
            true,
        ),
        page(
            vec![node("later-name", json!(6), json!("5"))],
            false,
            None,
            true,
        ),
    ])));
    let requests = Rc::new(RefCell::new(Vec::new()));
    let id = resolve_id_with("team-id", "5", None, {
        let pages = Rc::clone(&pages);
        let requests = Rc::clone(&requests);
        move |request| {
            requests
                .borrow_mut()
                .push(serde_json::to_value(request).expect("request"));
            ready(Ok(pages.borrow_mut().pop_front().expect("page")))
        }
    })
    .await
    .expect("cycle");
    assert_eq!(id, "first-number");
    let requests = requests.borrow();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[0]["variables"],
        json!({"teamId":"team-id","after":null})
    );
    assert_eq!(
        requests[1]["variables"],
        json!({"teamId":"team-id","after":""})
    );
    assert!(
        requests[0]["query"]
            .as_str()
            .expect("query")
            .contains("cycles(first: 250, after: $after)")
    );
}

#[tokio::test]
async fn first_page_validation_precedes_next_request() {
    let calls = Rc::new(RefCell::new(0));
    let error = resolve_id_with("team-id", "5", None, {
        let calls = Rc::clone(&calls);
        move |_| {
            *calls.borrow_mut() += 1;
            ready(Ok(page(vec![], true, Some("more"), false)))
        }
    })
    .await
    .expect_err("disabled");
    assert_eq!(*calls.borrow(), 1);
    assert_eq!(error.message(), "Cycles are not enabled for team ENG");
}

#[tokio::test]
async fn repeated_cursor_is_a_protocol_error() {
    let pages = Rc::new(RefCell::new(VecDeque::from([
        page(vec![], true, Some("same"), true),
        page(vec![], true, Some("same"), true),
    ])));
    let error = resolve_id_with("team-id", "5", None, move |_| {
        ready(Ok(pages.borrow_mut().pop_front().expect("page")))
    })
    .await
    .expect_err("repeated cursor");
    assert!(
        error
            .message()
            .contains("repeated a cycle pagination cursor")
    );
}

#[test]
fn detail_json_selects_only_wire_fields_and_preserves_null_name() {
    let body = json!({"data":{"cycle":{
        "id":"cycle-id","number":12,"name":null,"description":null,
        "startsAt":"2026-09-01T00:00:00.000Z","endsAt":"2026-09-08T00:00:00.000Z",
        "completedAt":null,"isActive":true,"isFuture":false,"isPast":false,
        "createdAt":"2026-01-01T00:00:00.000Z","updatedAt":"2026-01-02T00:00:00.000Z",
        "team":{"id":"team-id","key":"ENG","name":"Engineering","extra":"omitted"},
        "issues":{"nodes":[],"pageInfo":{"hasNextPage":true,"endCursor":"cursor"}},
        "serverOnly":"omitted"
    }}});
    let details: GetCycleDetails = parse_response(body.to_string().as_bytes()).expect("details");
    let cycle = details.cycle.expect("cycle");
    let output = String::from_utf8(json(&cycle).expect("json")).expect("utf8");
    assert!(output.contains("\"number\": 12,"));
    assert!(output.contains("\"name\": null"));
    assert!(output.contains("\"hasNextPage\": true"));
    assert!(!output.contains("serverOnly"));
    assert!(!output.contains("extra"));
    let text = markdown(&cycle, chrono::Utc::now(), &chrono::Utc).expect("markdown");
    assert!(text.starts_with("# Cycle 12\n\n**Number:** 12"));
    assert!(text.ends_with("_No issues in this cycle yet._"));
    assert_eq!(
        serde_json::to_value(detail_request("cycle-id")).expect("request")["variables"],
        json!({"id":"cycle-id"})
    );
    assert_eq!(
        serde_json::to_value(lookup_request("team-id", None)).expect("request")["variables"],
        json!({"teamId":"team-id","after":null})
    );
}

#[test]
fn null_connection_preserves_page_one_validation_order_and_key() {
    use linear_cli::commands::cycle_view::classify_lookup_page;
    use linear_cli::graphql::transport::RawHttpResponse;
    use linear_cli::refs::{CycleSelector, LinearUrlRef};
    let url = LinearUrlRef::Cycle {
        workspace: "example".to_owned(),
        team_key: "URL".to_owned(),
        cycle: CycleSelector::Number(5),
    };
    let response = |enabled: bool| RawHttpResponse {
        status: reqwest::StatusCode::OK,
        headers: reqwest::header::HeaderMap::new(),
        body: json!({"data":{"team":{
            "key":"WIRE","cyclesEnabled":enabled,"cycles":null,"activeCycle":null
        }}})
        .to_string()
        .into_bytes(),
    };
    let mut first_key = None;
    let mismatch = classify_lookup_page(response(false), 1, Some(&url), &mut first_key)
        .expect_err("URL mismatch precedes disabled and null connection");
    assert_eq!(
        mismatch.message(),
        "That cycle URL is for team URL, but this command is working in team WIRE."
    );
    assert_eq!(first_key.as_deref(), Some("WIRE"));
    let disabled = classify_lookup_page(response(false), 1, None, &mut first_key)
        .expect_err("disabled precedes null connection");
    assert_eq!(disabled.message(), "Cycles are not enabled for team WIRE");
    let first_null =
        classify_lookup_page(response(true), 1, None, &mut first_key).expect_err("null first page");
    assert_eq!(
        first_null.message(),
        "Linear returned a null cycle connection for team WIRE on page 1"
    );
    let later =
        classify_lookup_page(response(true), 2, None, &mut first_key).expect_err("null later page");
    assert_eq!(
        later.message(),
        "Linear returned a null cycle connection for team WIRE on page 2"
    );
}

#[tokio::test]
async fn unicode_url_team_case_matches_page_one_wire_key() {
    use linear_cli::refs::{CycleSelector, LinearUrlRef};
    let url = LinearUrlRef::Cycle {
        workspace: "example".to_owned(),
        team_key: "É".to_owned(),
        cycle: CycleSelector::Number(5),
    };
    let mut first = page(
        vec![node("five", json!(5), json!("Five"))],
        false,
        None,
        true,
    );
    first.team.as_mut().expect("team").key = "é".to_owned();
    let id = resolve_id_with("team-id", "url", Some(&url), |_| ready(Ok(first.clone())))
        .await
        .expect("Unicode uppercase match");
    assert_eq!(id, "five");

    use linear_cli::commands::cycle_view::classify_lookup_page;
    use linear_cli::graphql::transport::RawHttpResponse;
    let response = RawHttpResponse {
        status: reqwest::StatusCode::OK,
        headers: reqwest::header::HeaderMap::new(),
        body: json!({"data":{"team":{
            "key":"é","cyclesEnabled":true,"cycles":null,"activeCycle":null
        }}})
        .to_string()
        .into_bytes(),
    };
    let error = classify_lookup_page(response, 1, Some(&url), &mut None)
        .expect_err("null connection after matching Unicode team");
    assert_eq!(
        error.message(),
        "Linear returned a null cycle connection for team é on page 1"
    );
}

#[tokio::test]
async fn missing_and_nonadjacent_repeated_cursors_stop_without_extra_requests() {
    let calls = Rc::new(RefCell::new(0));
    let missing = resolve_id_with("team-id", "5", None, {
        let calls = Rc::clone(&calls);
        move |_| {
            *calls.borrow_mut() += 1;
            ready(Ok(page(vec![], true, None, true)))
        }
    })
    .await
    .expect_err("missing cursor");
    assert_eq!(*calls.borrow(), 1);
    assert!(missing.message().contains("no cycle pagination cursor"));

    let pages = Rc::new(RefCell::new(VecDeque::from([
        page(vec![], true, Some("A"), true),
        page(vec![], true, Some("B"), true),
        page(vec![], true, Some("A"), true),
    ])));
    let calls = Rc::new(RefCell::new(0));
    let repeated = resolve_id_with("team-id", "5", None, {
        let pages = Rc::clone(&pages);
        let calls = Rc::clone(&calls);
        move |_| {
            *calls.borrow_mut() += 1;
            ready(Ok(pages.borrow_mut().pop_front().expect("page")))
        }
    })
    .await
    .expect_err("nonadjacent repeated cursor");
    assert_eq!(*calls.borrow(), 3);
    assert!(
        repeated
            .message()
            .contains("repeated a cycle pagination cursor")
    );
}
