use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::ready;
use std::rc::Rc;

use serde_json::{Value, json};

use super::resolve_id_with;
use crate::graphql::envelope::parse_response;
use crate::graphql::operations::cycle::GetTeamCyclesForLookup;
use crate::refs::{CycleSelector, LinearUrlRef};

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

fn null_cycles(key: &str, enabled: bool) -> GetTeamCyclesForLookup {
    let body = json!({"data":{"team":{
        "key":key,"cyclesEnabled":enabled,"cycles":null,"activeCycle":null
    }}});
    parse_response(body.to_string().as_bytes()).expect("typed lookup page")
}

fn cycle_url(team_key: &str) -> LinearUrlRef {
    LinearUrlRef::Cycle {
        workspace: "example".to_owned(),
        team_key: team_key.to_owned(),
        cycle: CycleSelector::Number(5),
    }
}

#[tokio::test]
async fn every_page_is_fetched_and_a_number_match_beats_a_later_name_match() {
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
    assert_eq!(requests[0], json!({"teamId":"team-id","after":null}));
    assert_eq!(requests[1], json!({"teamId":"team-id","after":"next"}));
}

#[tokio::test]
async fn disabled_cycles_fail_on_the_first_page() {
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
async fn team_checks_come_before_a_missing_cycle_list() {
    let mismatch = resolve_id_with("team-id", "5", Some(&cycle_url("URL")), |_| {
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
async fn url_team_key_matches_case_insensitively() {
    let mut first = page(
        vec![node("five", json!(5), json!("Five"))],
        false,
        None,
        true,
    );
    first.team.as_mut().expect("team").key = "é".to_owned();
    let id = resolve_id_with("team-id", "url", Some(&cycle_url("É")), |_| {
        ready(Ok(first.clone()))
    })
    .await
    .expect("Unicode uppercase match");
    assert_eq!(id, "five");
}
