//! Response MIME failures preserve each source catch and already-sent effect stage.
use linear_cli::commands::issue::update as issue_update;
use serde_json::{Value, json};
fn operations(sent: &[Value]) -> Vec<&str> {
    sent.iter()
        .map(|request| request["operationName"].as_str().unwrap())
        .collect()
}
#[tokio::test]
async fn required_team_text_response_stops_before_later_lookup_or_issue_update() {
    let body = json!({"data":{"teams":{"nodes":[{"id":"team","key":"ENG","name":"Engineering"}]}}})
        .to_string();
    let (backend, server) =
        super::issue_write::network_with_content_types(vec![(Some("text/plain"), body.clone())]);
    let error = issue_update::input(
        &backend,
        "ENG-7",
        &issue_update::Fields {
            project: Some("never looked up".into()),
            ..Default::default()
        },
        None,
    )
    .await
    .unwrap_err();
    // This command keeps its existing handled-observer Error kind; the
    // ordinary transport and source exception classes are separately table-tested.
    assert_eq!(
        error.message(),
        format!("Invalid execution result: result is not object or array. \nGot:\n{body}")
    );
    let sent = server.join().unwrap();
    assert_eq!(operations(&sent), ["ResolveTeam"]);
    assert_eq!(sent[0]["variables"]["reference"], "ENG");
}
