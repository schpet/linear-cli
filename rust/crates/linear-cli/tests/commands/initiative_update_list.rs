use chrono::{DateTime, Utc};
use linear_cli::commands::initiative_update_list::{
    graphql_int, render_json, render_text, request,
};
use linear_cli::graphql::envelope::parse_response;
use linear_cli::graphql::operations::initiative_updates::ListInitiativeUpdates;
use serde_json::{Value, json};

const INITIATIVE: &str = "3b9a5c7e-1d2f-4a6b-8c9d-0e1f2a3b4c5d";

fn frozen(id: &str) -> Value {
    let path = format!(
        "{}/../../parity/runner/c048-frozen-cases/{id}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_slice(&std::fs::read(path).expect("frozen case")).expect("case JSON")
}

fn page(id: &str) -> ListInitiativeUpdates {
    let case = frozen(id);
    let data = &case["graphql"]["groups"][0]["steps"][0]["response"]["data"];
    parse_response(json!({"data":data}).to_string().as_bytes()).expect("typed page")
}

#[test]
fn request_preserves_one_page_graphql_int_and_selection() {
    for first in [10, 0, -1, 2, i32::MAX] {
        let query = request(INITIATIVE, first);
        assert!(query.query.contains("initiativeUpdates(first: $first)"));
        assert!(!query.query.contains("pageInfo"));
        assert_eq!(
            serde_json::to_value(query.variables.expect("variables")).expect("JSON"),
            json!({"id":INITIATIVE,"first":first})
        );
    }
    assert_eq!(
        graphql_int(std::num::NonZeroU32::new(2).unwrap()).expect("integer"),
        2
    );
    for bad in [2_147_483_648, u32::MAX] {
        let error = graphql_int(std::num::NonZeroU32::new(bad).unwrap()).unwrap_err();
        assert_eq!(
            error.message,
            "--limit must be at most 2147483647 for a GraphQL Int"
        );
        assert_eq!(
            error.context.as_deref(),
            Some("Failed to fetch initiative updates")
        );
    }
}

#[test]
fn json_keeps_source_field_order_and_connection_shape() {
    for id in ["c048-default-json", "c048-empty-json"] {
        let case = frozen(id);
        let initiative = page(id).initiative.expect("initiative");
        let actual =
            String::from_utf8(render_json(&initiative).expect("rendered JSON")).expect("UTF-8");
        assert_eq!(actual, case["expected"]["stdout"]["utf8"], "{id}");
    }
}

#[test]
fn text_uses_initiative_health_names_author_fallback_and_body_preview() {
    let now = DateTime::<Utc>::from(std::time::UNIX_EPOCH);
    for id in [
        "c048-default-text",
        "c048-empty-text",
        "c048-health-all",
        "c048-author-fallbacks",
        "c048-body-unicode",
        "c048-body-percent-s",
    ] {
        let case = frozen(id);
        let initiative = page(id).initiative.expect("initiative");
        assert_eq!(
            render_text(&initiative, 120, false, now),
            case["expected"]["stdout"]["utf8"],
            "{id}"
        );
    }
}

#[test]
fn author_percent_escapes_collapse_after_source_padding() {
    let case = frozen("c048-author-percent");
    let initiative = page("c048-author-percent").initiative.expect("initiative");
    let now = DateTime::<Utc>::from(std::time::UNIX_EPOCH);
    assert_eq!(
        render_text(&initiative, 120, false, now),
        case["expected"]["stdout"]["utf8"]
    );
    assert!(render_text(&initiative, 120, true, now).contains("A%B  \x1b[0m\n"));
}

#[test]
fn terminal_date_and_body_use_source_true_color_gray() {
    let now = DateTime::<Utc>::from(std::time::UNIX_EPOCH);
    let initiative = page("c048-default-text").initiative.expect("initiative");
    let output = render_text(&initiative, 120, true, now);
    assert!(output.contains("\x1b[38;2;39;174;96mOn Track\x1b[39m"));
    assert!(output.contains("\x1b[38;2;128;128;128m1 minute ago\x1b[39m"));
    assert!(output.contains("  \x1b[38;2;128;128;128mUpdate body\x1b[39m"));
}

#[test]
fn malformed_required_update_is_rejected_before_rendering() {
    let case = frozen("c048-missing-required-raw");
    let body = case["graphql"]["groups"][0]["steps"][0]["response"]["body"]["utf8"]
        .as_str()
        .expect("raw body");
    assert!(parse_response::<ListInitiativeUpdates>(body.as_bytes()).is_err());
}
