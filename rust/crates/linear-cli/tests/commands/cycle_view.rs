use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::ready;
use std::rc::Rc;

use linear_cli::commands::cycle::view::resolve_id_with;
use linear_cli::graphql::envelope::parse_response;
use linear_cli::graphql::operations::cycle_view::GetTeamCyclesForLookup;
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
async fn sends_explicit_null_then_the_cursor_and_searches_all_pages_in_order() {
    let pages = Rc::new(RefCell::new(VecDeque::from([
        page(
            vec![node("first-number", json!(5), json!("Other"))],
            true,
            Some("next"),
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
        json!({"teamId":"team-id","after":"next"})
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
        error.message().contains("same pagination cursor"),
        "{error}"
    );
}

fn null_cycles(key: &str, enabled: bool) -> GetTeamCyclesForLookup {
    let body = json!({"data":{"team":{
        "key":key,"cyclesEnabled":enabled,"cycles":null,"activeCycle":null
    }}});
    parse_response(body.to_string().as_bytes()).expect("typed lookup page")
}

#[tokio::test]
async fn null_cycles_are_reported_after_the_team_checks() {
    use linear_cli::refs::{CycleSelector, LinearUrlRef};
    let url = LinearUrlRef::Cycle {
        workspace: "example".to_owned(),
        team_key: "URL".to_owned(),
        cycle: CycleSelector::Number(5),
    };
    let mismatch = resolve_id_with("team-id", "5", Some(&url), |_| {
        ready(Ok(null_cycles("WIRE", false)))
    })
    .await
    .expect_err("URL mismatch comes first");
    assert_eq!(
        mismatch.message(),
        "That cycle URL is for team URL, but this command is working in team WIRE."
    );
    let disabled = resolve_id_with("team-id", "5", None, |_| {
        ready(Ok(null_cycles("WIRE", false)))
    })
    .await
    .expect_err("disabled comes next");
    assert_eq!(disabled.message(), "Cycles are not enabled for team WIRE");
    let missing = resolve_id_with("team-id", "5", None, |_| {
        ready(Ok(null_cycles("WIRE", true)))
    })
    .await
    .expect_err("null cycles");
    assert_eq!(
        missing.message(),
        "Linear returned no cycle list for team WIRE"
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
    assert!(missing.message().contains("no cursor"), "{missing}");

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
        repeated.message().contains("same pagination cursor"),
        "{repeated}"
    );
}
