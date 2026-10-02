use linear_cli::{
    commands::milestone_delete,
    graphql::{envelope::parse_response, operations::milestone_delete::DeleteProjectMilestone},
    refs::reject_linear_url,
};
use serde_json::{Value, json};

#[test]
fn request_matches_source_and_preserves_raw_id() {
    let frozen: Value = serde_json::from_slice(
        &std::fs::read(format!(
            "{}/../../parity/runner/c034-frozen-cases/c034-raw-id.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap(),
    )
    .unwrap();
    let source = &frozen["graphql"]["groups"][0]["steps"][0]["operation"];
    let request = serde_json::to_value(milestone_delete::request(
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
    assert_eq!(request["operationName"], "DeleteProjectMilestone");
    for payload in [
        json!(null),
        json!({}),
        json!({"success":null}),
        json!({"success":"true"}),
    ] {
        assert!(
            parse_response::<DeleteProjectMilestone>(
                json!({"data":{"projectMilestoneDelete":payload}})
                    .to_string()
                    .as_bytes()
            )
            .is_err()
        );
    }
}

#[test]
fn rejects_all_linear_urls_but_forwards_other_raw_values() {
    for input in [
        "https://linear.app/acme/unsupported",
        "https://linear.app/acme/project/%FF",
    ] {
        assert!(reject_linear_url(input, "a milestone UUID").is_err());
    }
    assert!(reject_linear_url("  not-a-uuid ", "a milestone UUID").is_ok());
    assert!(reject_linear_url("https://example.com/foo", "a milestone UUID").is_ok());
}

#[tokio::test]
async fn submit_preserves_raw_id_exact_success_and_false_failure_with_one_send() {
    for (body, success) in [
        (
            r#"{"data":{"projectMilestoneDelete":{"success":true}}}"#,
            true,
        ),
        (
            r#"{"data":{"projectMilestoneDelete":{"success":false}}}"#,
            false,
        ),
    ] {
        let (transport, server) = super::delete_server::serve(body);
        let result = milestone_delete::submit(&transport, " raw id ").await;
        if success {
            assert_eq!(result.unwrap(), "✓ Deleted milestone  raw id \n".as_bytes());
        } else {
            assert_eq!(result.unwrap_err().message(), milestone_delete::CONTEXT);
        }
        assert_eq!(
            server.join().unwrap()["variables"],
            json!({"id":" raw id "})
        );
    }
}
