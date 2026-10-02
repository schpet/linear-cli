use linear_cli::commands::issue_comment_delete;
use linear_cli::graphql::envelope::parse_response;
use linear_cli::graphql::operations::comment_delete::DeleteComment;
use linear_cli::graphql::transport::{
    ApiKey, Deadline, EndpointUrl, GraphQlTransport, ResponseCap, TransportConfig,
};
use linear_cli::refs::{reject_comment_url, reject_linear_url};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

const ID: &str = "7d2e4f1a-3b5c-4d6e-8f90-a1b2c3d4e5f6";

fn frozen_step(id: &str) -> Value {
    let case: Value = serde_json::from_slice(
        &std::fs::read(format!(
            "{}/../../parity/runner/c074-frozen-cases/{id}.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap(),
    )
    .unwrap();
    case["graphql"]["groups"][0]["steps"][0].clone()
}

#[test]
fn typed_request_matches_source_mutation_and_forwards_raw_ids() {
    let compact = |value: &str| {
        value
            .chars()
            .filter(|ch| !ch.is_whitespace() && *ch != ',')
            .collect::<String>()
    };
    for case in [
        "c074-uuid-success",
        "c074-raw-string-forwarded",
        "c074-non-linear-url-forwarded",
    ] {
        let expected = &frozen_step(case)["operation"];
        let id = expected["variables"]["id"].as_str().unwrap();
        let actual = serde_json::to_value(issue_comment_delete::request(id)).unwrap();
        assert_eq!(
            compact(actual["query"].as_str().unwrap()),
            compact(expected["document"].as_str().unwrap())
        );
        assert_eq!(actual["variables"], expected["variables"], "{case}");
        assert_eq!(actual["operationName"], "DeleteComment");
    }
}

#[test]
fn success_decodes_strictly_as_a_non_null_boolean() {
    let decoded: DeleteComment =
        parse_response(br#"{"data":{"commentDelete":{"success":false}}}"#).unwrap();
    assert!(!decoded.comment_delete.success);
    for payload in [
        json!(null),
        json!({}),
        json!({"success":null}),
        json!({"success":"true"}),
    ] {
        assert!(
            parse_response::<DeleteComment>(
                json!({"data":{"commentDelete":payload}})
                    .to_string()
                    .as_bytes()
            )
            .is_err(),
            "{payload}"
        );
    }
}

#[test]
fn comment_links_in_any_workspace_get_the_specific_rejection() {
    for input in [
        "https://linear.app/acme/issue/ENG-1/fix-login#comment-7d2e4f1a",
        "https://linear.app/other/issue/ENG-1#comment-7d2e4f1a",
    ] {
        let error = reject_comment_url(input).unwrap_err();
        assert_eq!(
            error.message(),
            format!(
                "\"{input}\" links to a comment, but a comment URL only carries the first eight characters of its ID."
            )
        );
        assert_eq!(
            error.hint(),
            Some("Pass the comment's full UUID, from `linear issue comment list <issue> --json`.")
        );
    }
    // Issue URLs without a comment anchor, malformed anchors and other
    // entities fall through to the generic rejection.
    for input in [
        "https://linear.app/acme/issue/ENG-1/fix-login",
        "https://linear.app/acme/issue/ENG-1#foo",
        "https://linear.app/acme/project/mobile-app-abc123def456",
    ] {
        assert!(reject_comment_url(input).is_ok(), "{input}");
        let error = reject_linear_url(input, "a comment UUID").unwrap_err();
        assert_eq!(
            error.message(),
            format!("\"{input}\" is a Linear URL, and this command does not take one.")
        );
        assert_eq!(error.hint(), Some("Pass a comment UUID."));
    }
    for input in [
        ID,
        "  not-a-uuid ",
        "https://example.com/acme/issue/ENG-1#comment-7d2e4f1a",
    ] {
        assert!(reject_comment_url(input).is_ok(), "{input}");
        assert!(
            reject_linear_url(input, "a comment UUID").is_ok(),
            "{input}"
        );
    }
}

#[derive(Clone, Copy, Debug)]
enum Reply {
    Respond(u16, &'static str),
    /// Keep the connection open past the client deadline.
    Silent,
    /// Close after reading the whole request, without a response.
    DropAfterWrite,
}

/// Reads one complete request, answers it as `reply` says, and reports the
/// request body plus whether a second connection arrived afterwards.
fn serve_once(reply: Reply) -> (String, thread::JoinHandle<(String, bool)>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/graphql", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut request = Vec::new();
        let mut chunk = [0_u8; 8192];
        let body = loop {
            let count = stream.read(&mut chunk).unwrap();
            assert!(count > 0, "request closed early");
            request.extend_from_slice(&chunk[..count]);
            let text = String::from_utf8_lossy(&request);
            if let Some((head, body)) = text.split_once("\r\n\r\n") {
                let length = head
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().unwrap())
                    })
                    .unwrap();
                if body.len() >= length {
                    break body.to_owned();
                }
            }
        };
        match reply {
            Reply::Respond(status, body) => {
                let head = format!(
                    "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(head.as_bytes()).unwrap();
                stream.write_all(body.as_bytes()).unwrap();
            }
            Reply::Silent => thread::sleep(Duration::from_millis(250)),
            Reply::DropAfterWrite => {}
        }
        drop(stream);
        thread::sleep(Duration::from_millis(100));
        listener.set_nonblocking(true).unwrap();
        (body, listener.accept().is_ok())
    });
    (endpoint, server)
}

fn transport(endpoint: &str, deadline: Duration) -> GraphQlTransport {
    GraphQlTransport::new(
        EndpointUrl::parse(endpoint).unwrap(),
        ApiKey::new("lin_api_fake".to_owned()).unwrap(),
        TransportConfig {
            ca_bundle: None,
            deadline: Deadline::new(deadline).unwrap(),
            max_response_bytes: ResponseCap::new(65536).unwrap(),
        },
    )
    .unwrap()
}

#[tokio::test]
async fn submit_prints_source_success_and_sends_once() {
    let (endpoint, server) = serve_once(Reply::Respond(
        200,
        r#"{"data":{"commentDelete":{"success":true}}}"#,
    ));
    let output = issue_comment_delete::submit(&transport(&endpoint, Duration::from_secs(5)), ID)
        .await
        .unwrap();
    assert_eq!(output, "✓ Comment deleted\n".as_bytes());
    let (body, retried) = server.join().unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&body).unwrap()["variables"],
        json!({"id": ID})
    );
    assert!(!retried);
}

#[tokio::test]
async fn delete_failures_never_retry_or_add_uncertainty() {
    let cases = [
        (
            Reply::Respond(200, r#"{"data":{"commentDelete":{"success":false}}}"#),
            Duration::from_secs(5),
        ),
        (
            Reply::Respond(200, r#"{"data":{"commentDelete":null}}"#),
            Duration::from_secs(5),
        ),
        (
            Reply::Respond(200, r#"{"data":{"commentDelete":{}}}"#),
            Duration::from_secs(5),
        ),
        (
            Reply::Respond(200, r#"{"errors":[{"message":"Entity not found"}]}"#),
            Duration::from_secs(5),
        ),
        (Reply::Silent, Duration::from_millis(50)),
        (Reply::DropAfterWrite, Duration::from_secs(5)),
    ];
    for (reply, deadline) in cases {
        let (endpoint, server) = serve_once(reply);
        let error = issue_comment_delete::submit(&transport(&endpoint, deadline), ID)
            .await
            .unwrap_err();
        assert!(
            !error.message().contains("may"),
            "{reply:?}: {}",
            error.message()
        );
        if matches!(reply, Reply::Respond(_, body) if body.contains("\"success\":false")) {
            assert_eq!(error.message(), "Failed to delete comment");
        }
        assert!(!server.join().unwrap().1, "delete must not retry");
    }
}
