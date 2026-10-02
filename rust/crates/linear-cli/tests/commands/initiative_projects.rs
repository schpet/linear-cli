use cynic::MutationBuilder;
use linear_cli::commands::initiative::projects::{self as command, Entity, Mode};
use linear_cli::graphql::envelope::{GraphQlRequest, parse_response};
use linear_cli::graphql::operations::initiative_projects::*;
use linear_cli::graphql::transport::{
    ApiKey, Deadline, EndpointUrl, GraphQlTransport, ResponseCap, TransportConfig,
};
use linear_cli::refs::WorkspaceScope;
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::Duration,
};
fn server(replies: Vec<(u16, String)>) -> (GraphQlTransport, thread::JoinHandle<Vec<Value>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/graphql", listener.local_addr().unwrap());
    let worker = thread::spawn(move || {
        let mut requests = Vec::new();
        for (status, body) in replies {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut input = Vec::new();
            loop {
                let mut bytes = [0; 8192];
                let n = stream.read(&mut bytes).unwrap();
                assert!(n > 0);
                input.extend_from_slice(&bytes[..n]);
                if let Some((headers, payload)) =
                    std::str::from_utf8(&input).unwrap().split_once("\r\n\r\n")
                {
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            let (key, value) = line.split_once(':')?;
                            key.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().unwrap())
                        })
                        .unwrap();
                    if payload.len() >= length {
                        requests.push(serde_json::from_str(payload).unwrap());
                        break;
                    }
                }
            }
            if status == 0 {
                continue;
            }
            write!(stream,"HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        }
        requests
    });
    let transport = GraphQlTransport::new(
        EndpointUrl::parse(&endpoint).unwrap(),
        ApiKey::new("lin_api_fake".into()).unwrap(),
        TransportConfig {
            ca_bundle: None,
            deadline: Deadline::new(Duration::from_secs(2)).unwrap(),
            max_response_bytes: ResponseCap::new(65536).unwrap(),
        },
    )
    .unwrap();
    (transport, worker)
}
fn scope() -> WorkspaceScope<'static> {
    WorkspaceScope {
        cli_workspace: None,
        sourced_workspace: None,
        default_workspace: None,
        api_key: linear_cli::auth::ApiKeyInput::Absent.clone(),
    }
}
fn entity(id: &str) -> Entity {
    Entity {
        id: id.into(),
        name: id.into(),
    }
}
#[test]
fn optional_sort_order_omission_and_selected_id_are_typed() {
    let make = |sort_order| {
        serde_json::to_value(GraphQlRequest::with_variables(
            AddProjectToInitiative::build(AddVariables {
                input: InitiativeToProjectCreateInput {
                    initiative_id: "i".into(),
                    project_id: "p".into(),
                    sort_order,
                },
            }),
        ))
        .unwrap()
    };
    assert_eq!(
        make(None)["variables"],
        json!({"input":{"initiativeId":"i","projectId":"p"}})
    );
    assert_eq!(make(Some(-1.25))["variables"]["input"]["sortOrder"], -1.25);
    let malformed: Result<AddProjectToInitiative, _> =
        parse_response(br#"{"data":{"initiativeToProjectCreate":null}}"#);
    assert!(malformed.is_err());
}
#[tokio::test]
async fn duplicate_matches_full_extension_partial_data_and_request_only() {
    for (body, project) in [
        (
            json!({"errors":[{"message":"failed","extensions":{"reason":"already exists"}}]}),
            "p",
        ),
        (
            json!({"errors":[{"message":"failed"}],"data":{"unselected":"duplicate"}}),
            "p",
        ),
        (
            json!({"errors":[{"message":"failed"}]}),
            "request-duplicate-only",
        ),
    ] {
        let (transport, worker) = server(vec![(200, body.to_string())]);
        let output = command::add(&transport, &entity("i"), &entity(project), None)
            .await
            .unwrap();
        assert!(
            String::from_utf8(output)
                .unwrap()
                .contains("is already linked")
        );
        assert_eq!(worker.join().unwrap().len(), 1);
    }
}
#[tokio::test]
async fn uppercase_duplicate_is_failure_and_success_metadata_never_reclassifies() {
    let (transport, worker) = server(vec![(
        200,
        json!({"errors":[{"message":"Duplicate"}]}).to_string(),
    )]);
    assert!(
        command::add(&transport, &entity("i"), &entity("p"), None)
            .await
            .unwrap_err()
            .to_string()
            .starts_with(&format!("{}: ", command::ADD_CONTEXT))
    );
    worker.join().unwrap();
    let (transport,worker)=server(vec![(200,json!({"data":{"initiativeToProjectCreate":{"success":true,"initiativeToProject":{"id":"duplicate"}}}}).to_string())]);
    assert_eq!(
        command::add(&transport, &entity("i"), &entity("duplicate"), None)
            .await
            .unwrap(),
        "✓ Added \"duplicate\" to initiative \"i\"\n".as_bytes()
    );
    worker.join().unwrap();
}
#[tokio::test]
async fn false_success_preserves_double_context_for_both_mutations() {
    let (transport,worker)=server(vec![(200,json!({"data":{"initiativeToProjectCreate":{"success":false,"initiativeToProject":{"id":"l"}}}}).to_string())]);
    let error = command::add(&transport, &entity("i"), &entity("p"), None)
        .await
        .unwrap_err();
    assert_eq!(error.message(), command::ADD_CONTEXT);
    assert!(
        error
            .to_string()
            .starts_with(&format!("{}: ", command::ADD_CONTEXT))
    );
    worker.join().unwrap();
    let (transport, worker) = server(vec![(
        200,
        json!({"data":{"initiativeToProjectDelete":{"success":false}}}).to_string(),
    )]);
    let error = command::remove(&transport, "l", &entity("i"), &entity("p"))
        .await
        .unwrap_err();
    assert_eq!(error.message(), command::REMOVE_CONTEXT);
    assert!(
        error
            .to_string()
            .starts_with(&format!("{}: ", command::REMOVE_CONTEXT))
    );
    worker.join().unwrap();
}
#[tokio::test]
async fn uuid_display_catches_graphql_failure_without_consuming_partial_fields() {
    let id = "00000000-0000-4000-9000-000000004101";
    let (transport, worker) = server(vec![(
        200,
        json!({"errors":[{"message":"display failed"}],"data":{"initiative":{"id":"corrupt","name":"Corrupt"}}}).to_string(),
    )]);
    assert_eq!(
        command::resolve_initiative(&transport, id, &scope(), Mode::Add)
            .await
            .unwrap(),
        entity(id)
    );
    worker.join().unwrap();
}
#[tokio::test]
async fn slug_error_then_first_name_accepts_empty_id_exactly_as_source() {
    let (transport,worker)=server(vec![(500,"unavailable".into()),(200,json!({"data":{"projects":{"nodes":[{"id":"","name":"First"},{"id":"other","name":"Second"}]}}}).to_string())]);
    let value = command::resolve_project(&transport, "name", &scope(), Mode::Remove)
        .await
        .unwrap();
    assert_eq!(
        value,
        Entity {
            id: "".into(),
            name: "First".into()
        }
    );
    let requests = worker.join().unwrap();
    assert_eq!(requests[0]["variables"], json!({"slugId":"name"}));
    assert_eq!(requests[1]["variables"], json!({"name":"name"}));
}
#[tokio::test]
async fn link_query_is_first_250_without_pagination_and_removes_first_pair() {
    let (transport,worker)=server(vec![(200,json!({"data":{"initiativeToProjects":{"nodes":[{"id":"first","initiative":{"id":"i"},"project":{"id":"p"}},{"id":"later","initiative":{"id":"i"},"project":{"id":"p"}}]}}}).to_string())]);
    assert_eq!(
        command::find_link(&transport, &entity("i"), &entity("p"))
            .await
            .unwrap(),
        Some("first".into())
    );
    let requests = worker.join().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["variables"], json!({"first":250}));
    assert!(!requests[0]["query"].as_str().unwrap().contains("pageInfo"));
}
