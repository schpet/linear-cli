//! Public command contracts through fake local HTTP.
use linear_cli::{
    app::block_on_network,
    commands::{issue_commits, issue_describe},
    graphql::{
        bulk_error::{ObservedExchangeFailure, SourceException, SourceExceptionKind},
        transport::{
            ApiKey, CaMode, Deadline, EndpointUrl, GraphQlTransport, ProxyMode, ResponseCap,
            TransportConfig,
        },
    },
};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::{Duration, Instant},
};
struct Reply {
    status: u16,
    mime: &'static str,
    body: String,
}
impl Reply {
    fn data(body: Value) -> Self {
        Self {
            status: 200,
            mime: "application/json",
            body: body.to_string(),
        }
    }
    fn raw(status: u16, mime: &'static str, body: &str) -> Self {
        Self {
            status,
            mime,
            body: body.to_owned(),
        }
    }
}
fn server(replies: Vec<Reply>) -> (GraphQlTransport, thread::JoinHandle<Vec<Value>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let endpoint = format!("http://{}/graphql", listener.local_addr().unwrap());
    let worker = thread::spawn(move || {
        let mut requests = vec![];
        for reply in replies {
            let started = Instant::now();
            let mut socket = loop {
                match listener.accept() {
                    Ok((socket, _)) => break socket,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            started.elapsed() < Duration::from_secs(4),
                            "expected request missing"
                        );
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(error) => panic!("accept: {error}"),
                }
            };
            socket
                .set_nonblocking(false)
                .expect("blocking accepted mock stream");
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut bytes = vec![];
            loop {
                let mut byte = [0];
                socket.read_exact(&mut byte).unwrap();
                bytes.push(byte[0]);
                if bytes.ends_with(b"\r\n\r\n") {
                    break;
                }
            }
            let length = std::str::from_utf8(&bytes)
                .unwrap()
                .lines()
                .find_map(|line| {
                    let (key, value) = line.split_once(':')?;
                    key.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .unwrap();
            let mut body = vec![0; length];
            socket.read_exact(&mut body).unwrap();
            requests.push(serde_json::from_slice(&body).unwrap());
            write!(socket, "HTTP/1.1 {} OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", reply.status, reply.mime, reply.body.len(), reply.body).unwrap();
        }
        assert!(listener.accept().is_err(), "unexpected request");
        requests
    });
    let transport = GraphQlTransport::new(
        EndpointUrl::parse(&endpoint).unwrap(),
        ApiKey::new("fixture-key".into()).unwrap(),
        TransportConfig {
            proxy: ProxyMode::Direct,
            ca: CaMode::PublicRoots,
            deadline: Deadline::new(Duration::from_secs(2)).unwrap(),
            max_response_bytes: ResponseCap::new(65536).unwrap(),
        },
    )
    .unwrap();
    (transport, worker)
}

#[test]
fn optional_commits_selected_absence_is_notfound_after_exact_one_read() {
    for response in [
        json!({"data":{"issue":null}}),
        json!({"data":{}}),
        json!({"data":{"issue":{}}}),
        json!({"data":{"issue":{"id":null}}}),
        json!({"data":{"issue":{"id":""}}}),
    ] {
        let (transport, worker) = server(vec![Reply::data(response)]);
        let error = block_on_network(issue_commits::lookup(&transport, "ENG-7")).unwrap_err();
        assert_eq!(error.message, "Issue not found: ENG-7");
        let requests = worker.join().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0]["operationName"], "GetIssueId");
        assert_eq!(requests[0]["variables"], json!({"id":"ENG-7"}));
        assert_eq!(
            requests[0]["query"],
            "query GetIssueId($id: String!) {\n  issue(id: $id) {\n    id\n  }\n}"
        );
        assert_eq!(
            requests[0]["query"]
                .as_str()
                .unwrap()
                .split_whitespace()
                .collect::<Vec<_>>(),
            vec![
                "query",
                "GetIssueId($id:",
                "String!)",
                "{",
                "issue(id:",
                "$id)",
                "{",
                "id",
                "}",
                "}"
            ]
        );
    }
}
#[test]
fn opaque_id_is_presence_but_wrong_selected_types_are_strict_after_one_read() {
    let (transport, worker) = server(vec![Reply::data(
        json!({"data":{"issue":{"id":"opaque non-UUID"}}}),
    )]);
    block_on_network(issue_commits::lookup(&transport, "ENG-7")).unwrap();
    assert_eq!(worker.join().unwrap().len(), 1);
    for response in [
        json!({"data":{"issue":[]}}),
        json!({"data":{"issue":{"id":7}}}),
        json!({"data":{"issue":{"id":false}}}),
    ] {
        let (transport, worker) = server(vec![Reply::data(response)]);
        let error = block_on_network(issue_commits::lookup(&transport, "ENG-7")).unwrap_err();
        assert_ne!(error.message, "Issue not found: ENG-7");
        assert_eq!(worker.join().unwrap().len(), 1);
    }
}
#[test]
fn both_leaves_keep_raw_http_fallback_but_only_commits_translate_client_notfound() {
    for leaf in ["commits", "describe"] {
        let (transport, worker) = server(vec![Reply::raw(
            500,
            "text/plain",
            "DUMMY Entity not found",
        )]);
        let error = if leaf == "commits" {
            block_on_network(issue_commits::lookup(&transport, "ENG-7")).unwrap_err()
        } else {
            block_on_network(issue_describe::fetch(&transport, "ENG-7")).unwrap_err()
        };
        if leaf == "commits" {
            assert_eq!(error.message, "Issue not found: ENG-7");
        } else {
            assert!(error.message.starts_with("GraphQL Error (Code: 500): "));
            assert!(error.message.contains("DUMMY Entity not found"));
            assert!(error.message.contains("\"response\""));
            assert!(error.message.contains("\"request\""));
        }
        assert_eq!(worker.join().unwrap().len(), 1);
    }
}
#[test]
fn first_empty_message_preserves_sdk_metadata_and_presentable_message_wins() {
    for leaf in ["commits", "describe"] {
        let (transport, worker) = server(vec![Reply::data(
            json!({"errors":[{"message":""},{"message":"Entity not found"}]}),
        )]);
        let error = if leaf == "commits" {
            block_on_network(issue_commits::lookup(&transport, "ENG-7")).unwrap_err()
        } else {
            block_on_network(issue_describe::fetch(&transport, "ENG-7")).unwrap_err()
        };
        if leaf == "commits" {
            assert_eq!(error.message, "Issue not found: ENG-7");
        } else {
            assert!(error.message.starts_with(": {"));
            assert!(
                error
                    .message
                    .contains("\"errors\":[{\"message\":\"\"},{\"message\":\"Entity not found\"}]")
            );
        }
        assert_eq!(worker.join().unwrap().len(), 1);
        let (transport, worker) = server(vec![Reply::data(
            json!({"errors":[{"message":"Entity not found","extensions":{"userPresentableMessage":"DUMMY preferred"}}]}),
        )]);
        let error = if leaf == "commits" {
            block_on_network(issue_commits::lookup(&transport, "ENG-7")).unwrap_err()
        } else {
            block_on_network(issue_describe::fetch(&transport, "ENG-7")).unwrap_err()
        };
        assert_eq!(error.message, "DUMMY preferred");
        assert_eq!(worker.join().unwrap().len(), 1);
    }
    let plain = SourceException {
        kind: SourceExceptionKind::Plain,
        message: "Entity not found".into(),
        preferred_message: None,
    };
    assert_eq!(
        issue_commits::lookup_failure(ObservedExchangeFailure::Ordinary(plain), "ENG-7").message,
        "Entity not found"
    );
}
#[test]
fn describe_full_shape_rejects_source_success_minimal_without_partial_print() {
    let (transport, worker) = server(vec![Reply::data(
        json!({"data":{"issue":{"title":"DUMMY minimal","url":"https://example.invalid/dummy"}}}),
    )]);
    assert!(block_on_network(issue_describe::fetch(&transport, "ENG-7")).is_err());
    let request = worker.join().unwrap();
    assert_eq!(request.len(), 1);
    assert_eq!(request[0]["operationName"], "GetIssueDetails");
    assert_eq!(request[0]["variables"], json!({"id":"ENG-7"}));
}
#[test]
fn description_retains_resolved_reference_and_exact_raw_text_and_trailer_alias_meaning() {
    for (references, magic) in [(false, "Fixes"), (true, "References")] {
        assert_eq!(issue_describe::format("ENG-7","DUMMY\nsecond\t界","https://example.invalid/x#frag",references),format!("ENG-7 DUMMY\nsecond\t界\n\nLinear-issue: {magic} ENG-7\nLinear-issue-url: https://example.invalid/x#frag\n").as_bytes());
        assert_eq!(
            issue_describe::format("ENG-7", "", "", references),
            format!("ENG-7 \n\nLinear-issue: {magic} ENG-7\nLinear-issue-url: \n").as_bytes()
        );
    }
}
