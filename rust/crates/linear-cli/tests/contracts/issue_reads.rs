//! Actual public CLI request-order and handled-error contracts, synthetic API only.
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
struct Reply {
    operation: &'static str,
    status: u16,
    mime: &'static str,
    body: String,
}
impl Reply {
    fn data(operation: &'static str, data: Value) -> Self {
        Self {
            operation,
            status: 200,
            mime: "application/json",
            body: json!({"data":data}).to_string(),
        }
    }
}
struct Server {
    endpoint: String,
    done: Arc<AtomicBool>,
    worker: thread::JoinHandle<Vec<Value>>,
}
impl Server {
    fn new(replies: Vec<Reply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}/graphql", listener.local_addr().unwrap());
        let done = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&done);
        let worker = thread::spawn(move || {
            let mut requests = vec![];
            let start = Instant::now();
            while !stop.load(Ordering::SeqCst) {
                let (mut stream, _) = match listener.accept() {
                    Ok(v) => v,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            start.elapsed() < Duration::from_secs(8),
                            "CLI did not complete"
                        );
                        thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(e) => panic!("{e}"),
                };
                stream
                    .set_nonblocking(false)
                    .expect("blocking accepted mock stream");
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut bytes = vec![];
                let request = loop {
                    let mut chunk = [0; 8192];
                    let n = stream.read(&mut chunk).unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&chunk[..n]);
                    let text = std::str::from_utf8(&bytes).unwrap();
                    if let Some((header, body)) = text.split_once("\r\n\r\n") {
                        let length = header
                            .lines()
                            .find_map(|line| {
                                let (k, v) = line.split_once(':')?;
                                k.eq_ignore_ascii_case("content-length")
                                    .then(|| v.trim().parse::<usize>().unwrap())
                            })
                            .unwrap();
                        if body.len() >= length {
                            break serde_json::from_str::<Value>(&body[..length]).unwrap();
                        }
                    }
                };
                let index = requests.len();
                requests.push(request);
                let fallback = Reply {
                    operation: "UNEXPECTED",
                    status: 200,
                    mime: "application/json",
                    body: "{\"errors\":[{\"message\":\"unexpected additional request\"}]}"
                        .to_owned(),
                };
                let reply = replies.get(index).unwrap_or(&fallback);
                if reply.operation != "UNEXPECTED" {
                    assert_eq!(requests[index]["operationName"], reply.operation);
                }
                write!(stream,"HTTP/1.1 {} Fixture\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",reply.status,reply.mime,reply.body.len(),reply.body).unwrap();
            }
            assert!(
                requests.len() >= replies.len(),
                "missing source resolver prefix"
            );
            requests
        });
        Self {
            endpoint,
            done,
            worker,
        }
    }
    fn finish(self) -> Vec<Value> {
        self.done.store(true, Ordering::SeqCst);
        self.worker.join().unwrap()
    }
}
fn scope() -> Reply {
    Reply::data(
        "ResolveTeam",
        json!({"teams":{"nodes":[{"id":"team-id","key":"ENG","name":"Engineering"}]}}),
    )
}
fn invoke(server: &Server, args: &[&str], sort: Option<&str>) -> std::process::Output {
    let sandbox = super::sandbox::BinarySandbox::new();
    let mut command = sandbox.command();
    command
        .env("LINEAR_API_KEY", "lin_api_fake")
        .env("LINEAR_TEAM_ID", "ENG")
        .env("LINEAR_GRAPHQL_ENDPOINT", &server.endpoint)
        .env("NO_COLOR", "1")
        .env("TZ", "UTC");
    if let Some(sort) = sort {
        command.env("LINEAR_ISSUE_SORT", sort);
    }
    command.args(args).output().unwrap()
}
#[test]
fn invalid_configured_sort_fails_at_startup() {
    let server = Server::new(vec![]);
    let output = invoke(
        &server,
        &["issue", "mine", "--team", "eng"],
        Some("invalid"),
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .starts_with("✗ invalid config option LINEAR_ISSUE_SORT")
    );
    assert!(server.finish().is_empty());
    // Help never reads configuration.
    let sandbox = super::sandbox::BinarySandbox::new();
    let output = sandbox
        .command()
        .env("LINEAR_ISSUE_SORT", "invalid")
        .args(["team", "list", "--help"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
}
#[test]
fn errors_skip_an_empty_first_message_and_name_the_http_status() {
    for (status, mime, body) in [
        (
            200,
            "application/json",
            "{\"errors\":[{\"message\":\"\"},{\"message\":\"boom\"}],\"extensions\":{\"value\":9007199254740993}}",
        ),
        (500, "text/plain", "raw fixture failure"),
    ] {
        let server = Server::new(vec![Reply {
            operation: "GetIssuesForQuery",
            status,
            mime,
            body: body.to_owned(),
        }]);
        let output = invoke(&server, &["issue", "query", "--all-teams", "--json"], None);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.starts_with("✗ Failed to query issues: "));
        assert!(stderr.contains(if status == 500 { "500" } else { "boom" }));
        assert!(!stderr.contains("ClientError:"));
        assert_eq!(server.finish().len(), 1);
    }
}

#[test]
fn default_team_cycle_url_error_follows_one_team_lookup() {
    let server = Server::new(vec![scope()]);
    let output = invoke(
        &server,
        &[
            "issue",
            "mine",
            "--cycle",
            "https://linear.app/dummy/issue/ENG-1/wrong",
        ],
        None,
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.starts_with("✗ Failed to list issues: "), "{error}");
    assert!(error.contains("cycle URL, number, or name"), "{error}");
    let requests = server.finish();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["operationName"], "ResolveTeam");
    assert_eq!(
        requests[0]["variables"],
        json!({"reference":"ENG","id":null,"isUuid":false})
    );
}
