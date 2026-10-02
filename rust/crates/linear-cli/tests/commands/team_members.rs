use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::ready;
use std::rc::Rc;

use linear_cli::commands::team_members::{Options, run_with};
use linear_cli::graphql::envelope::parse_response;
use linear_cli::graphql::operations::team_members::GetTeamMembers;
use serde_json::{Value, json};

fn member(display_name: &str, active: bool) -> Value {
    json!({
        "id":"user-one", "name":"Ada Lovelace", "displayName":display_name,
        "email":"ada@example.invalid", "active":active, "initials":"AL",
        "description":null, "timezone":null, "lastSeen":null,
        "statusEmoji":null, "statusLabel":null, "guest":false,
        "isAssignable":true, "admin":false, "owner":false, "isMe":false,
        "url":"https://linear.app/example/profiles/ada"
    })
}

fn page(nodes: Vec<Value>, more: bool, cursor: Option<&str>) -> GetTeamMembers {
    let body = json!({"data":{"team":{"members":{
        "nodes":nodes,
        "pageInfo":{"hasNextPage":more,"endCursor":cursor}
    }}}});
    parse_response(body.to_string().as_bytes()).expect("typed member page")
}

#[tokio::test]
async fn json_uses_typed_query_variables_all_pages_and_active_filter() {
    let pages = Rc::new(RefCell::new(VecDeque::from([
        page(vec![member("Zed", false)], true, Some("cursor-one")),
        page(vec![member("Ada", true)], false, Some("cursor-two")),
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
        "ENG",
        Options {
            all: false,
            json: true,
        },
    )
    .await
    .expect("member JSON");
    let output = String::from_utf8(output).expect("UTF-8");
    assert!(output.ends_with('\n'));
    let parsed: Value = serde_json::from_str(&output).expect("connection");
    assert_eq!(parsed["nodes"].as_array().unwrap().len(), 1);
    assert_eq!(parsed["nodes"][0]["displayName"], "Ada");
    assert_eq!(
        parsed["pageInfo"],
        json!({"hasNextPage":false,"endCursor":"cursor-two"})
    );
    let requests = requests.borrow();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0]["operationName"], "GetTeamMembers");
    assert_eq!(
        requests[0]["variables"],
        json!({"teamKey":"ENG","includeDisabled":false,"first":100})
    );
    assert_eq!(
        requests[1]["variables"],
        json!({"teamKey":"ENG","includeDisabled":false,"first":100,"after":"cursor-one"})
    );
    let query = requests[0]["query"].as_str().unwrap();
    assert_eq!(
        query,
        concat!(
            "query GetTeamMembers($teamKey: String!, $includeDisabled: Boolean!, $first: Int, $after: String) {\n",
            "  team(id: $teamKey) {\n",
            "    members(includeDisabled: $includeDisabled, first: $first, after: $after) {\n",
            "      nodes {\n",
            "        id\n        name\n        displayName\n        email\n        active\n",
            "        initials\n        description\n        timezone\n        lastSeen\n",
            "        statusEmoji\n        statusLabel\n        guest\n        isAssignable\n",
            "        admin\n        owner\n        isMe\n        url\n",
            "      }\n",
            "      pageInfo {\n        hasNextPage\n        endCursor\n      }\n",
            "    }\n  }\n}\n",
        )
    );
}

#[tokio::test]
async fn empty_cursor_is_sent_once_then_rejected_on_nonadvance() {
    let requests = Rc::new(RefCell::new(Vec::new()));
    let result = run_with(
        {
            let requests = Rc::clone(&requests);
            move |request| {
                requests
                    .borrow_mut()
                    .push(serde_json::to_value(&request).unwrap());
                ready(Ok(page(Vec::new(), true, Some(""))))
            }
        },
        "ENG",
        Options {
            all: false,
            json: true,
        },
    )
    .await
    .expect_err("stalled empty cursor");
    assert_eq!(
        result.to_string(),
        "Failed to fetch team members: Linear reported more team members but did not advance the page cursor"
    );
    assert_eq!(result.hint(), None);
    let requests = requests.borrow();
    assert_eq!(requests.len(), 2);
    assert!(requests[0]["variables"].get("after").is_none());
    assert_eq!(requests[1]["variables"]["after"], "");
}

#[tokio::test]
async fn cursor_cycle_is_stopped_after_the_repeated_page() {
    let pages = Rc::new(RefCell::new(VecDeque::from([
        page(Vec::new(), true, Some("A")),
        page(Vec::new(), true, Some("B")),
        page(Vec::new(), true, Some("A")),
    ])));
    let requests = Rc::new(RefCell::new(Vec::new()));
    let result = run_with(
        {
            let pages = Rc::clone(&pages);
            let requests = Rc::clone(&requests);
            move |request| {
                requests
                    .borrow_mut()
                    .push(serde_json::to_value(&request).unwrap());
                ready(Ok(pages.borrow_mut().pop_front().expect("three pages")))
            }
        },
        "ENG",
        Options {
            all: false,
            json: true,
        },
    )
    .await
    .expect_err("cursor cycle");
    assert_eq!(
        result.message(),
        "Linear reported more team members but did not advance the page cursor"
    );
    let requests = requests.borrow();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[1]["variables"]["after"], "A");
    assert_eq!(requests[2]["variables"]["after"], "B");
}

#[test]
fn required_null_member_fields_fail_typed_decode() {
    let null_team = json!({"data":{"team":null}});
    assert!(parse_response::<GetTeamMembers>(null_team.to_string().as_bytes()).is_err());
    let mut missing_display = member("Ada", true);
    missing_display["displayName"] = Value::Null;
    let body = json!({"data":{"team":{"members":{
        "nodes":[missing_display],
        "pageInfo":{"hasNextPage":false,"endCursor":null}
    }}}});
    assert!(parse_response::<GetTeamMembers>(body.to_string().as_bytes()).is_err());
}

#[tokio::test]
async fn all_keeps_inactive_and_renders_independent_markers() {
    let mut marked = member("", false);
    marked["guest"] = json!(true);
    marked["isAssignable"] = json!(false);
    marked["admin"] = json!(true);
    marked["owner"] = json!(true);
    marked["isMe"] = json!(true);
    marked["description"] = json!("Owner");
    marked["statusEmoji"] = json!("🔥");
    marked["statusLabel"] = json!("Focus");
    marked["lastSeen"] = json!("2026-01-02T03:04:05Z");
    let output = run_with(
        |request| {
            let body = serde_json::to_value(&request).unwrap();
            assert_eq!(body["variables"]["includeDisabled"], true);
            ready(Ok(page(vec![marked.clone()], false, None)))
        },
        "ENG",
        Options {
            all: true,
            json: false,
        },
    )
    .await
    .expect("text");
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("Ada Lovelace (Ada Lovelace) [AL] (inactive) (guest) (not assignable) (admin) (owner) (you)\n"));
    assert!(text.contains("  Status: 🔥 Focus\n  Last seen: "));
}

#[tokio::test]
async fn date_only_last_seen_keeps_raw_json_and_shows_a_local_time() {
    let mut dated = member("Ada", true);
    dated["lastSeen"] = json!("2026-01-02");
    let text = run_with(
        |_request| ready(Ok(page(vec![dated.clone()], false, None))),
        "ENG",
        Options {
            all: false,
            json: false,
        },
    )
    .await
    .expect("text result");
    assert!(
        String::from_utf8(text)
            .expect("UTF-8")
            .contains("  Last seen: 1/")
    );

    let json_output = run_with(
        |_request| ready(Ok(page(vec![dated.clone()], false, None))),
        "ENG",
        Options {
            all: false,
            json: true,
        },
    )
    .await
    .expect("JSON result");
    let parsed: Value = serde_json::from_slice(&json_output).expect("JSON connection");
    assert_eq!(parsed["nodes"][0]["lastSeen"], "2026-01-02");
}
