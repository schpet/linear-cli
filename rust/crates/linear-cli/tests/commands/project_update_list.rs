use std::cell::RefCell;
use std::future::ready;
use std::rc::Rc;
use std::time::{Duration, UNIX_EPOCH};

use linear_cli::commands::project_update_list::{
    RenderOptions, graphql_int, output_color, render_json, render_text, request, run_with,
};
use linear_cli::graphql::envelope::parse_response;
use linear_cli::graphql::operations::project_updates::ListProjectUpdates;
use linear_cli::graphql::operations::projects::ProjectUpdateHealthType;
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
            error.message,
            "--limit must be at most 2147483647 for a GraphQL Int"
        );
        assert_eq!(
            error.context.as_deref(),
            Some("Failed to fetch project updates")
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

#[test]
fn text_handles_health_author_body_and_unicode_width() {
    let now = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
    for id in [
        "c035-health-all",
        "c035-author-wide",
        "c035-body-unicode",
        "c035-body-percent-s",
        "c035-body-percent-double",
        "c035-body-percent-trailing",
        "c035-author-percent-health",
    ] {
        let case = frozen(id);
        let project = page(id).project.expect("project");
        assert_eq!(
            render_text(&project, 120, false, now),
            case["expected"]["stdout"]["utf8"].as_str().expect("stdout"),
            "{id}"
        );
    }
}

#[test]
fn colored_rows_and_console_placeholders_match_pinned_deno_pty_bytes() {
    // Direct localhost PTY probes used the SHA-pinned Deno reference binary
    // a17675c5ab9a0bf5f32f65e5e68112676576972a9979f5a97bc844f6b23e0835.
    let mut project = page("c035-default-text").project.expect("project");
    let prefix = "Status updates for: Mobile App\n\n\x1b[4mID      \x1b[24m \x1b[4mHEALTH \x1b[24m \x1b[4mDATE    \x1b[24m \x1b[4mAUTHOR \x1b[0m\n00000000 \x1b[32monTrack\x1b[39m just now Alice A\x1b[0m\n";
    for (body, expected) in [
        (
            "Update body",
            "\x1b[38;2;128;128;128m   Update body\x1b[39m\x1b[0m\n",
        ),
        ("a %d b", "\x1b[38;2;128;128;128m   a NaN b%c\x1b[0m\n"),
        ("a %i b", "\x1b[38;2;128;128;128m   a NaN b%c\x1b[0m\n"),
        ("a %f b", "\x1b[38;2;128;128;128m   a NaN b%c\x1b[0m\n"),
        (
            "a %o b",
            "\x1b[38;2;128;128;128m   a \x1b[32m\"\"\x1b[39m b%c\x1b[0m\n",
        ),
        (
            "a %O b",
            "\x1b[38;2;128;128;128m   a \x1b[32m\"\"\x1b[39m b%c\x1b[0m\n",
        ),
        ("a %c b", "\x1b[38;2;128;128;128m   a \x1b[39m b%c\x1b[0m\n"),
        ("a %", "\x1b[38;2;128;128;128m   a %c\x1b[0m \n"),
        ("a %% b", "\x1b[38;2;128;128;128m   a % b\x1b[39m\x1b[0m\n"),
        ("a %s b", "\x1b[38;2;128;128;128m   a  b%c\x1b[0m\n"),
        (
            "\u{00a0}body\u{00a0}",
            "\x1b[38;2;128;128;128m   body\x1b[39m\x1b[0m\n",
        ),
    ] {
        project.project_updates.nodes[0].body = body.to_owned();
        assert_eq!(
            render_text(&project, 120, true, UNIX_EPOCH),
            format!("{prefix}{expected}"),
            "{body}"
        );
    }
    project.project_updates.nodes[0].body = "a %f b".to_owned();
    assert!(render_text(&project, 120, false, UNIX_EPOCH).ends_with("   a NaN b%c\n"));
    project.project_updates.nodes[0].health = Some(ProjectUpdateHealthType::Unknown(String::new()));
    project.project_updates.nodes[0].body = "Update body".to_owned();
    assert!(
        render_text(&project, 120, false, UNIX_EPOCH)
            .contains("00000000 -      just now Alice A\n")
    );
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
    assert!(error.display_message().contains("Original Project"));
}

#[test]
fn missing_required_node_field_is_rejected_before_output() {
    let case = frozen("c035-missing-required-raw");
    let step = &case["graphql"]["groups"][0]["steps"][0];
    let body = step["response"]["body"]["utf8"].as_str().expect("body");
    let result = parse_response::<ListProjectUpdates>(body.as_bytes());
    assert!(result.is_err());
}
