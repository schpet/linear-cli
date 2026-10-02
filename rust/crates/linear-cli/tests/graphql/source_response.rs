//! Response classification shared by typed operations and bulk error reporting.
use linear_cli::graphql::{
    bulk_error::{self, BulkExchangeFailure, SourceExceptionKind},
    envelope::{GraphQlRequest, ResponseError},
    transport::{RawHttpResponse, TransportFailure, classify_typed},
};
use reqwest::{
    StatusCode,
    header::{CONTENT_TYPE, HeaderMap, HeaderValue},
};
use serde_json::{Value, json};
fn response(status: u16, mime: Option<&str>, body: &[u8]) -> RawHttpResponse {
    let mut headers = HeaderMap::new();
    if let Some(mime) = mime {
        headers.insert(CONTENT_TYPE, HeaderValue::from_str(mime).unwrap());
    }
    RawHttpResponse {
        status: StatusCode::from_u16(status).unwrap(),
        headers,
        body: body.to_vec(),
    }
}
fn request() -> GraphQlRequest<()> {
    GraphQlRequest {
        query: "query Sample { viewer { id } }\n".into(),
        variables: None,
        operation_name: Some("Sample".into()),
    }
}
#[test]
fn source_json_substring_domain_and_fetch_text_are_shared_by_all_three_consumers() {
    for mime in [
        "application/json",
        "ApPlIcAtIoN/JsOn; charset=UTF-8",
        "application/graphql-response+json; charset=utf-8",
        "prefix application/json suffix",
        "application/json-patch+json",
    ] {
        let raw = || {
            response(
                200,
                Some(mime),
                b"\xef\xbb\xbf{\"data\":{\"viewer\":{\"id\":\"dummy\"}}}",
            )
        };
        assert!(
            bulk_error::source_error(&raw(), &request())
                .map_err(BulkExchangeFailure::into_error)
                .unwrap()
                .is_none()
        );
        assert!(
            bulk_error::observe_source_error(&raw(), &request())
                .map_err(BulkExchangeFailure::into_error)
                .unwrap()
                .is_none()
        );
    }
    // Repeated headers are joined by Fetch; the second JSON value admits parsing.
    let mut raw = response(200, Some("text/plain"), b"{\"data\":{\"value\":1}}");
    raw.headers
        .append(CONTENT_TYPE, HeaderValue::from_static("Application/JSON"));
    assert_eq!(classify_typed::<Value>(raw).unwrap(), json!({"value":1}));
    // HeaderValue::to_str refuses non-ASCII suffix bytes that do not affect source admission.
    let mut raw = response(200, None, b"{\"data\":{\"value\":1}}");
    raw.headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_bytes(b"Application/JSON; note=\xff").unwrap(),
    );
    assert_eq!(classify_typed::<Value>(raw).unwrap(), json!({"value":1}));
}
#[test]
fn non_json_mime_never_extracts_errors_from_json_looking_text() {
    let text = r#"{"errors":[{"message":"looks like GraphQL"}],"data":{"ignored":true}}"#;
    for mime in [None, Some("text/plain"), Some("application/problem+json")] {
        let raw = || response(200, mime, text.as_bytes());
        let want =
            format!("Invalid execution result: result is not object or array. \nGot:\n{text}");
        assert!(matches!(
            classify_typed::<Value>(raw()),
            Err(TransportFailure::GraphQl { .. })
        ));
        assert_eq!(
            bulk_error::source_error(&raw(), &request())
                .map_err(BulkExchangeFailure::into_error)
                .unwrap(),
            Some(want.clone())
        );
        let error = bulk_error::observe_source_error(&raw(), &request())
            .map_err(BulkExchangeFailure::into_error)
            .unwrap()
            .unwrap();
        assert_eq!(error.kind, SourceExceptionKind::Plain);
        assert_eq!(error.message, want);
        assert!(error.preferred_message.is_none());
        let raw = || response(500, mime, text.as_bytes());
        assert!(matches!(
            classify_typed::<Value>(raw()),
            Err(TransportFailure::GraphQl { .. })
        ));
        let error = bulk_error::observe_source_error(&raw(), &request())
            .map_err(BulkExchangeFailure::into_error)
            .unwrap()
            .unwrap();
        assert_eq!(error.kind, SourceExceptionKind::Client);
        assert!(error.message.starts_with("GraphQL Error (Code: 500): "));
        assert!(error.preferred_message.is_none());
        let metadata: Value = serde_json::from_str(
            error
                .message
                .strip_prefix("GraphQL Error (Code: 500): ")
                .unwrap(),
        )
        .unwrap();
        assert!(metadata["response"].get("errors").is_none());
        assert!(metadata["response"].get("data").is_none());
        assert_eq!(metadata["response"]["body"], text);
    }
}
#[test]
fn malformed_json_success_remains_strict_and_non_success_retains_client_body() {
    for status in [200, 500] {
        let raw = || response(status, Some("application/json"), b"{invalid");
        match status {
            200 => {
                assert!(matches!(
                    classify_typed::<Value>(raw()),
                    Err(TransportFailure::Response(ResponseError::MalformedJson(_)))
                ));
                assert!(matches!(
                    bulk_error::source_error(&raw(), &request()),
                    Err(BulkExchangeFailure::Strict(_))
                ));
                assert!(matches!(
                    bulk_error::observe_source_error(&raw(), &request()),
                    Err(BulkExchangeFailure::Strict(_))
                ));
            }
            500 => {
                assert!(matches!(
                    classify_typed::<Value>(raw()),
                    Err(TransportFailure::Http { .. })
                ));
                let error = bulk_error::observe_source_error(&raw(), &request())
                    .map_err(BulkExchangeFailure::into_error)
                    .unwrap()
                    .unwrap();
                assert_eq!(error.kind, SourceExceptionKind::Client);
                assert!(error.message.contains("{invalid"));
                assert!(error.preferred_message.is_none());
            }
            _ => unreachable!(),
        }
    }
}
#[test]
fn graphql_client_metadata_keeps_raw_prefix_body_order_numbers_and_handled_preference() {
    let text = "\u{feff}{\"errors\":[{\"message\":\"first raw\\r\\nline\",\"extensions\":{\"userPresentableMessage\":\"Friendly\",\"metadata\":9007199254740993}}],\"data\":{\"partial\":true},\"extensions\":{\"server\":\"kept\"}}";
    for status in [200, 401] {
        let raw = || {
            response(
                status,
                Some("application/graphql-response+json"),
                text.as_bytes(),
            )
        };
        let without_bom = response(
            status,
            Some("application/graphql-response+json"),
            text.trim_start_matches('\u{feff}').as_bytes(),
        );
        let ordinary = classify_typed::<Value>(without_bom).unwrap_err();
        assert!(matches!(
            &ordinary,
            TransportFailure::GraphQl {
                partial_data: true,
                ..
            }
        ));
        assert_eq!(ordinary.to_string(), "Friendly");
        let original = bulk_error::source_error(&raw(), &request())
            .map_err(BulkExchangeFailure::into_error)
            .unwrap()
            .unwrap();
        assert!(original.starts_with("first raw\r\nline: "));
        let observed = bulk_error::observe_source_error(&raw(), &request())
            .map_err(BulkExchangeFailure::into_error)
            .unwrap()
            .unwrap();
        assert_eq!(observed.kind, SourceExceptionKind::Client);
        assert_eq!(observed.message, original);
        assert_eq!(observed.preferred_message.as_deref(), Some("Friendly"));
        let metadata: Value = serde_json::from_str(original.split_once(": ").unwrap().1).unwrap();
        assert_eq!(
            metadata["response"]["body"],
            text.strip_prefix('\u{feff}').unwrap()
        );
        assert_eq!(metadata["response"]["data"], json!({"partial":true}));
        assert_eq!(metadata["response"]["extensions"], json!({"server":"kept"}));
        assert_eq!(
            metadata["request"]["query"],
            "query Sample { viewer { id } }"
        );
        assert!(metadata["request"].get("variables").is_none());
        assert!(original.contains("9007199254740993"));
    }
}
