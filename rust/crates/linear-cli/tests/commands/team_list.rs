use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::ready;
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use linear_cli::commands::team::list::{render_text, run_with};
use linear_cli::graphql::envelope::parse_response;
use linear_cli::graphql::operations::teams::GetTeams;
use serde_json::{Value, json};

fn team(id: &str, name: &str, archived_at: Value, updated_at: &str) -> Value {
    json!({
        "id": id,
        "name": name,
        "key": id,
        "description": null,
        "icon": null,
        "color": "#4466aa",
        "cyclesEnabled": true,
        "createdAt": "2025-01-01T00:00:00.000Z",
        "updatedAt": updated_at,
        "archivedAt": archived_at,
        "organization": {"id":"org-fake-1","name":"Example Org"}
    })
}

fn page(nodes: Vec<Value>, has_next_page: bool, end_cursor: Option<&str>) -> GetTeams {
    let body = json!({"data":{"teams":{
        "nodes": nodes,
        "pageInfo":{"hasNextPage":has_next_page,"endCursor":end_cursor}
    }}});
    parse_response(body.to_string().as_bytes()).expect("typed team page")
}

#[tokio::test]
async fn json_paginates_filters_sorts_and_preserves_connection_shape() {
    let future = "9999-01-01T00:00:00.000Z";
    let pages = Rc::new(RefCell::new(VecDeque::from([
        page(
            vec![team("z", "Zulu", Value::Null, future)],
            true,
            Some("next"),
        ),
        page(
            vec![
                team("old", "Archived", json!("2025-02-01T00:00:00.000Z"), future),
                team("a", "Alpha", Value::Null, future),
            ],
            false,
            Some("done"),
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
        true,
        || UNIX_EPOCH,
        120,
    )
    .await
    .expect("teams");
    let output = String::from_utf8(output).expect("JSON UTF-8");
    let parsed: Value = serde_json::from_str(&output).expect("JSON shape");
    assert_eq!(parsed["nodes"].as_array().unwrap().len(), 2);
    assert_eq!(parsed["nodes"][0]["name"], "Alpha");
    assert_eq!(parsed["nodes"][1]["name"], "Zulu");
    assert_eq!(
        parsed["pageInfo"],
        json!({"hasNextPage":false,"endCursor":"done"})
    );
    assert!(output.starts_with("{\n  \"nodes\": ["));
    assert!(output.ends_with("\n"));
    let requests = requests.borrow();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0]["variables"], json!({"first":100}));
    assert_eq!(
        requests[1]["variables"],
        json!({"first":100,"after":"next"})
    );
}

#[tokio::test]
async fn missing_cursor_discards_nodes_and_returns_specific_guidance() {
    let result = run_with(
        |_| {
            ready(Ok(page(
                vec![team("a", "Alpha", Value::Null, "9999-01-01T00:00:00.000Z")],
                true,
                None,
            )))
        },
        true,
        || UNIX_EPOCH,
        120,
    )
    .await
    .expect_err("missing cursor");
    assert_eq!(
        result.to_string(),
        "Failed to fetch teams: Linear reported more teams but returned no pagination cursor"
    );
    assert_eq!(result.hint(), Some("Retry the command."));
}

#[tokio::test]
async fn text_handles_invalid_dates_and_narrow_terminals_without_clock_flakiness() {
    let value = page(
        vec![team("t-raw", "Raw Date", json!(""), "not-a-date")],
        false,
        None,
    );
    let output = run_with(|_| ready(Ok(value.clone())), false, || UNIX_EPOCH, 120)
        .await
        .expect("table");
    assert_eq!(
        output,
        b"KEY   NAME     CYCLES UPDATED    ID   \nt-raw Raw Date Yes    not-a-date t-raw\n"
    );

    let future = page(
        vec![team(
            "id",
            "A long team name",
            Value::Null,
            "9999-01-01T00:00:00.000Z",
        )],
        false,
        None,
    );
    let output = run_with(
        |_| ready(Ok(future.clone())),
        false,
        || UNIX_EPOCH + Duration::from_secs(1),
        10,
    )
    .await
    .expect("narrow table");
    assert!(String::from_utf8_lossy(&output).contains("A long team name"));
}

#[tokio::test]
async fn repeated_cursor_fails_before_a_third_request() {
    let calls = Rc::new(RefCell::new(0));
    let result = run_with(
        {
            let calls = Rc::clone(&calls);
            move |_| {
                *calls.borrow_mut() += 1;
                ready(Ok(page(vec![], true, Some("same"))))
            }
        },
        true,
        SystemTime::now,
        120,
    )
    .await
    .expect_err("repeated cursor");
    assert_eq!(*calls.borrow(), 2);
    assert!(
        result
            .to_string()
            .contains("repeated a team pagination cursor")
    );
}

#[test]
fn terminal_table_and_spinner_use_the_observed_control_sequences() {
    let data = page(
        vec![team(
            "id1",
            "Alpha",
            Value::Null,
            "9999-01-01T00:00:00.000Z",
        )],
        false,
        None,
    );
    let styled = render_text(&data.teams.nodes, UNIX_EPOCH, 120, true);
    assert!(styled.starts_with("\x1b[4mKEY\x1b[24m \x1b[4mNAME \x1b[24m"));
    assert!(styled.contains("\x1b[38;2;68;102;170mid1\x1b[39m"));
    assert!(styled.contains("\x1b[38;2;128;128;128mjust now\x1b[39m"));
    assert!(styled.ends_with("\x1b[39m\x1b[0m\n"));
}
