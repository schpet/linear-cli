use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::ready;
use std::rc::Rc;

use chrono::{TimeZone, Utc};
use linear_cli::commands::project_comment_list::{render_json, render_text, request, run_with};
use linear_cli::error::AppError;
use linear_cli::graphql::envelope::{GraphQlRequest, parse_response};
use linear_cli::graphql::operations::project_comments::{
    GetProjectComments, GetProjectCommentsVariables,
};
use serde_json::{Value, json};

const PROJECT: &str = "3b9a5c7e-1d2f-4a6b-8c9d-0e1f2a3b4c5d";
type Sent = Rc<RefCell<Vec<Value>>>;
type PageResult = Result<GetProjectComments, AppError>;
type Request = GraphQlRequest<GetProjectCommentsVariables>;

fn frozen(id: &str) -> Value {
    let path = format!(
        "{}/../../parity/runner/c027-frozen-cases/{id}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_slice(&std::fs::read(path).expect("frozen case")).expect("case JSON")
}

fn frozen_page(id: &str) -> GetProjectComments {
    let case = frozen(id);
    let data = &case["graphql"]["groups"][0]["steps"][0]["response"]["data"];
    parse_response(json!({"data": data}).to_string().as_bytes()).expect("typed page")
}

fn page(nodes: Value, next: bool, cursor: Value) -> GetProjectComments {
    parse_response(
        json!({"data":{
            "project":{"id":PROJECT,"name":"Mobile App"},
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
    let first = request(PROJECT, None);
    let query = &first.query;
    assert!(query.contains("project(id: $id)"), "{query}");
    assert!(query.contains("comments("), "{query}");
    assert!(query.contains("orderBy: createdAt"), "{query}");
    assert!(query.contains("eq: $filterId"), "{query}");
    assert!(query.contains("$id: String!"), "{query}");
    assert!(query.contains("$filterId: ID!"), "{query}");
    let values = serde_json::to_value(first.variables.expect("variables")).expect("JSON");
    assert_eq!(
        values,
        json!({"id":PROJECT,"filterId":PROJECT,"after":null})
    );
    let second = request(PROJECT, Some(String::new()));
    assert_eq!(
        serde_json::to_value(second.variables.expect("variables")).expect("JSON"),
        json!({"id":PROJECT,"filterId":PROJECT,"after":""})
    );
}

#[test]
fn frozen_full_json_keeps_selected_graphql_field_names_and_order() {
    let case = frozen("c027-uuid-json-full");
    let page = frozen_page("c027-uuid-json-full");
    let output = render_json(&page.comments.nodes, &page.comments.page_info).expect("JSON");
    assert_eq!(
        String::from_utf8(output).expect("UTF-8"),
        case["expected"]["stdout"]["utf8"].as_str().expect("stdout")
    );
}

#[test]
fn frozen_offset_fraction_and_ties_render_in_source_order() {
    let case = frozen("c027-root-sort-ties");
    let page = frozen_page("c027-root-sort-ties");
    let now = Utc
        .with_ymd_and_hms(2026, 9, 28, 0, 0, 0)
        .single()
        .expect("now");
    assert_eq!(
        render_text(&page.comments.nodes, now, false),
        case["expected"]["stdout"]["utf8"].as_str().expect("stdout")
    );
}

#[test]
fn frozen_threads_preserve_quote_indentation_and_orphan_tail() {
    for id in [
        "c027-uuid-text-thread",
        "c027-orphan-reply",
        "c027-author-bot",
    ] {
        let case = frozen(id);
        let page = frozen_page(id);
        let now = Utc
            .with_ymd_and_hms(2026, 9, 28, 0, 0, 0)
            .single()
            .expect("now");
        assert_eq!(
            render_text(&page.comments.nodes, now, false),
            case["expected"]["stdout"]["utf8"].as_str().expect("stdout"),
            "{id}"
        );
    }
}

#[tokio::test]
async fn pages_aggregate_without_sorting_json_and_send_the_prior_cursor() {
    let case = frozen("c027-two-page-json");
    let steps = case["graphql"]["groups"][0]["steps"]
        .as_array()
        .expect("steps");
    let pages = steps
        .iter()
        .map(|step| {
            parse_response::<GetProjectComments>(
                json!({"data":step["response"]["data"]})
                    .to_string()
                    .as_bytes(),
            )
            .map_err(AppError::from)
        })
        .collect();
    let (sent, fetch) = scripted(pages);
    let now = Utc
        .with_ymd_and_hms(2026, 9, 28, 0, 0, 0)
        .single()
        .expect("now");
    let output = run_with(PROJECT, PROJECT, fetch, true, false, now)
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
async fn cursor_failure_and_later_null_project_discard_prior_pages() {
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
        let error = run_with(PROJECT, PROJECT, fetch, true, false, now)
            .await
            .expect_err("cursor");
        assert_eq!(
            error.to_string(),
            "Failed to list comments: Linear reported more comments but did not return a usable cursor"
        );
        assert_eq!(
            error.suggestion.as_deref(),
            Some("Rerun the command; if it persists, report it.")
        );
        assert!(!sent.borrow().is_empty());
    }
    let null_project: GetProjectComments = parse_response(
        json!({"data":{"project":null,"comments":{"nodes":[],"pageInfo":{"hasNextPage":false,"endCursor":null}}}})
            .to_string().as_bytes(),
    ).expect("nullable project");
    let (sent, fetch) = scripted(vec![
        Ok(page(json!([]), true, json!("next"))),
        Ok(null_project),
    ]);
    let error = run_with("Original Name", PROJECT, fetch, true, false, now)
        .await
        .expect_err("later null");
    assert_eq!(
        error.to_string(),
        "Failed to list comments: Project not found: Original Name"
    );
    assert_eq!(error.suggestion, None);
    assert_eq!(sent.borrow().len(), 2);
}

#[tokio::test]
async fn empty_cursor_is_sent_and_invalid_dates_have_stable_last_order() {
    let now = Utc
        .with_ymd_and_hms(2026, 9, 28, 0, 0, 0)
        .single()
        .expect("now");
    let (sent, fetch) = scripted(vec![
        Ok(page(json!([]), true, json!(""))),
        Ok(page(json!([]), false, Value::Null)),
    ]);
    let output = run_with(PROJECT, PROJECT, fetch, false, false, now)
        .await
        .expect("empty cursor");
    assert_eq!(output, b"No comments found for this project\n");
    assert_eq!(sent.borrow()[1]["after"], "");

    let mut nodes = frozen_page("c027-root-sort-ties").comments.nodes;
    nodes[0].created_at.0 = "not-a-date".to_owned();
    let text = render_text(&nodes, now, false);
    assert!(text.contains("commented not-a-date"));
    let invalid = text.find("not-a-date").expect("invalid date label");
    let last_valid = text.rfind("commented 1/1/2020").expect("valid date label");
    assert!(invalid > last_valid, "invalid dates sort after valid roots");
}
