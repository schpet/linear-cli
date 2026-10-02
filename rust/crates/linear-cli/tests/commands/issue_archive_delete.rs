//! Public command boundaries with captured local wire requests; no real API writes.
use linear_cli::{
    auth::ApiKeyInput,
    commands::{
        initiative_bulk::{self, BulkInput, BulkOutcome},
        issue_archive_delete::{self as command, Mode, Target},
    },
    graphql::{
        bulk_error,
        transport::{
            ApiKey, Deadline, EndpointUrl, GraphQlTransport, RawHttpResponse, ResponseCap,
            TransportConfig,
        },
    },
    refs::WorkspaceScope,
};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
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
        cli_workspace: Some("acme"),
        sourced_workspace: None,
        default_workspace: None,
        api_key: ApiKeyInput::Absent.clone(),
    }
}
fn target(id: &str) -> Target {
    Target::prepare(id.to_owned(), None, &scope())
}
fn details(archived: Value) -> Reply {
    Reply::data(
        json!({"data":{"issue":{"identifier":"ENG-99","title":"Name","archivedAt":archived}}}),
    )
}
fn mutation(mode: Mode, success: Value) -> Reply {
    Reply::data(match mode {
        Mode::Archive => json!({"data":{"issueArchive":{"success":success}}}),
        Mode::Delete => json!({"data":{"issueDelete":{"success":success}}}),
    })
}
fn failure(row: &initiative_bulk::BulkResult) -> &str {
    match &row.outcome {
        BulkOutcome::Failed(message) => message,
        BulkOutcome::Succeeded => panic!("expected failure"),
    }
}
#[test]
fn observer_classifies_only_client_errors_and_first_preferred_message() {
    let request = command::details_request("ENG-1", Mode::Archive, true);
    let cases = [
        (404, "text/plain", "Not Found", true),
        (500, "text/plain", "boom", false),
        (200, "text/plain", "not found", false),
        (
            200,
            "application/json",
            r#"{"errors":[{"message":"Entity not found","extensions":{"userPresentableMessage":"Denied"}}]}"#,
            false,
        ),
        (
            200,
            "application/json",
            r#"{"errors":[{"message":"Entity not found","extensions":{"userPresentableMessage":""}}]}"#,
            true,
        ),
        (
            400,
            "application/json",
            r#"{"errors":[{"message":""},{"message":"Entity not found"}]}"#,
            true,
        ),
        (
            400,
            "application/json",
            r#"{"errors":[{"message":""},{"message":"boom"}]}"#,
            false,
        ),
        (
            400,
            "application/json",
            r#"{"errors":[{"message":"Denied"},{"message":"Entity not found"}]}"#,
            false,
        ),
    ];
    for (status, mime, body, expected) in cases {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::CONTENT_TYPE, mime.parse().unwrap());
        let response = RawHttpResponse {
            status: reqwest::StatusCode::from_u16(status).unwrap(),
            headers,
            body: body.as_bytes().to_vec(),
        };
        let observed = bulk_error::observe_source_error(&response, &request)
            .ok()
            .unwrap()
            .unwrap();
        assert_eq!(observed.is_not_found(), expected, "{status} {mime} {body}");
        assert_eq!(
            Some(observed.message),
            bulk_error::source_error(&response, &request).ok().unwrap(),
            "opt-in observer must preserve SDK bytes"
        );
    }
}
#[test]
fn bulk_ids_reject_invalid_utf8_and_split_on_commas_and_whitespace() {
    let dir = std::env::temp_dir().join(format!(
        "issue-lossy-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&dir).unwrap();
    let file = dir.join("ids");
    std::fs::write(&file, b"ENG-1,\xff").unwrap();
    let argv = vec![" Raw, argv ".to_owned(), "".to_owned()];
    let input = BulkInput {
        argv: Some(&argv),
        file: Some(&file),
        stdin: true,
    };
    assert!(
        initiative_bulk::collect_ids(&input, &mut &b"ENG-2,\xff"[..])
            .unwrap_err()
            .message()
            .starts_with("Bulk file must be valid UTF-8")
    );
    std::fs::write(&file, "\u{feff}ENG-1,ENG-2\n ENG-1\tjoined\n").unwrap();
    assert_eq!(
        initiative_bulk::collect_ids(
            &BulkInput {
                argv: None,
                file: Some(&file),
                stdin: false
            },
            &mut &b""[..],
        )
        .unwrap(),
        vec!["ENG-1", "ENG-2", "joined"]
    );
    let strict_stdin = BulkInput {
        argv: None,
        file: None,
        stdin: true,
    };
    assert_eq!(
        initiative_bulk::collect_ids(&strict_stdin, &mut &b"\xff"[..])
            .unwrap_err()
            .message(),
        "Bulk stdin must be valid UTF-8"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[tokio::test]
async fn local_reference_failures_are_ordered_rows_and_do_not_stop_following_valid_input() {
    let (transport, worker) = server(vec![
        details(Value::Null),
        mutation(Mode::Archive, json!(true)),
    ]);
    let ids = [
        "00000000-0000-4000-8000-000000000001",
        "3",
        "https://linear.app/acme/settings/x",
        "eng-1",
    ];
    let mut progress = vec![];
    let rows = command::execute(
        &transport,
        ids.iter().map(|id| target(id)).collect(),
        Mode::Archive,
        |value| {
            progress.push(value);
            Ok(())
        },
    )
    .await
    .unwrap();
    assert_eq!(
        rows.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
        vec![ids[0], ids[1], ids[2], "ENG-1"]
    );
    assert_eq!(failure(rows.first().unwrap()), "Issue not found");
    assert_eq!(
        failure(rows.get(1).unwrap()),
        "an integer id was provided, but no team is set"
    );
    assert_eq!(
        failure(rows.get(2).unwrap()),
        "\"https://linear.app/acme/settings/x\" is a Linear URL, but \"settings\" is not an entity this command can use."
    );
    assert!(rows.last().unwrap().succeeded());
    assert_eq!(progress.len(), 4);
    assert!(progress.iter().all(|value| value.succeeded == 0));
    let requests = worker.join().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(
        requests
            .iter()
            .all(|request| request["variables"]["id"] == "ENG-1")
    );
}
#[tokio::test]
async fn archive_not_found_translation_is_client_only_and_already_archived_skips_write() {
    for reply in [
        Reply::raw(404, "text/plain", "Not Found"),
        Reply::data(json!({"data":{"issue":null}})),
    ] {
        let (transport, worker) = server(vec![reply]);
        let row = command::run_item(&transport, target("eng-1"), Mode::Archive).await;
        assert_eq!(row.id, "ENG-1");
        assert_eq!(failure(&row), "Issue not found");
        assert_eq!(worker.join().unwrap().len(), 1);
    }
    for reply in [
        Reply::raw(200, "text/plain", "not found"),
        Reply::data(
            json!({"errors":[{"message":"Entity not found","extensions":{"userPresentableMessage":"Denied"}}]}),
        ),
    ] {
        let (transport, worker) = server(vec![reply]);
        let row = command::run_item(&transport, target("eng-1"), Mode::Archive).await;
        assert_eq!(row.id, "eng-1");
        assert!(row.name.is_none());
        assert_ne!(failure(&row), "Issue not found");
        assert_eq!(worker.join().unwrap().len(), 1);
    }
    let (transport, worker) = server(vec![details(json!("2026-01-01T00:00:00Z"))]);
    let row = command::run_item(&transport, target("eng-1"), Mode::Archive).await;
    assert!(row.succeeded());
    assert_eq!(row.name.as_deref(), Some("ENG-99: Name"));
    assert_eq!(worker.join().unwrap().len(), 1);
}
#[tokio::test]
async fn delete_catches_every_details_failure_but_still_sends_one_mutation() {
    let replies = [
        Reply::data(json!({"data":{"issue":null}})),
        Reply::raw(200, "application/json", "{"),
        Reply::data(json!({"bad":true})),
        Reply::data(json!({"data":{"issue":{"identifier":null,"title":"Name"}}})),
        Reply::raw(200, "text/plain", r#"{"data":{"issue":null}}"#),
    ];
    for reply in replies {
        let (transport, worker) = server(vec![reply, mutation(Mode::Delete, json!(false))]);
        let row = command::run_item(&transport, target("eng-1"), Mode::Delete).await;
        assert_eq!(row.id, "ENG-1");
        assert_eq!(row.name.as_deref(), Some("ENG-1"));
        assert_eq!(failure(&row), "Delete operation failed");
        let requests = worker.join().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(
            requests.last().unwrap()["query"]
                .as_str()
                .unwrap()
                .starts_with("mutation BulkDeleteIssue")
        );
        assert!(
            requests
                .iter()
                .all(|request| request["variables"]["id"] == "ENG-1")
        );
    }
}
#[tokio::test]
async fn strict_details_and_mutation_boundaries_fail_original_rows_without_retries() {
    for reply in [
        Reply::raw(200, "application/json", "{"),
        Reply::data(json!({"data":{"issue":{"identifier":null,"title":"Name","archivedAt":null}}})),
        details(json!(false)),
    ] {
        let (transport, worker) = server(vec![reply]);
        let row = command::run_item(&transport, target("eng-1"), Mode::Archive).await;
        assert_eq!(row.id, "eng-1");
        assert!(row.name.is_none());
        assert!(!failure(&row).is_empty());
        assert_eq!(worker.join().unwrap().len(), 1);
    }
    for mode in [Mode::Archive, Mode::Delete] {
        let field = match mode {
            Mode::Archive => "issueArchive",
            Mode::Delete => "issueDelete",
        };
        let mut replies = vec![mutation(mode, json!("true")), mutation(mode, Value::Null)];
        replies.push(Reply::data(json!({"data":{field:{}}})));
        replies.push(Reply::data(json!({"data":{field:null}})));
        for reply in replies {
            let (transport, worker) = server(vec![details(Value::Null), reply]);
            let row = command::run_item(&transport, target("eng-1"), mode).await;
            assert_eq!(row.id, "eng-1");
            assert!(row.name.is_none());
            assert!(!failure(&row).is_empty());
            assert_eq!(worker.join().unwrap().len(), 2);
        }
    }
}
#[tokio::test]
async fn single_paths_preserve_source_display_false_errors_and_resolved_mutation_id() {
    for mode in [Mode::Archive, Mode::Delete] {
        let last = match mode {
            Mode::Archive => mutation(mode, json!(true)),
            Mode::Delete => Reply::data(
                json!({"data":{"issueDelete":{"success":true,"entity":{"identifier":"CHANGED-2","title":"Changed"}}}}),
            ),
        };
        let (transport, worker) = server(vec![details(Value::Null), last]);
        let found = command::single_details(&transport, "ENG-1", mode)
            .await
            .unwrap();
        assert_eq!(
            command::submit_single(&transport, "ENG-1", &found, mode)
                .await
                .unwrap(),
            format!("✓ Successfully {} issue: ENG-99: Name\n", mode.past()).as_bytes()
        );
        let requests = worker.join().unwrap();
        assert!(
            requests
                .iter()
                .all(|request| request["variables"]["id"] == "ENG-1")
        );
        let last = match mode {
            Mode::Archive => mutation(mode, json!(false)),
            Mode::Delete => {
                Reply::data(json!({"data":{"issueDelete":{"success":false,"entity":null}}}))
            }
        };
        let (transport, worker) = server(vec![last]);
        assert_eq!(
            command::submit_single(&transport, "ENG-1", &found, mode)
                .await
                .unwrap_err()
                .message(),
            match mode {
                Mode::Archive => "Linear reported the archive as unsuccessful",
                Mode::Delete => "Failed to delete issue",
            }
        );
        worker.join().unwrap();
    }
}
#[test]
fn failed_summary_preserves_multiline_raw_messages_and_source_verbs() {
    let row = initiative_bulk::BulkResult {
        id: "Original".to_owned(),
        name: None,
        outcome: BulkOutcome::Failed("raw\r\nSDK metadata".to_owned()),
    };
    for (mode, verb) in [(Mode::Archive, "archive"), (Mode::Delete, "delete")] {
        let (bytes, failed) = command::summary(std::slice::from_ref(&row), mode);
        assert!(failed);
        assert_eq!(bytes,format!("\n✗ Failed to {verb} all 1 issue\n\nFailed operations:\n  - Original: raw\r\nSDK metadata\n").as_bytes());
    }
}

#[tokio::test]
async fn single_client_errors_preserve_empty_first_and_non_json_raw_sdk_fallback() {
    for mode in [Mode::Archive, Mode::Delete] {
        for (status, mime, body) in [
            (
                200,
                "application/json",
                r#"{"errors":[{"message":""},{"message":"boom"}]}"#,
            ),
            (500, "text/plain", "boom"),
        ] {
            let request = command::details_request("ENG-1", mode, false);
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(reqwest::header::CONTENT_TYPE, mime.parse().unwrap());
            let response = RawHttpResponse {
                status: reqwest::StatusCode::from_u16(status).unwrap(),
                headers,
                body: body.as_bytes().to_vec(),
            };
            let expected = bulk_error::source_error(&response, &request)
                .ok()
                .unwrap()
                .unwrap();
            let (transport, worker) = server(vec![Reply::raw(status, mime, body)]);
            let error = match command::single_details(&transport, "ENG-1", mode).await {
                Ok(_) => panic!("expected single ClientError"),
                Err(error) => error,
            };
            assert_eq!(worker.join().unwrap().len(), 1);
            assert_eq!(error.message(), expected, "{mode:?} {status} {mime}");
        }
    }
}
