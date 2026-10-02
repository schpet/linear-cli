use cynic::{MutationBuilder, QueryBuilder};
use linear_cli::commands::{initiative_unarchive as command, initiative_view::Reference};
use linear_cli::graphql::envelope::{GraphQlRequest, parse_response};
use linear_cli::graphql::operations::initiative_unarchive::*;
use linear_cli::graphql::transport::{
    ApiKey, Deadline, EndpointUrl, GraphQlTransport, ResponseCap, TransportConfig,
};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::Duration,
};

const RESPONSE_LIMIT: usize = 65536;

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
            write!(stream,"HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n",body.len()).unwrap();
            // The client rejects an oversized declared length before reading
            // the body. Do not race its expected close by writing that body;
            // the sequential server must still serve the fallback request.
            if body.len() <= RESPONSE_LIMIT {
                stream.write_all(body.as_bytes()).unwrap();
            }
        }
        requests
    });
    let transport = GraphQlTransport::new(
        EndpointUrl::parse(&endpoint).unwrap(),
        ApiKey::new("lin_api_fake".into()).unwrap(),
        TransportConfig {
            ca_bundle: None,
            deadline: Deadline::new(Duration::from_secs(2)).unwrap(),
            max_response_bytes: ResponseCap::new(RESPONSE_LIMIT).unwrap(),
        },
    )
    .unwrap();
    (transport, worker)
}
fn nodes(nodes: Value) -> (u16, String) {
    (
        200,
        json!({"data":{"initiatives":{"nodes":nodes}}}).to_string(),
    )
}
fn frozen(id: &str) -> Value {
    serde_json::from_slice(
        &std::fs::read(format!(
            "{}/../../parity/runner/c046-frozen-cases/c046-{id}.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap(),
    )
    .unwrap()
}
#[test]
fn source_documents_variables_and_selections_are_exact() {
    fn shape<T: serde::Serialize>(request: GraphQlRequest<T>) -> Value {
        serde_json::to_value(request).unwrap()
    }
    let id = "ABCDEFAB-1234-5678-90AB-ABCDEFABCDEF";
    let requests = [
        (
            "slug-first",
            0,
            shape(GraphQlRequest::with_variables(
                GetInitiativeBySlugIncludeArchived::build(SlugVariables {
                    slug_id: "INI-46".into(),
                }),
            )),
        ),
        (
            "name-first",
            1,
            shape(GraphQlRequest::with_variables(
                GetInitiativeByNameIncludeArchived::build(NameVariables {
                    name: "Original Name".into(),
                }),
            )),
        ),
        (
            "uppercase-uuid",
            0,
            shape(GraphQlRequest::with_variables(
                GetInitiativeForUnarchive::build(DetailVariables {
                    id: cynic::Id::new(id),
                }),
            )),
        ),
        (
            "uppercase-uuid",
            1,
            shape(GraphQlRequest::with_variables(UnarchiveInitiative::build(
                UnarchiveVariables { id: id.into() },
            ))),
        ),
    ];
    let compact = |s: &str| {
        s.chars()
            .filter(|c| !c.is_whitespace() && *c != ',')
            .collect::<String>()
    };
    for (case, index, actual) in requests {
        let source = frozen(case);
        let expected = &source["graphql"]["groups"][0]["steps"][index]["operation"];
        assert_eq!(
            compact(actual["query"].as_str().unwrap()),
            compact(expected["document"].as_str().unwrap())
        );
        assert_eq!(actual["variables"], expected["variables"]);
    }
}
#[tokio::test]
async fn slug_and_name_each_take_first_match_preserving_wire_ids() {
    for (slug_reply, expected_ops) in [
        (
            nodes(json!([{"id":"wire-id","slugId":"x"},{"id":"second","slugId":"x"}])),
            1,
        ),
        (nodes(json!([])), 2),
    ] {
        let mut replies = vec![slug_reply];
        if expected_ops == 2 {
            replies.push(nodes(
                json!([{"id":"wire-id","name":"Name"},{"id":"second","name":"Name"}]),
            ));
        }
        let (transport, worker) = server(replies);
        assert_eq!(
            command::resolve_reference(&transport, &Reference::NameOrSlug("Name".into()), "Name")
                .await
                .unwrap(),
            "wire-id"
        );
        let requests = worker.join().unwrap();
        assert_eq!(requests.len(), expected_ops);
        assert_eq!(requests[0]["variables"], json!({"slugId":"Name"}));
        if expected_ops == 2 {
            assert_eq!(requests[1]["variables"], json!({"name":"Name"}));
        }
    }
}
#[tokio::test]
async fn real_exchange_failures_fall_back_but_shapes_do_not() {
    for reply in [
        (0, String::new()),
        (200, "x".repeat(70000)),
        (401, "unauthorized".into()),
        (500, "server failed".into()),
        (200, json!({"errors":[{"message":"no"}]}).to_string()),
        (200, "not-json".into()),
        (200, "{}".into()),
    ] {
        let (transport, worker) = server(vec![reply, nodes(json!([{"id":"id","name":"Name"}]))]);
        assert_eq!(
            command::resolve_reference(&transport, &Reference::NameOrSlug("Name".into()), "Name")
                .await
                .unwrap(),
            "id"
        );
        let requests = worker.join().unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0]["variables"], json!({"slugId":"Name"}));
        assert_eq!(requests[1]["variables"], json!({"name":"Name"}));
    }
    for body in [
        json!([]),
        json!({"data":{"initiatives":{"nodes":[{"id":"first","slugId":"x"},null]}}}),
    ] {
        let (transport, worker) = server(vec![(200, body.to_string())]);
        let error =
            command::resolve_reference(&transport, &Reference::NameOrSlug("Name".into()), "Name")
                .await
                .unwrap_err();
        assert!(error.message.contains("expected operation shape"));
        assert_eq!(worker.join().unwrap().len(), 1);
    }
}
#[tokio::test]
async fn url_is_archived_minimal_and_nonuuid_return_uses_returned_token() {
    let (transport, worker) = server(vec![
        nodes(json!([{"id":"token"}])),
        nodes(json!([{"id":"resolved","slugId":"token"}])),
    ]);
    assert_eq!(
        command::resolve_reference(&transport, &Reference::UrlSlug("INI-46".into()), "raw URL")
            .await
            .unwrap(),
        "resolved"
    );
    let requests = worker.join().unwrap();
    assert_eq!(
        requests[0]["variables"],
        json!({"slugId":"INI-46","includeArchived":true})
    );
    assert_eq!(requests[1]["variables"], json!({"slugId":"token"}));
    assert!(!requests[0]["query"].as_str().unwrap().contains("name"));
}
#[tokio::test]
async fn url_miss_error_and_empty_resolved_id_never_fall_back() {
    for (reference, response) in [
        (Reference::UrlSlug("x".into()), nodes(json!([]))),
        (Reference::UrlSlug("x".into()), (401, "denied".into())),
        (
            Reference::NameOrSlug("x".into()),
            nodes(json!([{"id":"","slugId":"x"}])),
        ),
    ] {
        let (transport, worker) = server(vec![response]);
        assert!(
            command::resolve_reference(&transport, &reference, "raw")
                .await
                .is_err()
        );
        assert_eq!(worker.join().unwrap().len(), 1);
    }
}
#[tokio::test]
async fn active_truthiness_and_details_missing_context_preserve_source() {
    for value in [Value::Null, json!("")] {
        let (transport, worker) = server(vec![nodes(
            json!([{"id":"other-id","slugId":"s","name":"Before","archivedAt":value}]),
        )]);
        let detail = command::fetch_details(&transport, "resolved-id", "raw")
            .await
            .unwrap();
        assert_eq!(
            command::active_output(&detail).unwrap(),
            b"Initiative \"Before\" is not archived.\n"
        );
        assert_eq!(
            worker.join().unwrap()[0]["variables"],
            json!({"id":"resolved-id"})
        );
    }
    let (transport, worker) = server(vec![nodes(json!([]))]);
    assert_eq!(
        command::fetch_details(&transport, "id", "raw URL")
            .await
            .unwrap_err()
            .context
            .as_deref(),
        Some("Failed to resolve initiative")
    );
    worker.join().unwrap();
}
#[tokio::test]
async fn nullable_entity_empty_fields_and_false_success_are_distinct() {
    for (success, entity, expected) in [
        (
            true,
            Value::Null,
            Some("✓ Unarchived initiative: undefined\n"),
        ),
        (
            true,
            json!({"id":"","slugId":"","name":"","url":""}),
            Some("✓ Unarchived initiative: \n"),
        ),
        (false, Value::Null, None),
    ] {
        let (transport, worker) = server(vec![(
            200,
            json!({"data":{"initiativeUnarchive":{"success":success,"entity":entity}}}).to_string(),
        )]);
        let result = command::submit(&transport, "resolved-wire-id").await;
        match expected {
            Some(output) => assert_eq!(result.unwrap(), output.as_bytes()),
            None => assert_eq!(
                result.unwrap_err().message,
                "Failed to unarchive initiative"
            ),
        }
        assert_eq!(
            worker.join().unwrap()[0]["variables"],
            json!({"id":"resolved-wire-id"})
        );
    }
}
#[test]
fn selected_required_fields_fail_strictly_and_confirmation_defaults_yes() {
    for payload in [
        json!({"entity":null}),
        json!({"success":null,"entity":null}),
        json!({"success":true,"entity":{"id":"i"}}),
    ] {
        assert!(
            parse_response::<UnarchiveInitiative>(
                json!({"data":{"initiativeUnarchive":payload}})
                    .to_string()
                    .as_bytes()
            )
            .is_err()
        );
    }
    use linear_cli::platform::prompt::{PromptOutcome, PromptSession};
    let mut prompt = PromptSession::script(std::io::Cursor::new(b"\n"), Vec::new());
    assert_eq!(
        prompt.confirm("Unarchive?", true).unwrap(),
        PromptOutcome::Submitted(true)
    );
}

#[tokio::test]
async fn mutation_exchange_errors_are_contextual_and_never_retry_or_fall_back() {
    for response in [
        (401, "unauthorized".to_owned()),
        (
            200,
            json!({"errors":[{"message":"write denied"}]}).to_string(),
        ),
        (
            200,
            json!({"data":{"initiativeUnarchive":null}}).to_string(),
        ),
    ] {
        let (transport, worker) = server(vec![response]);
        let error = command::submit(&transport, "resolved-id")
            .await
            .unwrap_err();
        assert_eq!(error.context.as_deref(), Some(command::CONTEXT));
        let requests = worker.join().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0]["operationName"], "UnarchiveInitiative");
    }
}
