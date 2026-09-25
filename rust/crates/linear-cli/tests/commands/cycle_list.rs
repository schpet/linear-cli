use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::ready;
use std::rc::Rc;

use linear_cli::commands::cycle_list::{render_text, run_with};
use linear_cli::graphql::envelope::parse_response;
use linear_cli::graphql::operations::cycles::{Cycle, GetTeamCycles};
use linear_cli::json_number::finite_js_number;
use serde_json::{Value, json};

fn cycle(
    id: &str,
    number: Value,
    name: Value,
    starts_at: &str,
    flags: (bool, bool, bool),
    completed: Value,
) -> Value {
    let (active, future, past) = flags;
    json!({
        "id": id,
        "number": number,
        "name": name,
        "startsAt": starts_at,
        "endsAt": "2026-12-31T00:00:00.000Z",
        "completedAt": completed,
        "isActive": active,
        "isFuture": future,
        "isPast": past
    })
}

fn page(nodes: Vec<Value>, has_next_page: bool, end_cursor: Option<&str>) -> GetTeamCycles {
    let body = json!({"data":{"team":{
        "id":"team-eng-id",
        "name":"Engineering",
        "cycles":{
            "nodes":nodes,
            "pageInfo":{"hasNextPage":has_next_page,"endCursor":end_cursor}
        }
    }}});
    parse_response(body.to_string().as_bytes()).expect("typed cycle page")
}

fn typed_cycle(value: Value) -> Cycle {
    page(vec![value], false, None).team.cycles.nodes.remove(0)
}

#[tokio::test]
async fn empty_connection_has_distinct_text_and_json_outputs() {
    let empty = page(Vec::new(), false, None);
    let text = run_with(
        "team-eng-id",
        |_| ready(Ok(empty.clone())),
        false,
        120,
        false,
    )
    .await
    .expect("empty text");
    assert_eq!(text, b"No cycles found for this team.\n");

    let json = run_with(
        "team-eng-id",
        |_| ready(Ok(empty.clone())),
        true,
        120,
        false,
    )
    .await
    .expect("empty JSON");
    assert_eq!(
        json,
        b"{\n  \"nodes\": [],\n  \"pageInfo\": {\n    \"hasNextPage\": false,\n    \"endCursor\": null\n  }\n}\n"
    );
}

#[test]
fn js_numbers_match_text_and_json_examples() {
    for (number, expected) in [
        (12.5, "12.5"),
        (12.0, "12"),
        (1e21, "1e+21"),
        (1.2e20, "120000000000000000000"),
        (1e-7, "1e-7"),
        (-0.0, "0"),
    ] {
        assert_eq!(
            finite_js_number(number).expect("finite number").get(),
            expected,
            "{number:?}"
        );
    }
}

#[tokio::test]
async fn json_collects_two_pages_sorts_stably_and_preserves_float_spellings() {
    let pages = Rc::new(RefCell::new(VecDeque::from([
        page(
            vec![
                cycle(
                    "past",
                    json!(12.0),
                    json!("Past"),
                    "2026-01-01T00:00:00.000Z",
                    (false, false, true),
                    Value::Null,
                ),
                cycle(
                    "fraction",
                    json!(12.5),
                    json!("Fraction"),
                    "2026-03-01T00:00:00.000Z",
                    (false, false, false),
                    Value::Null,
                ),
            ],
            true,
            Some("next"),
        ),
        page(
            vec![cycle(
                "big",
                json!(1e21),
                json!("Big"),
                "2026-03-01T00:00:00.000Z",
                (false, false, false),
                Value::Null,
            )],
            false,
            Some("done"),
        ),
    ])));
    let requests = Rc::new(RefCell::new(Vec::new()));
    let output = run_with(
        "team-eng-id",
        {
            let pages = Rc::clone(&pages);
            let requests = Rc::clone(&requests);
            move |request| {
                requests
                    .borrow_mut()
                    .push(serde_json::to_value(request).expect("request"));
                ready(Ok(pages.borrow_mut().pop_front().expect("two pages")))
            }
        },
        true,
        120,
        false,
    )
    .await
    .expect("cycles");
    let text = String::from_utf8(output).expect("JSON UTF-8");
    assert!(text.starts_with("{\n  \"nodes\": [\n"));
    assert!(text.ends_with("\n"));
    assert!(text.contains("\"number\": 12.5"));
    assert!(text.contains("\"number\": 1e+21"));
    assert!(text.contains("\"number\": 12,"));
    let result: Value = serde_json::from_str(&text).expect("JSON shape");
    assert_eq!(result["nodes"][0]["id"], "fraction");
    assert_eq!(result["nodes"][1]["id"], "big");
    assert_eq!(result["nodes"][2]["id"], "past");
    assert_eq!(
        result["pageInfo"],
        json!({"hasNextPage":false,"endCursor":"done"})
    );
    let requests = requests.borrow();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[0]["variables"],
        json!({"teamId":"team-eng-id","first":100})
    );
    assert_eq!(
        requests[1]["variables"],
        json!({"teamId":"team-eng-id","first":100,"after":"next"})
    );
    assert!(
        requests[0]["query"]
            .as_str()
            .expect("query")
            .contains("query GetTeamCycles")
    );
}

#[tokio::test]
async fn missing_and_empty_cursor_discard_partial_nodes() {
    for cursor in [None, Some("")] {
        let result = run_with(
            "team-eng-id",
            |_| {
                ready(Ok(page(
                    vec![cycle(
                        "one",
                        json!(1),
                        Value::Null,
                        "2026-01-01T00:00:00Z",
                        (false, false, false),
                        Value::Null,
                    )],
                    true,
                    cursor,
                )))
            },
            false,
            120,
            false,
        )
        .await
        .expect_err("missing cursor");
        assert_eq!(
            result.display_message(),
            "Failed to list cycles: Linear reported more cycles but returned no pagination cursor"
        );
        assert_eq!(result.suggestion.as_deref(), Some("Retry the command."));
    }
}

#[tokio::test]
async fn repeated_cursor_stops_without_requesting_a_third_page() {
    let pages = Rc::new(RefCell::new(VecDeque::from([
        page(Vec::new(), true, Some("same")),
        page(Vec::new(), true, Some("same")),
    ])));
    let requests = Rc::new(RefCell::new(0));
    let error = run_with(
        "team-eng-id",
        {
            let pages = Rc::clone(&pages);
            let requests = Rc::clone(&requests);
            move |_| {
                *requests.borrow_mut() += 1;
                ready(Ok(pages.borrow_mut().pop_front().expect("two pages")))
            }
        },
        true,
        120,
        false,
    )
    .await
    .expect_err("repeated cursor");
    assert_eq!(*requests.borrow(), 2);
    assert_eq!(
        error.display_message(),
        "Failed to list cycles: Linear repeated a cycle pagination cursor on page 2"
    );
}

#[test]
fn text_keeps_status_label_separate_from_color_and_falls_back_on_empty_name() {
    let cycles = vec![
        typed_cycle(cycle(
            "future-past",
            json!(2),
            json!(""),
            "2026-09-01T00:00:00Z",
            (false, true, true),
            Value::Null,
        )),
        typed_cycle(cycle(
            "active",
            json!(1),
            Value::Null,
            "2026-08-01T00:00:00Z",
            (true, false, false),
            Value::Null,
        )),
        typed_cycle(cycle(
            "completed-empty",
            json!(3),
            json!(""),
            "2026-07-01T00:00:00Z",
            (false, false, false),
            json!(""),
        )),
    ];
    let plain = render_text(&cycles, 120, false).expect("plain table");
    assert!(plain.contains("2 Cycle 2 2026-09-01 2026-12-31 Upcoming "));
    assert!(plain.contains("1 Cycle 1 2026-08-01 2026-12-31 Active   "));
    let color = render_text(&cycles, 120, true).expect("colored table");
    assert!(color.starts_with("\x1b[1m\x1b[4m"));
    assert!(color.contains("\x1b[90mUpcoming \x1b[39m"));
    assert!(color.contains("\x1b[32mActive   \x1b[39m"));
    assert!(color.contains("\x1b[90mCompleted\x1b[39m"));
}
