use linear_cli::{
    commands::{
        document_delete as doc,
        initiative_bulk::{BulkOutcome, BulkResult},
        team_delete as team,
    },
    error::{AppError, AppErrorKind},
    graphql::{
        bulk_error::{self, BulkExchangeFailure},
        operations::team_delete::{
            GetTeamIssuesForMove, MoveIssue, MoveIssueToTeam, MoveIssues, MovePayload, MoveTeam,
        },
        operations::teams::PageInfo,
        transport::RawHttpResponse,
    },
};
use serde_json::{Value, json};
use std::{collections::VecDeque, future::ready};
fn response(status: u16, mime: &str, body: &str) -> RawHttpResponse {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::CONTENT_TYPE, mime.parse().unwrap());
    RawHttpResponse {
        status: reqwest::StatusCode::from_u16(status).unwrap(),
        headers,
        body: body.as_bytes().to_vec(),
    }
}
fn message(status: u16, mime: &str, body: &str) -> Result<Option<String>, BulkExchangeFailure> {
    bulk_error::source_error(
        &response(status, mime, body),
        &doc::bulk_delete_request("error-uuid"),
    )
}
#[test]
fn compiled_bulk_request_and_full_source_error_are_byte_exact() {
    let case: Value = serde_json::from_str(include_str!(
        "../../../../parity/runner/c013-c056-frozen-cases/c056-bulk-mixed-results.json"
    ))
    .unwrap();
    let body = case["graphql"]["groups"][0]["lanes"][3]["steps"][1]["response"]["body"]["utf8"]
        .as_str()
        .unwrap();
    let output = case["expected"]["stdout"]["utf8"].as_str().unwrap();
    let expected = output
        .split_once("  - error-item: ")
        .unwrap()
        .1
        .strip_suffix('\n')
        .unwrap();
    let actual = message(200, "application/json", body)
        .ok()
        .unwrap()
        .unwrap();
    assert_eq!(actual, expected);
    let request = doc::bulk_delete_request("error-uuid");
    let metadata: Value = serde_json::from_str(
        actual
            .split_once(": {\"response\"")
            .map(|(_, tail)| format!("{{\"response\"{tail}"))
            .as_ref()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        metadata["request"]["query"],
        request.query.strip_suffix('\n').unwrap_or(&request.query)
    );
    assert_eq!(metadata["request"]["variables"], json!({"id":"error-uuid"}));
}
#[test]
fn source_error_content_type_status_and_empty_error_arrays() {
    let body = r#"{"errors":[{"message":"should not project"}]}"#;
    let text = message(503, "text/plain", body).ok().unwrap().unwrap();
    assert!(text.starts_with("GraphQL Error (Code: 503): "));
    let fields: Value =
        serde_json::from_str(&format!("{{{}", text.split_once(": {").unwrap().1)).unwrap();
    assert_eq!(fields["response"].as_object().unwrap().len(), 3);
    assert_eq!(fields["response"]["body"], body);
    assert!(
        message(
            200,
            "application/JSON; charset=utf-8",
            r#"{"data":{},"errors":[]}"#
        )
        .ok()
        .unwrap()
        .is_none()
    );
    let text = message(
        400,
        "application/graphql-response+json",
        r#"{"data":null,"errors":[]}"#,
    )
    .ok()
    .unwrap()
    .unwrap();
    assert!(text.starts_with("GraphQL Error (Code: 400): "));
    assert!(text.contains("\"data\":null,\"errors\":[],\"status\":400"));
    let text = message(200, "text/plain", body).ok().unwrap().unwrap();
    assert_eq!(
        text,
        format!("Invalid execution result: result is not object or array. \nGot:\n{body}")
    );
    let text = message(500, "application/json", "not json")
        .ok()
        .unwrap()
        .unwrap();
    assert!(text.starts_with("GraphQL Error (Code: 500): "));
}
#[test]
fn raw_bulk_error_body_matches_source_fetch_utf8_replacement_and_bom_removal() {
    let request = doc::bulk_delete_request("error-uuid");
    let json = br#"{"errors":[{"message":"byte"}]}"#;
    let mut bom = vec![0xef, 0xbb, 0xbf];
    bom.extend_from_slice(json);
    let mut invalid = br#"{"errors":[{"message":""#.to_vec();
    invalid.push(0xff);
    invalid.extend_from_slice(br#""}]}"#);
    for (body, decoded) in [
        (bom, String::from_utf8(json.to_vec()).unwrap()),
        (
            invalid,
            "{\"errors\":[{\"message\":\"\u{fffd}\"}]}".to_owned(),
        ),
    ] {
        let mut raw = response(200, "application/json", "");
        raw.body = body;
        let actual = bulk_error::source_error(&raw, &request)
            .ok()
            .unwrap()
            .unwrap();
        let metadata: Value = serde_json::from_str(actual.split_once(": ").unwrap().1).unwrap();
        assert_eq!(metadata["response"]["body"], decoded);
        assert_eq!(
            metadata["response"]["errors"],
            serde_json::from_str::<Value>(&decoded).unwrap()["errors"]
        );
    }
}
#[test]
fn observed_error_metadata_keeps_numbers_keys_and_raw_body() {
    let body = r#"{"errors":[{"message":"bad","extensions":{"large":9007199254740993,"zero":-0,"10":"ten","2":"two","after":true}}],"extensions":{"x":1.0}}"#;
    let text = message(200, "APPLICATION/JSON", body)
        .ok()
        .unwrap()
        .unwrap();
    assert!(text.contains("\"large\":9007199254740993,"));
    assert!(text.find("\"2\":\"two\"").unwrap() < text.find("\"10\":\"ten\"").unwrap());
    assert!(text.contains("\"extensions\":{\"x\":1.0}"));
    let metadata: Value = serde_json::from_str(text.split_once(": ").unwrap().1).unwrap();
    assert_eq!(metadata["response"]["body"], body);
    assert_eq!(metadata["response"]["headers"], json!({}));
    assert!(metadata["request"].get("operationName").is_none());
}
#[test]
fn nonstandard_bulk_shapes_are_strict_and_partial_data_is_not_lost() {
    for (body, name) in [
        (r#"{"errors":{}}"#, "native-bulk-error-array-shape"),
        ("[]", "native-bulk-single-response-envelope"),
        ("oops", "native-bulk-response-json-syntax"),
        (r#"{"data":3}"#, "native-bulk-execution-field-shape"),
        (
            r#"{"errors":[{"message":3}]}"#,
            "native-bulk-execution-field-shape",
        ),
    ] {
        match message(200, "application/json", body) {
            Err(BulkExchangeFailure::Strict(error)) => assert!(error.message.starts_with(name)),
            _ => panic!("invalid shape was accepted"),
        }
    }
    let body = r#"{"data":{"unused":true},"errors":[{"message":"first\r\nsecond","extensions":{"userPresentableMessage":"friendly"}}]}"#;
    let text = message(200, "application/json", body)
        .ok()
        .unwrap()
        .unwrap();
    assert!(text.starts_with("first\r\nsecond: "));
    assert!(text.contains("\"data\":{\"unused\":true},\"errors\""));
    assert!(!text.starts_with("friendly"));
}
fn page(ids: &[&str], next: bool, cursor: Option<&str>) -> GetTeamIssuesForMove {
    GetTeamIssuesForMove {
        team: Some(MoveTeam {
            issues: MoveIssues {
                nodes: ids
                    .iter()
                    .map(|id| MoveIssue {
                        id: cynic::Id::new(*id),
                        identifier: format!("SRC-{id}"),
                    })
                    .collect(),
                page_info: PageInfo {
                    has_next_page: next,
                    end_cursor: cursor.map(str::to_owned),
                },
            },
        }),
    }
}
#[tokio::test]
async fn issue_pages_preserve_duplicates_and_omit_initial_cursor() {
    let mut pages = VecDeque::from([
        page(&["one", "one"], true, Some("more")),
        page(&["two"], false, None),
    ]);
    let mut variables = Vec::new();
    let issues = team::all_issues("source", |request| {
        variables.push(serde_json::to_value(request.variables).unwrap());
        ready(Ok(pages.pop_front().unwrap()))
    })
    .await
    .unwrap();
    assert_eq!(
        issues.iter().map(|i| i.id.inner()).collect::<Vec<_>>(),
        ["one", "one", "two"]
    );
    assert_eq!(
        variables,
        [
            json!({"teamId":"source","first":100}),
            json!({"teamId":"source","first":100,"after":"more"})
        ]
    );
}
#[tokio::test]
async fn invalid_issue_cursors_fail_before_any_move() {
    let missing = team::all_issues("source", |_| ready(Ok(page(&["one"], true, None))))
        .await
        .unwrap_err();
    assert!(missing.message.contains("no pagination cursor"));
    let repeated = team::all_issues("source", |_| {
        ready(Ok(page(&["one"], true, Some("repeat"))))
    })
    .await
    .unwrap_err();
    assert!(repeated.message.contains("repeated"));
}
#[tokio::test]
async fn sequential_moves_ignore_false_and_stop_on_partial_failure() {
    let issues = page(&["one", "two", "three"], false, None)
        .team
        .unwrap()
        .issues
        .nodes;
    let mut writes = Vec::new();
    let mut progress = Vec::new();
    let moved = team::move_all(
        &issues,
        "target",
        |request| {
            writes.push(request.variables.unwrap().id);
            ready(Ok(MoveIssueToTeam {
                issue_update: MovePayload { success: false },
            }))
        },
        |done, total| {
            progress.push((done, total));
            Ok(())
        },
    )
    .await
    .unwrap();
    assert_eq!(moved, 3);
    assert_eq!(writes, ["one", "two", "three"]);
    assert_eq!(progress, [(1, 3), (2, 3), (3, 3)]);
    writes.clear();
    let result = team::move_all(
        &issues,
        "target",
        |request| {
            let id = request.variables.unwrap().id;
            writes.push(id.clone());
            ready(if id == "two" {
                Err(AppError::new(AppErrorKind::GraphQl, "second failed"))
            } else {
                Ok(MoveIssueToTeam {
                    issue_update: MovePayload { success: true },
                })
            })
        },
        |_, _| Ok(()),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(writes, ["one", "two"]);
}
#[test]
fn document_summary_keeps_source_names_multiline_and_delete_typo() {
    let rows = [BulkResult {
        id: "id".into(),
        name: None,
        outcome: BulkOutcome::Failed("raw\nerror".into()),
    }];
    let (text, failed) = doc::summary(&rows);
    assert!(failed);
    assert_eq!(
        String::from_utf8(text).unwrap(),
        "\n✗ Failed to delet all 1 document\n\nFailed operations:\n  - id: raw\nerror\n"
    );
    let rows = [
        BulkResult {
            id: "yes".into(),
            name: Some("title".into()),
            outcome: BulkOutcome::Succeeded,
        },
        BulkResult {
            id: "no".into(),
            name: Some("".into()),
            outcome: BulkOutcome::Failed("".into()),
        },
    ];
    assert_eq!(
        String::from_utf8(doc::summary(&rows).0).unwrap(),
        "\nCompleted: 1/2 documents deleted\n  ✓ Succeeded: 1\n  ✗ Failed: 1\n\nFailed operations:\n  - no: Unknown error\n"
    );
}

#[tokio::test]
async fn corrupt_bulk_lookup_is_not_an_ordinary_not_found_fallback() {
    let target = || doc::Target {
        original: "original".to_owned(),
        id: Ok("resolved".to_owned()),
    };
    let (transport, server) = super::delete_server::serve(
        r#"{"data":{"document":{"id":3,"slugId":"slug","title":"title"}}}"#,
    );
    let result = doc::run_item(&transport, target()).await;
    let request = server.join().unwrap();
    assert_eq!(request["variables"], json!({"id":"resolved"}));
    match result.outcome {
        BulkOutcome::Failed(message) => {
            assert!(message.contains("expected operation shape"));
            assert_ne!(message, "Document not found")
        }
        _ => panic!("corrupt ID accepted"),
    }
    let (transport, server) =
        super::delete_server::serve(r#"{"errors":[{"message":"ordinary lookup error"}]}"#);
    let result = doc::run_item(&transport, target()).await;
    server.join().unwrap();
    assert_eq!(
        result.outcome,
        BulkOutcome::Failed("Document not found".to_owned())
    );
    assert_eq!(result.id, "original");
    assert!(result.name.is_none());
}

#[tokio::test]
async fn non_json_bulk_details_never_send_a_delete() {
    let (transport, server) = super::delete_server::serve_with_content_type(
        r#"{"data":{"document":{"id":"resolved-id","slugId":"slug","title":"title"}}}"#,
        "text/plain; charset=utf-8",
    );
    let result = doc::run_item(
        &transport,
        doc::Target {
            original: "original".to_owned(),
            id: Ok("resolved".to_owned()),
        },
    )
    .await;
    let request = server.join().unwrap();
    assert_eq!(request["variables"], json!({"id":"resolved"}));
    assert_eq!(
        result.outcome,
        BulkOutcome::Failed("Document not found".to_owned())
    );
    assert_eq!(result.id, "original");
    assert!(result.name.is_none());
}
