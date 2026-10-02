//! Response MIME failures preserve each source catch and already-sent effect stage.
use linear_cli::commands::{initiative_bulk as bulk, initiative_view::Reference, issue_update};
use serde_json::{Value, json};
const ID: &str = "00000000-0000-4000-8000-000000000001";
const OTHER: &str = "00000000-0000-4000-8000-000000000002";
fn operations(sent: &[Value]) -> Vec<&str> {
    sent.iter()
        .map(|request| request["operationName"].as_str().unwrap())
        .collect()
}
fn target(id: &str) -> bulk::Target {
    bulk::Target {
        original: id.into(),
        reference: Ok(Reference::Id(id.into())),
    }
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
#[tokio::test]
async fn bulk_mutation_text_failure_retains_sent_unknown_effect_and_continues_other_item() {
    let (transport, server) = super::project_write_server::serve_responses(4, move |request| {
        match request["operationName"].as_str().unwrap() {
            "GetInitiativeNameForBulkArchive" => (Some("application/json"),json!({"data":{"initiative":{"id":request["variables"]["id"],"name":"Name","archivedAt":null}}}).to_string()),
            "BulkArchiveInitiative" => (
                Some(if request["variables"]["id"]==ID {"text/plain"} else {"application/json"}),
                json!({"data":{"initiativeArchive":{"success":true}}}).to_string(),
            ),
            name => panic!("unexpected operation {name}"),
        }
    });
    let rows = bulk::execute(
        &transport,
        vec![target(ID), target(OTHER)],
        bulk::Mode::Archive,
        |_| Ok(()),
    )
    .await
    .unwrap();
    assert_eq!(
        rows.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
        [ID, OTHER]
    );
    assert_eq!(rows[0].name, None);
    assert!(!rows[0].succeeded());
    assert!(rows[1].succeeded());
    let bulk::BulkOutcome::Failed(message) = &rows[0].outcome else {
        panic!("failed row required")
    };
    assert_eq!(
        message,
        &format!(
            "Invalid execution result: result is not object or array. \nGot:\n{}",
            json!({"data":{"initiativeArchive":{"success":true}}})
        )
    );
    let sent = server.join().unwrap();
    for id in [ID, OTHER] {
        assert_eq!(
            sent.iter()
                .filter(
                    |request| request["operationName"] == "BulkArchiveInitiative"
                        && request["variables"]["id"] == id
                )
                .count(),
            1,
            "one sent mutation, no retry; remote effect unknown for refused MIME"
        );
    }
}
