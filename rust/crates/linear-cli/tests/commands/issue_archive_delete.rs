//! Public command boundaries with captured local wire requests; no real API writes.
use linear_cli::commands::bulk::{BulkInput, BulkOutcome};
use linear_cli::{
    auth::ApiKeyInput,
    commands::issue::archive::{self as command, Mode, Target},
    graphql::transport::{
        ApiKey, Deadline, EndpointUrl, GraphQlTransport, ResponseCap, TransportConfig,
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
fn failure(row: &linear_cli::commands::bulk::BulkResult) -> &str {
    match &row.outcome {
        BulkOutcome::Failed(message) => message,
        BulkOutcome::Succeeded => panic!("expected failure"),
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
        linear_cli::commands::bulk::collect_ids(&input, &mut &b"ENG-2,\xff"[..])
            .unwrap_err()
            .message()
            .starts_with("Bulk file must be valid UTF-8")
    );
    std::fs::write(&file, "\u{feff}ENG-1,ENG-2\n ENG-1\tjoined\n").unwrap();
    assert_eq!(
        linear_cli::commands::bulk::collect_ids(
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
        linear_cli::commands::bulk::collect_ids(&strict_stdin, &mut &b"\xff"[..])
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
    let mut rows = vec![];
    for id in ids {
        rows.push(command::run_item(&transport, target(id), Mode::Archive).await);
    }
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
    let requests = worker.join().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(
        requests
            .iter()
            .all(|request| request["variables"]["id"] == "ENG-1")
    );
}
#[tokio::test]
async fn archive_not_found_and_already_archived_skips_write() {
    for reply in [Reply::data(json!({"data":{"issue":null}}))] {
        let (transport, worker) = server(vec![reply]);
        let row = command::run_item(&transport, target("eng-1"), Mode::Archive).await;
        assert_eq!(row.id, "ENG-1");
        assert_eq!(failure(&row), "Issue not found");
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
        Reply::data(json!({"data":{"issue":{"identifier":null,"title":"Name"}}})),
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
