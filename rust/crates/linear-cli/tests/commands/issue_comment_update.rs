//! Public comment-update contracts, including missing optional body and full mutation decoding.
use linear_cli::{
    commands::issue_comment_update as command,
    graphql::{
        envelope::parse_response,
        operations::comment_update::{GetComment, UpdateComment},
    },
    platform::prompt::{PromptOutcome, PromptSession},
};
use serde_json::{Value, json};
use std::{
    io::Cursor,
    time::{SystemTime, UNIX_EPOCH},
};
fn complete(success: bool, url: &str) -> Value {
    json!({"data":{"commentUpdate":{"success":success,"comment":{"id":"returned","body":"server body","updatedAt":"opaque date","url":url,"user":{"name":"Server","displayName":"Display"}}}}})
}
#[test]
fn local_guards_precede_conflict_and_body_files_keep_source_text_policy() {
    let error = command::prepare_body(
        "https://linear.app/acme/issue/abc-1/name#comment-12345678",
        Some("x"),
        Some("missing.md"),
    )
    .unwrap_err();
    assert!(error.message.contains("comment"));
    assert!(!error.message.contains("Cannot specify both"));
    assert_eq!(
        command::prepare_body("opaque", Some("x"), Some("missing.md"))
            .unwrap_err()
            .message,
        "Cannot specify both --body and --body-file"
    );
    let root = std::env::temp_dir().join(format!(
        "c073-files-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    for (name, bytes, expected) in [
        ("bom", b"\xef\xbb\xbfraw\r\n".as_slice(), "raw\r\n"),
        ("space", b" \t\r\n", " \t\r\n"),
        ("empty", b"", ""),
    ] {
        let path = root.join(name);
        std::fs::write(&path, bytes).unwrap();
        let actual = command::prepare_body(" opaque ", None, Some(path.to_str().unwrap()))
            .unwrap()
            .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(command::needs_prompt(Some(&actual)), actual.is_empty());
    }
    std::fs::write(root.join("invalid"), b"a\xffb").unwrap();
    for path in [root.join("missing"), root.clone(), root.join("invalid")] {
        let error =
            command::prepare_body("opaque", None, Some(path.to_str().unwrap())).unwrap_err();
        assert_eq!(
            error.message,
            format!("Failed to read body file: {}", path.display())
        );
        assert!(error.suggestion.unwrap().starts_with("Error: "));
    }
    for body in [" ", "\u{feff}", " raw Markdown\r\n界 "] {
        let actual = command::prepare_body("https://example.test/raw", Some(body), None).unwrap();
        assert_eq!(actual.as_deref(), Some(body));
        assert!(!command::needs_prompt(actual.as_deref()));
    }
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn text_default_raw_semantics_and_blank_eof_errors_keep_outer_context() {
    for default in ["  padded ", "\u{feff}Markdown\r\n界  "] {
        let mut session = PromptSession::script_cr_or_lf(Cursor::new(b"\r"), vec![]);
        assert_eq!(
            command::prompt_body(&mut session, default).unwrap(),
            PromptOutcome::Submitted(default.to_owned())
        );
    }
    let mut session =
        PromptSession::script_cr_or_lf(Cursor::new("  Edited界  \r".as_bytes()), vec![]);
    assert_eq!(
        command::prompt_body(&mut session, "").unwrap(),
        PromptOutcome::Submitted("Edited界".to_owned())
    );
    let output = String::from_utf8(session.into_output().unwrap()).unwrap();
    assert!(output.contains("New comment body ()"));
    for answer in [b"\r".as_slice(), b" \r"] {
        let mut session = PromptSession::script_cr_or_lf(Cursor::new(answer), vec![]);
        assert_eq!(
            command::prompt_body(&mut session, "")
                .unwrap_err()
                .with_context(command::CONTEXT)
                .display_message(),
            "Failed to update comment: Comment body cannot be empty"
        );
    }
    let mut session = PromptSession::script_cr_or_lf(Cursor::new(b""), vec![]);
    assert_eq!(
        command::prompt_body(&mut session, "")
            .unwrap_err()
            .with_context(command::CONTEXT)
            .display_message(),
        "Failed to update comment: unexpected EOF while prompting for comment body"
    );
    for answer in [b"partial".as_slice(), b"\xff\r", b"\0\r"] {
        let mut session = PromptSession::script_cr_or_lf(Cursor::new(answer), vec![]);
        let error = command::prompt_body(&mut session, "seed")
            .unwrap_err()
            .with_context(command::CONTEXT);
        assert!(
            error
                .display_message()
                .starts_with("Failed to update comment:")
        );
    }
}
#[test]
fn whole_optional_shapes_preserve_null_and_missing_body_defaults() {
    for comment in [Value::Null, json!({"body":null}), json!({})] {
        let decoded: GetComment =
            parse_response(json!({"data":{"comment":comment}}).to_string().as_bytes()).unwrap();
        assert!(decoded.comment.and_then(|c| c.body).is_none());
    }
    for success in [true, false] {
        let decoded: UpdateComment = parse_response(
            json!({"data":{"commentUpdate":{"success":success,"comment":null}}})
                .to_string()
                .as_bytes(),
        )
        .unwrap();
        assert_eq!(decoded.comment_update.success, success);
        assert!(decoded.comment_update.comment.is_none());
    }
}
#[tokio::test]
async fn missing_body_get_new_cr_update_proves_cynic_option_and_exact_requests() {
    let (transport, worker) = super::project_write_server::serve(vec![
        json!({"data":{"comment":{}}}).to_string(),
        complete(true, "").to_string(),
    ]);
    let existing = command::existing_body(&transport, " opaque ")
        .await
        .unwrap();
    assert_eq!(existing, "");
    let mut session = PromptSession::script_cr_or_lf(Cursor::new(b"New\r"), vec![]);
    let body = match command::prompt_body(&mut session, &existing).unwrap() {
        PromptOutcome::Submitted(body) => body,
        _ => panic!("submitted expected"),
    };
    assert_eq!(
        command::submit(&transport, " opaque ", body).await.unwrap(),
        "✓ Comment updated\n\n".as_bytes()
    );
    let requests = worker.join().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0]["operationName"], "GetComment");
    assert_eq!(requests[0]["variables"], json!({"id":" opaque "}));
    assert_eq!(requests[1]["operationName"], "UpdateComment");
    assert_eq!(
        requests[1]["variables"],
        json!({"id":" opaque ","input":{"body":"New"}})
    );
}
#[tokio::test]
async fn false_precedes_optional_null_business_error_but_corrupt_required_fields_are_unknown() {
    for success in [false, true] {
        let (transport, worker) = super::project_write_server::serve(vec![
            json!({"data":{"commentUpdate":{"success":success,"comment":null}}}).to_string(),
        ]);
        let error = command::submit(&transport, "raw-id", "body".to_owned())
            .await
            .unwrap_err()
            .with_context(command::CONTEXT);
        assert_eq!(
            error.display_message(),
            if success {
                "Failed to update comment: Comment update failed - no comment returned"
            } else {
                "Failed to update comment: Failed to update comment"
            }
        );
        assert_eq!(worker.join().unwrap().len(), 1);
    }
    for success in [true, false] {
        let mut body = complete(success, "URL");
        body["data"]["commentUpdate"]["comment"]["body"] = json!(5);
        let (transport, worker) = super::project_write_server::serve(vec![body.to_string()]);
        let error = command::submit(&transport, "id", "raw".to_owned())
            .await
            .unwrap_err();
        assert!(error.message.contains("C073-UNEXPECTED-SHAPE"));
        assert!(error.message.contains("update outcome unknown"));
        assert!(!error.message.contains("confirmed"));
        assert_eq!(worker.join().unwrap().len(), 1);
    }
}
#[tokio::test]
async fn friendly_get_and_update_errors_keep_one_action_context_and_no_lookup_fallback() {
    for mutation in [false, true] {
        let (transport,worker)=super::project_write_server::serve(vec![json!({"errors":[{"message":"raw","extensions":{"userPresentableMessage":"Friendly failure"}}]}).to_string()]);
        let error = if mutation {
            command::submit(&transport, "id", "body".to_owned())
                .await
                .unwrap_err()
        } else {
            command::existing_body(&transport, "id").await.unwrap_err()
        };
        assert_eq!(
            error.with_context(command::CONTEXT).display_message(),
            "Failed to update comment: Friendly failure"
        );
        assert_eq!(worker.join().unwrap().len(), 1);
    }
}

use linear_cli::graphql::transport::{
    ApiKey, Deadline, EndpointUrl, GraphQlTransport, ResponseCap, TransportConfig,
};
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
#[tokio::test]
async fn handled_client_fallback_preserves_empty_first_and_non_json_raw_message() {
    for mutation in [false, true] {
        for (status, mime, body) in [
            (
                200,
                "application/json",
                r#"{"errors":[{"message":""},{"message":"boom"}]}"#,
            ),
            (500, "text/plain", "boom"),
        ] {
            let (transport, worker) = server(vec![Reply::raw(status, mime, body)]);
            let error = if mutation {
                command::submit(&transport, " opaque ", "body".to_owned())
                    .await
                    .unwrap_err()
            } else {
                command::existing_body(&transport, " opaque ")
                    .await
                    .unwrap_err()
            };
            let rendered = error.with_context(command::CONTEXT).display_message();
            assert!(rendered.starts_with("Failed to update comment: "));
            assert!(rendered.contains("boom"));
            assert!(rendered.contains("request"));
            assert!(rendered.contains("query"));
            assert!(!rendered.contains("ClientError:"));
            assert!(!rendered.contains("unexpected HTTP status"));
            assert!(!rendered.contains("without an error message"));
            let requests = worker.join().unwrap();
            assert_eq!(requests.len(), 1);
            assert!(
                rendered.contains(
                    requests[0]["query"]
                        .as_str()
                        .unwrap()
                        .split_whitespace()
                        .nth(1)
                        .unwrap()
                        .split('(')
                        .next()
                        .unwrap()
                )
            );
            assert_eq!(requests[0]["variables"]["id"], " opaque ");
        }
    }
}

#[test]
fn stdin_constructor_pipe_child() {
    if std::env::var("C073_PROMPT_CONSTRUCTOR_CHILD").as_deref() != Ok("1") {
        return;
    }
    let mut session = PromptSession::stdin_stdio_cr_or_lf(Vec::new()).unwrap();
    assert_eq!(
        session.text("First:", 1, |_| Ok(())).unwrap(),
        PromptOutcome::Submitted("one".to_owned())
    );
    assert_eq!(
        session.text("Second:", 1, |_| Ok(())).unwrap(),
        PromptOutcome::Submitted("two".to_owned())
    );
    session.close().unwrap();
    let output = String::from_utf8(session.into_output().unwrap()).unwrap();
    assert!(output.contains("? First: › one\n"));
    assert!(output.contains("? Second: › two\n"));
}

#[test]
fn stdin_owned_constructor_reads_actual_pipe_with_cr_framing_and_deferred_lf() {
    use std::process::{Command, Stdio};
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "issue_comment_update::stdin_constructor_pipe_child",
            "--exact",
            "--nocapture",
        ])
        .env_clear()
        .env("C073_PROMPT_CONSTRUCTOR_CHILD", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    for chunk in [b"one\r".as_slice(), b"\n", b"two\r"] {
        input.write_all(chunk).unwrap();
        input.flush().unwrap();
    }
    drop(input);
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("stdin-owned CR constructor did not finish");
        }
        thread::sleep(Duration::from_millis(5));
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "stdout={:?}, stderr={:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn text_topology_refuses_only_terminal_stdin_with_fifo_stdout_before_raw_mode() {
    use linear_cli::error::AppErrorKind;
    for (stdin_tty, stdout_fifo) in [(false, false), (false, true), (true, false)] {
        command::check_prompt_topology(stdin_tty, stdout_fifo).unwrap();
    }
    let error = command::check_prompt_topology(true, true).unwrap_err();
    assert_eq!(error.kind, AppErrorKind::Validation);
    assert!(error.message.starts_with("Comment text prompt requires"));
    assert!(
        error
            .with_context(command::CONTEXT)
            .display_message()
            .starts_with("Failed to update comment: Comment text prompt requires")
    );
}
