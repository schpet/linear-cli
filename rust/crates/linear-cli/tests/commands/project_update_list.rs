use std::cell::RefCell;
use std::future::ready;
use std::rc::Rc;
use std::time::{Duration, UNIX_EPOCH};

use linear_cli::commands::project_update_list::{
    RenderOptions, graphql_int, output_color, render_json, render_text, request, run_with,
};
use linear_cli::graphql::envelope::parse_response;
use linear_cli::graphql::operations::project_updates::ListProjectUpdates;
use serde_json::{Value, json};

const PROJECT: &str = "3b9a5c7e-1d2f-4a6b-8c9d-0e1f2a3b4c5d";

fn frozen(id: &str) -> Value {
    let path = format!(
        "{}/../../parity/runner/c035-frozen-cases/{id}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_slice(&std::fs::read(path).expect("frozen case")).expect("case JSON")
}

fn page(id: &str) -> ListProjectUpdates {
    let case = frozen(id);
    let response = &case["graphql"]["groups"][0]["steps"][0]["response"];
    let body = if response["kind"] == "transport" {
        response["body"]["utf8"]
            .as_str()
            .expect("raw body")
            .to_owned()
    } else {
        json!({"data":response["data"]}).to_string()
    };
    parse_response(body.as_bytes()).expect("typed page")
}

#[test]
fn one_page_request_keeps_exact_graphql_int() {
    for (first, name) in [
        (10, "default"),
        (0, "zero"),
        (-1, "negative"),
        (2, "hex"),
        (2147483647, "max"),
    ] {
        let request = request(PROJECT, first);
        assert!(
            request.query.contains("projectUpdates(first: $first)"),
            "{name}"
        );
        assert_eq!(
            serde_json::to_value(request.variables.expect("variables")).expect("JSON"),
            json!({"id":PROJECT,"first":first}),
            "{name}"
        );
    }
}

#[test]
fn graphql_int_converts_positive_u32_without_truncation() {
    use std::num::NonZeroU32;
    assert_eq!(graphql_int(NonZeroU32::new(1).unwrap()).unwrap(), 1);
    assert_eq!(
        graphql_int(NonZeroU32::new(2_147_483_647).unwrap()).unwrap(),
        i32::MAX
    );
    for value in [2_147_483_648, u32::MAX] {
        let error = graphql_int(NonZeroU32::new(value).unwrap()).unwrap_err();
        assert_eq!(
            error.message(),
            "--limit must be at most 2147483647 for a GraphQL Int"
        );
        assert!(
            error
                .to_string()
                .starts_with(&format!("{}: ", "Failed to fetch project updates"))
        );
    }
}

#[test]
fn json_is_containing_project_and_preserves_first_page_info() {
    let case = frozen("c035-default-json");
    let project = page("c035-default-json").project.expect("project");
    assert_eq!(
        String::from_utf8(render_json(&project).expect("JSON")).expect("UTF-8"),
        case["expected"]["stdout"]["utf8"].as_str().expect("stdout")
    );
    assert!(project.project_updates.page_info.has_next_page);
}

#[test]
fn pipe_without_no_color_uses_plain_deno_bytes() {
    let case = frozen("c035-default-text");
    let project = page("c035-default-text").project.expect("project");
    assert!(!output_color(false, false));
    assert!(!output_color(true, true));
    assert!(output_color(true, false));
    assert_eq!(
        render_text(&project, 120, output_color(false, false), UNIX_EPOCH),
        case["expected"]["stdout"]["utf8"].as_str().expect("stdout")
    );
}

/// The author column is as wide as the widest name in terminal columns.
const AUTHOR_WIDE: &str = "Status updates for: Mobile App\n\nID       HEALTH  DATE     AUTHOR        \n00000000 onTrack just now 宽宽宽宽宽宽宽\n   Wide author shifts body width xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx...\n00000000 onTrack just now e\u{301}             \n   Combining mark\n";

#[test]
fn text_handles_health_author_body_and_unicode_width() {
    let now = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
    for id in ["c035-health-all", "c035-author-wide", "c035-body-unicode"] {
        let case = frozen(id);
        let project = page(id).project.expect("project");
        let expected = match id {
            "c035-author-wide" => AUTHOR_WIDE,
            _ => case["expected"]["stdout"]["utf8"].as_str().expect("stdout"),
        };
        assert_eq!(render_text(&project, 120, false, now), expected, "{id}");
    }
}

#[tokio::test]
async fn has_next_page_does_not_send_a_second_request() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&calls);
    let response = page("c035-default-json");
    run_with(
        PROJECT,
        PROJECT,
        10,
        move |request| {
            recorded
                .borrow_mut()
                .push(request.variables.expect("variables").first);
            ready(Ok(response))
        },
        RenderOptions {
            json: true,
            columns: 120,
            color: false,
            now: UNIX_EPOCH,
        },
    )
    .await
    .expect("one page");
    assert_eq!(*calls.borrow(), vec![Some(10)]);
}

#[tokio::test]
async fn null_project_reports_original_reference() {
    let response = page("c035-null-project");
    let error = run_with(
        "Original Project",
        PROJECT,
        10,
        |_| ready(Ok(response)),
        RenderOptions {
            json: false,
            columns: 120,
            color: false,
            now: UNIX_EPOCH,
        },
    )
    .await
    .expect_err("null project");
    assert!(error.to_string().contains("Original Project"));
}

#[test]
fn missing_required_node_field_is_rejected_before_output() {
    let case = frozen("c035-missing-required-raw");
    let step = &case["graphql"]["groups"][0]["steps"][0];
    let body = step["response"]["body"]["utf8"].as_str().expect("body");
    let result = parse_response::<ListProjectUpdates>(body.as_bytes());
    assert!(result.is_err());
}
