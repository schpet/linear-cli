use linear_cli::{
    graphql::{
        bulk_error::{self, BulkExchangeFailure},
        envelope::GraphQlRequest,
        transport::RawHttpResponse,
    },
};
use serde_json::{Value, json};
fn response(status: u16, mime: &str, body: &str) -> RawHttpResponse {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::CONTENT_TYPE, mime.parse().unwrap());
    RawHttpResponse {
        status: reqwest::StatusCode::from_u16(status).unwrap(),
        headers,
        body: body.as_bytes().to_vec(),
    }
}
/// A document delete mutation, as the sample request in error metadata.
fn bulk_delete_request(id: &str) -> GraphQlRequest<Value> {
    GraphQlRequest {
        query: "mutation BulkDeleteDocument($id: String!) {\n  documentDelete(id: $id) {\n    success\n  }\n}\n".to_owned(),
        variables: Some(json!({ "id": id })),
        operation_name: Some("BulkDeleteDocument".to_owned()),
    }
}
fn message(status: u16, mime: &str, body: &str) -> Result<Option<String>, BulkExchangeFailure> {
    bulk_error::source_error(
        &response(status, mime, body),
        &bulk_delete_request("error-uuid"),
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
    let request = bulk_delete_request("error-uuid");
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
    let request = bulk_delete_request("error-uuid");
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
            Err(BulkExchangeFailure::Strict(error)) => assert!(error.message().starts_with(name)),
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
