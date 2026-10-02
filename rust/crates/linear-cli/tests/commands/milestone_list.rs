use std::cell::RefCell;
use std::collections::VecDeque;
use std::ffi::OsString;
use std::future::ready;
use std::rc::Rc;

use linear_cli::cli::{Cli, RootCommand};
use linear_cli::commands::milestone::list::{CONTEXT, render_text, request, run_with};
use linear_cli::error::Error;
use linear_cli::graphql::envelope::{GraphQlRequest, parse_response};
use linear_cli::graphql::operations::milestones::{
    GetProjectMilestones, GetProjectMilestonesVariables,
};
use serde_json::{Value, json};

const PROJECT: &str = "3b9a5c7e-1d2f-4a6b-8c9d-0e1f2a3b4c5d";

fn frozen(id: &str) -> Value {
    let file = format!(
        "{}/../../parity/runner/c030-frozen-cases/{id}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_slice(&std::fs::read(file).expect("frozen case")).expect("case JSON")
}

fn milestone_steps(case: &Value) -> Vec<Value> {
    case["graphql"]["groups"]
        .as_array()
        .expect("groups")
        .iter()
        .flat_map(|group| group["steps"].as_array().expect("steps").clone())
        .filter(|step| {
            step["operation"]["document"]
                .as_str()
                .is_some_and(|document| document.starts_with("query GetProjectMilestones"))
        })
        .collect()
}

/// The typed page a frozen step returns, from structured data or a raw body.
fn response(step: &Value) -> Result<GetProjectMilestones, Error> {
    let response = &step["response"];
    let body = match response["kind"].as_str() {
        Some("data") => json!({"data": response["data"]}).to_string(),
        Some("transport") => response["body"]["utf8"]
            .as_str()
            .expect("raw body")
            .to_owned(),
        other => panic!("unsupported frozen response kind {other:?}"),
    };
    parse_response(body.as_bytes())
        .map_err(|error| Error::new("typed page failed").with_source(error))
}

fn variables(request: &GraphQlRequest<GetProjectMilestonesVariables>) -> Value {
    serde_json::to_value(request.variables.as_ref().expect("variables")).expect("variables JSON")
}

type Sent = Rc<RefCell<Vec<Value>>>;

fn scripted(
    pages: Vec<Result<GetProjectMilestones, Error>>,
) -> (
    Sent,
    impl FnMut(
        GraphQlRequest<GetProjectMilestonesVariables>,
    ) -> std::future::Ready<Result<GetProjectMilestones, Error>>,
) {
    let sent: Sent = Rc::new(RefCell::new(Vec::new()));
    let queue = Rc::new(RefCell::new(VecDeque::from(pages)));
    let recorder = sent.clone();
    let fetch = move |request: GraphQlRequest<GetProjectMilestonesVariables>| {
        recorder.borrow_mut().push(variables(&request));
        ready(
            queue
                .borrow_mut()
                .pop_front()
                .expect("unexpected extra page request"),
        )
    };
    (sent, fetch)
}

fn page(nodes: Value, has_next_page: bool, end_cursor: Value) -> GetProjectMilestones {
    parse_response(
        json!({"data":{"project":{
            "id": PROJECT,
            "name": "Mobile App",
            "projectMilestones": {
                "nodes": nodes,
                "pageInfo": {"hasNextPage": has_next_page, "endCursor": end_cursor}
            }
        }}})
        .to_string()
        .as_bytes(),
    )
    .expect("typed milestone page")
}

fn milestone(id: &str, name: &str, date: Value) -> Value {
    json!({
        "id": id,
        "name": name,
        "targetDate": date,
        "sortOrder": 1,
        "project": {"id": PROJECT, "name": "Mobile App"}
    })
}

fn null_root() -> GetProjectMilestones {
    parse_response(br#"{"data":{"project":null}}"#).expect("null root")
}

#[test]
fn request_matches_the_source_document_and_omits_the_first_cursor() {
    let first = request(PROJECT, None);
    let expected =
        frozen("c030-two-pages-json")["graphql"]["groups"][0]["steps"][0]["operation"]["document"]
            .as_str()
            .expect("frozen document")
            .to_owned();
    let normalize = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
    assert_eq!(normalize(&first.query), normalize(&expected));
    assert_eq!(
        variables(&first),
        json!({"projectId": PROJECT, "first": 100})
    );
    assert_eq!(
        variables(&request(PROJECT, Some("cursor-1".to_owned()))),
        json!({"projectId": PROJECT, "first": 100, "after": "cursor-1"})
    );
}

/// Wide and emoji names are truncated by display width without splitting characters.
const LONG_WIDE_TEXT: &str = "NAME                                  ID                                   TARGET DATE  PROJECT                       \nA very long milestone name that ke... 00000000-0000-4000-8000-000000000101 2026-01-01   Mobile App                    \n日本語のマイルストーン名前は幅が広... 00000000-0000-4000-8000-000000000102 2026-01-02   Mobile App                    \nx🚀🚀🚀🚀🚀🚀🚀🚀🚀🚀🚀🚀🚀🚀🚀🚀...  00000000-0000-4000-8000-000000000103 2026-01-03   Mobile App                    \nShort                                 00000000-0000-4000-8000-000000000104 2026-01-04   An Extremely Long Project N...\nWide project                          00000000-0000-4000-8000-000000000105 2026-01-05   プロジェクト名前テスト        \n";

#[tokio::test]
async fn success_cases_render_text_and_json_with_page_variables() {
    for id in [
        "c030-one-page-text",
        "c030-two-pages-json",
        "c030-two-pages-text",
        "c030-three-pages-json",
        "c030-three-pages-text",
        "c030-mixed-sort-json",
        "c030-mixed-sort-text",
        "c030-long-wide-text",
        "c030-short-name-text",
        "c030-empty-json",
        "c030-empty-text",
        "c030-empty-cursor-final",
        "c030-null-empty-date-json",
        "c030-null-empty-date-text",
        "c030-sortorder-numbers-json",
    ] {
        let case = frozen(id);
        let steps = milestone_steps(&case);
        let (sent, fetch) = scripted(steps.iter().map(response).collect());
        let json = case["argv"]
            .as_array()
            .expect("argv")
            .iter()
            .any(|arg| arg == "--json" || arg == "-j");
        let output = run_with(PROJECT, PROJECT, fetch, json, 120, false)
            .await
            .unwrap_or_else(|error| panic!("{id}: {error}"));
        let expected = match id {
            "c030-long-wide-text" => LONG_WIDE_TEXT,
            _ => case["expected"]["stdout"]["utf8"].as_str().expect("stdout"),
        };
        assert_eq!(String::from_utf8(output).expect("UTF-8"), expected, "{id}");
        let expected: Vec<Value> = steps
            .iter()
            .map(|step| step["operation"]["variables"].clone())
            .collect();
        assert_eq!(*sent.borrow(), expected, "{id}");
    }
}

#[tokio::test]
async fn null_root_reports_the_raw_reference_and_discards_earlier_pages() {
    for reference in [
        "Mobile App",
        "3B9A5C7E-1D2F-4A6B-8C9D-0E1F2A3B4C5D",
        "https://linear.app/alpha/project/mobile-app-abc123def456",
    ] {
        let (sent, fetch) = scripted(vec![
            Ok(page(
                json!([milestone("m-1", "Kept?", json!("2026-01-01"))]),
                true,
                json!("cursor-1"),
            )),
            Ok(null_root()),
        ]);
        let error = run_with(reference, PROJECT, fetch, true, 120, false)
            .await
            .expect_err("later null root");
        assert_eq!(
            error.to_string(),
            format!("{CONTEXT}: Project not found: {reference}")
        );
        assert_eq!(error.hint(), None);
        assert_eq!(sent.borrow().len(), 2);
    }
}

#[tokio::test]
async fn missing_and_empty_cursors_use_the_source_message_without_a_page_suffix() {
    for cursor in [Value::Null, json!("")] {
        let (sent, fetch) = scripted(vec![Ok(page(json!([]), true, cursor))]);
        let error = run_with(PROJECT, PROJECT, fetch, false, 120, false)
            .await
            .expect_err("missing cursor");
        assert_eq!(
            error.to_string(),
            "Failed to fetch milestones: Linear reported more milestones but returned no pagination cursor"
        );
        assert_eq!(error.hint(), Some("Retry the command."));
        assert_eq!(sent.borrow().len(), 1);
    }
}

#[tokio::test]
async fn repeated_cursors_stop_instead_of_looping() {
    // The finite frozen shape: page two repeats page one's cursor.
    let (sent, fetch) = scripted(vec![
        Ok(page(json!([]), true, json!("same"))),
        Ok(page(json!([]), true, json!("same"))),
    ]);
    let error = run_with(PROJECT, PROJECT, fetch, true, 120, false)
        .await
        .expect_err("repeated cursor");
    assert_eq!(
        error.to_string(),
        "Failed to fetch milestones: Linear repeated a milestone pagination cursor on page 2"
    );
    assert_eq!(sent.borrow().len(), 2);

    // A truly cyclic A -> B -> A walk, which the source would follow forever.
    let (sent, fetch) = scripted(vec![
        Ok(page(json!([]), true, json!("a"))),
        Ok(page(json!([]), true, json!("b"))),
        Ok(page(json!([]), true, json!("a"))),
    ]);
    let error = run_with(PROJECT, PROJECT, fetch, true, 120, false)
        .await
        .expect_err("cyclic cursor");
    assert_eq!(
        error.to_string(),
        "Failed to fetch milestones: Linear repeated a milestone pagination cursor on page 3"
    );
    assert_eq!(error.hint(), Some("Retry the command."));
    assert_eq!(
        *sent.borrow(),
        vec![
            json!({"projectId": PROJECT, "first": 100}),
            json!({"projectId": PROJECT, "first": 100, "after": "a"}),
            json!({"projectId": PROJECT, "first": 100, "after": "b"}),
        ]
    );
}

#[tokio::test]
async fn page_errors_keep_their_message_and_gain_the_context_once() {
    let (_, fetch) = scripted(vec![
        Ok(page(json!([]), true, json!("cursor-1"))),
        Err(Error::new("synthetic page failure")),
    ]);
    let error = run_with(PROJECT, PROJECT, fetch, true, 120, false)
        .await
        .expect_err("page failure");
    assert_eq!(
        error.to_string(),
        "Failed to fetch milestones: synthetic page failure"
    );
}

#[test]
fn narrow_tables_keep_a_minimum_name_width_and_header_style() {
    let data = page(
        json!([
            milestone("m-1", "Launch readiness", json!("2026-01-01")),
            milestone("m-2", "Beta", Value::Null),
        ]),
        false,
        Value::Null,
    );
    let nodes = data.project.expect("project").project_milestones.nodes;

    // 40 columns leave no room, so names keep the minimum column width.
    let narrow = render_text(&nodes, 40, false);
    let tail = |id: &str, date: &str| format!("{id:<36} {date:<12} Mobile App");
    assert_eq!(
        narrow,
        format!(
            "NAME             {:<36} {:<12} PROJECT   \nLaunch readiness {}\nBeta             {}\n",
            "ID",
            "TARGET DATE",
            tail("m-1", "2026-01-01"),
            tail("m-2", "No date"),
        )
    );
    assert_eq!(render_text(&nodes, 0, false), narrow);

    let colored = render_text(&nodes, 120, true);
    assert_eq!(
        colored,
        format!(
            "\x1b[4m{:<16}\x1b[24m \x1b[4m{:<36}\x1b[24m \x1b[4m{:<12}\x1b[24m \x1b[4mPROJECT   \x1b[0m\n{:<16} {}\n{:<16} {}\n",
            "NAME",
            "ID",
            "TARGET DATE",
            "Launch readiness",
            tail("m-1", "2026-01-01"),
            "Beta",
            tail("m-2", "No date"),
        )
    );
}

fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
    let args: Vec<OsString> = args.iter().map(OsString::from).collect();
    crate::parse(&args)
}

#[test]
fn route_dispatches_and_requires_an_attached_hyphen_leading_project() {
    let cli = parse(&["milestone", "list", "--project=--json", "-j"]).expect("parse");
    let RootCommand::Milestone(group) = cli.command else {
        panic!("group")
    };
    let linear_cli::cli::milestone::MilestoneCommand::List(action) = group.command else {
        panic!("action")
    };
    assert_eq!(action.project, "--json");
    assert!(action.json);

    for args in [
        ["milestone", "list", "--project", "--json"],
        ["milestone", "list", "--project", "-j"],
        ["milestone", "list", "--project=", "--json"],
    ] {
        let error = parse(&args).expect_err("v3 rejects a pending hyphen value");
        let expected = linear_cli::cli::command()
            .try_get_matches_from(std::iter::once("linear").chain(args.iter().copied()))
            .expect_err("native missing value");
        assert_eq!(error.to_string(), expected.to_string(), "{args:?}");
        assert_eq!(error.kind(), expected.kind());
    }
}
