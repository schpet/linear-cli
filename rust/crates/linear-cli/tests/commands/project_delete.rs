use linear_cli::{
    commands::project::delete as project_delete,
    graphql::{envelope::parse_response, operations::project_delete::DeleteProject},
};
use serde_json::{Value, json};

#[test]
fn request_matches_frozen_source_document_and_variables() {
    let frozen: Value = serde_json::from_slice(
        &std::fs::read(format!(
            "{}/../../parity/runner/c029-frozen-cases/c029-uuid-success.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap(),
    )
    .unwrap();
    let source = &frozen["graphql"]["groups"][0]["steps"][0]["operation"];
    let request = serde_json::to_value(project_delete::request(
        source["variables"]["id"].as_str().unwrap(),
    ))
    .unwrap();
    let compact = |s: &str| {
        s.chars()
            .filter(|c| !c.is_whitespace() && *c != ',')
            .collect::<String>()
    };
    assert_eq!(
        compact(request["query"].as_str().unwrap()),
        compact(source["document"].as_str().unwrap())
    );
    assert_eq!(request["variables"], source["variables"]);
    assert_eq!(request["operationName"], "DeleteProject");
}

#[test]
fn schema_allows_null_entity_but_requires_boolean_and_entity_id_name() {
    for entity in [json!(null), json!({"id":"id","name":""})] {
        assert!(
            parse_response::<DeleteProject>(
                json!({"data":{"projectDelete":{"success":true,"entity":entity}}})
                    .to_string()
                    .as_bytes()
            )
            .is_ok()
        );
    }
    for payload in [
        json!(null),
        json!({}),
        json!({"success":"true","entity":null}),
        json!({"success":true,"entity":{"id":"id","name":null}}),
        json!({"success":true,"entity":{"id":null,"name":"name"}}),
    ] {
        assert!(
            parse_response::<DeleteProject>(
                json!({"data":{"projectDelete":payload}})
                    .to_string()
                    .as_bytes()
            )
            .is_err()
        );
    }
}

#[tokio::test]
async fn output_preserves_original_input_and_empty_name_and_never_retries() {
    for (body, expected) in [
        (
            r#"{"data":{"projectDelete":{"success":true,"entity":null}}}"#,
            "✓ Deleted project: Original name\n",
        ),
        (
            r#"{"data":{"projectDelete":{"success":true,"entity":{"id":"id","name":""}}}}"#,
            "✓ Deleted project: \n",
        ),
    ] {
        let (transport, server) = super::delete_server::serve(body);
        assert_eq!(
            project_delete::submit(&transport, "Original name", "resolved-id")
                .await
                .unwrap(),
            expected.as_bytes()
        );
        assert_eq!(
            server.join().unwrap()["variables"],
            json!({"id":"resolved-id"})
        );
    }
    let (transport, server) = super::delete_server::serve(
        r#"{"data":{"projectDelete":{"success":false,"entity":null}}}"#,
    );
    assert_eq!(
        project_delete::submit(&transport, "Original", "id")
            .await
            .unwrap_err()
            .message(),
        project_delete::CONTEXT
    );
    server.join().unwrap();
}
