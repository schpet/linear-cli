use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::ready;
use std::rc::Rc;

use chrono::{TimeZone, Utc};
use linear_cli::commands::initiative_comment_list::{render_json, request, run_with};
use linear_cli::error::Error;
use linear_cli::graphql::envelope::{GraphQlRequest, parse_response};
use linear_cli::graphql::operations::initiative_comments::{
    GetInitiativeComments, GetInitiativeCommentsVariables,
};
use serde_json::{Value, json};

const INITIATIVE: &str = "3b9a5c7e-1d2f-4a6b-8c9d-0e1f2a3b4c5d";
type Sent = Rc<RefCell<Vec<Value>>>;
type PageResult = Result<GetInitiativeComments, Error>;
type Request = GraphQlRequest<GetInitiativeCommentsVariables>;

fn frozen(id: &str) -> Value {
    let path = format!(
        "{}/../../parity/runner/c043-frozen-cases/{id}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_slice(&std::fs::read(path).expect("frozen case")).expect("case JSON")
}

fn frozen_page(id: &str) -> GetInitiativeComments {
    let case = frozen(id);
    let data = &case["graphql"]["groups"][0]["steps"][0]["response"]["data"];
    parse_response(json!({"data": data}).to_string().as_bytes()).expect("typed page")
}

fn page(nodes: Value, next: bool, cursor: Value) -> GetInitiativeComments {
    parse_response(
        json!({"data":{
            "initiative":{"id":INITIATIVE,"name":"Mobile App"},
            "comments":{"nodes":nodes,"pageInfo":{"hasNextPage":next,"endCursor":cursor}}
        }})
        .to_string()
        .as_bytes(),
    )
    .expect("typed page")
}

fn scripted(
    pages: Vec<PageResult>,
) -> (Sent, impl FnMut(Request) -> std::future::Ready<PageResult>) {
    let sent = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&sent);
    let queue = Rc::new(RefCell::new(VecDeque::from(pages)));
    let fetch = move |request: Request| {
        recorded.borrow_mut().push(
            serde_json::to_value(request.variables.expect("variables")).expect("variables JSON"),
        );
        ready(queue.borrow_mut().pop_front().expect("unexpected page"))
    };
    (sent, fetch)
}

#[test]
fn request_uses_root_connection_dual_typed_ids_and_explicit_null_cursor() {
    let first = request(INITIATIVE, None);
    let query = &first.query;
    assert!(query.contains("initiative(id: $id)"), "{query}");
    assert!(query.contains("comments("), "{query}");
    assert!(query.contains("orderBy: createdAt"), "{query}");
    assert!(query.contains("eq: $filterId"), "{query}");
    assert!(query.contains("$id: String!"), "{query}");
    assert!(query.contains("$filterId: ID!"), "{query}");
    let values = serde_json::to_value(first.variables.expect("variables")).expect("JSON");
    assert_eq!(
        values,
        json!({"id":INITIATIVE,"filterId":INITIATIVE,"after":null})
    );
    let second = request(INITIATIVE, Some(String::new()));
    assert_eq!(
        serde_json::to_value(second.variables.expect("variables")).expect("JSON"),
        json!({"id":INITIATIVE,"filterId":INITIATIVE,"after":""})
    );
}

#[test]
fn frozen_full_json_keeps_selected_graphql_field_names_and_order() {
    let case = frozen("c043-uuid-json-full");
    let page = frozen_page("c043-uuid-json-full");
    let output = render_json(&page.comments.nodes, &page.comments.page_info).expect("JSON");
    assert_eq!(
        String::from_utf8(output).expect("UTF-8"),
        case["expected"]["stdout"]["utf8"].as_str().expect("stdout")
    );
}

#[tokio::test]
async fn pages_aggregate_without_sorting_json_and_send_the_prior_cursor() {
    let case = frozen("c043-two-page-json");
    let steps = case["graphql"]["groups"][0]["steps"]
        .as_array()
        .expect("steps");
    let pages = steps
        .iter()
        .map(|step| {
            parse_response::<GetInitiativeComments>(
                json!({"data":step["response"]["data"]})
                    .to_string()
                    .as_bytes(),
            )
            .map_err(Error::from)
        })
        .collect();
    let (sent, fetch) = scripted(pages);
    let now = Utc
        .with_ymd_and_hms(2026, 9, 28, 0, 0, 0)
        .single()
        .expect("now");
    let output = run_with(INITIATIVE, INITIATIVE, fetch, true, false, now)
        .await
        .expect("pages");
    assert_eq!(
        String::from_utf8(output).expect("UTF-8"),
        case["expected"]["stdout"]["utf8"].as_str().expect("stdout")
    );
    let expected: Vec<Value> = steps
        .iter()
        .map(|step| step["operation"]["variables"].clone())
        .collect();
    assert_eq!(*sent.borrow(), expected);
}

#[tokio::test]
async fn cursor_failure_and_later_null_initiative_discard_prior_pages() {
    let now = Utc
        .with_ymd_and_hms(2026, 9, 28, 0, 0, 0)
        .single()
        .expect("now");
    for cursor in [Value::Null, json!("same")] {
        let pages = if cursor.is_null() {
            vec![Ok(page(json!([]), true, cursor))]
        } else {
            vec![
                Ok(page(json!([]), true, cursor.clone())),
                Ok(page(json!([]), true, cursor)),
            ]
        };
        let (sent, fetch) = scripted(pages);
        let error = run_with(INITIATIVE, INITIATIVE, fetch, true, false, now)
            .await
            .expect_err("cursor");
        assert_eq!(
            error.to_string(),
            "Failed to list comments: Linear reported more comments but did not return a usable cursor"
        );
        assert_eq!(
            error.hint(),
            Some("Rerun the command; if it persists, report it.")
        );
        assert!(!sent.borrow().is_empty());
    }
    let null_initiative: GetInitiativeComments = parse_response(
        json!({"data":{"initiative":null,"comments":{"nodes":[],"pageInfo":{"hasNextPage":false,"endCursor":null}}}})
            .to_string().as_bytes(),
    ).expect("nullable initiative");
    let (sent, fetch) = scripted(vec![
        Ok(page(json!([]), true, json!("next"))),
        Ok(null_initiative),
    ]);
    let error = run_with("Original Name", INITIATIVE, fetch, true, false, now)
        .await
        .expect_err("later null");
    assert_eq!(
        error.to_string(),
        "Failed to list comments: Initiative not found: Original Name"
    );
    assert_eq!(error.hint(), None);
    assert_eq!(sent.borrow().len(), 2);
}
