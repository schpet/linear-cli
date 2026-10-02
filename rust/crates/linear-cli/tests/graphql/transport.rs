//! Loopback transport tests.
//!
//! Two kinds of server stand in for Linear:
//! - [`ScriptedServer`] answers a fixed list of GraphQL exchanges, checks each
//!   request's operation, variables and identity headers, and reports any
//!   mismatch or unexpected extra request;
//! - [`LocalServer`] is a raw listener for stalls, oversized and chunked
//!   bodies and cancellation.

use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use cynic::{MutationBuilder, QueryBuilder};
use linear_cli::graphql::edit::Edit;
use linear_cli::graphql::envelope::{
    GraphQlRequest, ResponseError, graphql_message, require_entity, require_success,
};
use linear_cli::graphql::operations::issue_update::{
    IssueUpdateInput, UpdateIssue, UpdateIssueVariables,
};
use linear_cli::graphql::operations::teams::{GetTeams, GetTeamsVariables};
use linear_cli::graphql::pagination::{Page, paginate};
use linear_cli::graphql::transport::{
    ApiKey, ApiKeyError, CONTENT_TYPE_VALUE, ConfigError, Deadline, EndpointUrl, EndpointUrlError,
    GraphQlTransport, HttpBodyShape, NetworkPhase, RawHttpResponse, ResponseCap,
    TransportBuildError, TransportConfig, TransportFailure, USER_AGENT_VALUE, classify_typed,
};
use reqwest::StatusCode;
use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::{Map, Value, json};

const FAKE_KEY: &str = "lin_api_fake";

fn tls_fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/tls")
        .join(name)
}

fn fake_key() -> ApiKey {
    ApiKey::new(FAKE_KEY.to_owned()).expect("fake key")
}

fn config(deadline: Duration, cap: usize) -> TransportConfig {
    TransportConfig {
        ca_bundle: None,
        deadline: Deadline::new(deadline).expect("deadline"),
        max_response_bytes: ResponseCap::new(cap).expect("cap"),
    }
}

fn transport_for(endpoint: &str, config: TransportConfig) -> GraphQlTransport {
    GraphQlTransport::new(
        EndpointUrl::parse(endpoint).expect("endpoint"),
        fake_key(),
        config,
    )
    .expect("transport")
}

fn teams_request(first: Option<i32>, after: Option<&str>) -> GraphQlRequest<GetTeamsVariables> {
    GraphQlRequest::with_variables(GetTeams::build(GetTeamsVariables {
        filter: None,
        first,
        after: after.map(str::to_owned),
    }))
}

fn update_request(title: &str) -> GraphQlRequest<UpdateIssueVariables> {
    GraphQlRequest::with_variables(UpdateIssue::build(UpdateIssueVariables {
        id: "issue-1".to_owned(),
        input: IssueUpdateInput {
            title: Edit::Set(title.to_owned()),
            ..IssueUpdateInput::default()
        },
    }))
}

// ---------------------------------------------------------------------------
// Raw local server

#[derive(Clone, Copy)]
enum Behavior {
    /// Complete response with `Content-Length`, then close.
    Respond {
        status: &'static str,
        body_len: usize,
    },
    /// Headers plus a body prefix, then hold the connection open.
    PartialBodyThenStall,
    /// Read the request and never answer.
    NeverRespond,
    /// Chunked body that never ends.
    ChunkedForever,
}

#[derive(Debug, Default)]
struct Observed {
    request_head: String,
    client_closed: bool,
    accepted: bool,
}

struct LocalServer {
    port: u16,
    stop: mpsc::Sender<()>,
    handle: JoinHandle<Observed>,
}

impl LocalServer {
    fn start(behavior: Behavior) -> LocalServer {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        listener.set_nonblocking(true).expect("nonblocking");
        let port = listener.local_addr().expect("addr").port();
        let (stop, stopped) = mpsc::channel::<()>();
        let handle = thread::spawn(move || serve_one(&listener, behavior, &stopped));
        LocalServer { port, stop, handle }
    }

    fn endpoint(&self) -> String {
        format!("http://127.0.0.1:{}/graphql", self.port)
    }

    fn stop(self) -> Observed {
        let _ = self.stop.send(());
        self.handle.join().expect("server thread")
    }

    fn finished(&self) -> bool {
        self.handle.is_finished()
    }
}

fn should_stop(stopped: &mpsc::Receiver<()>) -> bool {
    !matches!(
        stopped.recv_timeout(Duration::from_millis(20)),
        Err(RecvTimeoutError::Timeout)
    )
}

fn serve_one(listener: &TcpListener, behavior: Behavior, stopped: &mpsc::Receiver<()>) -> Observed {
    let mut observed = Observed::default();
    let stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                if should_stop(stopped) {
                    return observed;
                }
            }
            Err(error) => panic!("accept: {error}"),
        }
    };
    observed.accepted = true;
    stream.set_nonblocking(false).expect("blocking stream");
    stream
        .set_read_timeout(Some(Duration::from_millis(20)))
        .expect("read timeout");
    observed.request_head = read_request(&stream);
    let mut stream = &stream;
    match behavior {
        Behavior::Respond { status, body_len } => {
            let body = vec![b'x'; body_len];
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .expect("head");
            stream.write_all(&body).expect("body");
            stream.flush().expect("flush");
            return observed;
        }
        Behavior::PartialBodyThenStall => {
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 1000\r\n\r\n{{\"data\":"
            )
            .expect("partial");
            stream.flush().expect("flush");
        }
        Behavior::NeverRespond => {}
        Behavior::ChunkedForever => {
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n"
            )
            .expect("chunked head");
            let chunk = vec![b'y'; 512];
            loop {
                if should_stop(stopped) {
                    return observed;
                }
                let written = write!(stream, "{:x}\r\n", chunk.len())
                    .and_then(|()| stream.write_all(&chunk))
                    .and_then(|()| stream.write_all(b"\r\n"))
                    .and_then(|()| stream.flush());
                if written.is_err() {
                    observed.client_closed = true;
                    return observed;
                }
            }
        }
    }
    // Hold the connection open until the client closes it or the test stops us.
    let mut sink = [0_u8; 64];
    loop {
        if should_stop(stopped) {
            return observed;
        }
        match stream.read(&mut sink) {
            Ok(0) => {
                observed.client_closed = true;
                return observed;
            }
            Ok(_) => {}
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(_) => {
                observed.client_closed = true;
                return observed;
            }
        }
    }
}

/// Reads the request head and its `Content-Length` body from a blocking stream.
fn read_request(mut stream: &TcpStream) -> String {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 1024];
    let started = Instant::now();
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => buffer.extend_from_slice(chunk.get(..n).expect("chunk slice")),
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(error) => panic!("read request: {error}"),
        }
        let text = String::from_utf8_lossy(&buffer);
        if let Some(head_end) = text.find("\r\n\r\n") {
            let head = &text[..head_end];
            let length = head
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);
            if buffer.len() >= head_end + 4 + length {
                break;
            }
        }
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "request never completed"
        );
    }
    String::from_utf8_lossy(&buffer).into_owned()
}

// ---------------------------------------------------------------------------
// Value types

#[test]
fn endpoint_url_keeps_path_and_query_but_displays_only_the_origin() {
    let endpoint = EndpointUrl::parse("https://api.linear.app/graphql?sig=SECRET#frag-less")
        .expect_err("fragment rejected");
    assert_eq!(endpoint, EndpointUrlError::Fragment);
    let endpoint = EndpointUrl::parse("https://api.linear.app/graphql?sig=SECRET").expect("ok");
    assert_eq!(
        endpoint.url().as_str(),
        "https://api.linear.app/graphql?sig=SECRET"
    );
    assert_eq!(endpoint.origin(), "https://api.linear.app");
    assert_eq!(endpoint.to_string(), "https://api.linear.app");
    assert_eq!(
        format!("{endpoint:?}"),
        "EndpointUrl(\"https://api.linear.app\")"
    );
    let loopback = EndpointUrl::parse("http://127.0.0.1:4321/graphql").expect("ok");
    assert_eq!(loopback.to_string(), "http://127.0.0.1:4321");
    assert_eq!(
        EndpointUrl::parse("ftp://api.linear.app/graphql").expect_err("scheme"),
        EndpointUrlError::Scheme("ftp".to_owned())
    );
    assert_eq!(
        EndpointUrl::parse("https://user:pw@api.linear.app/graphql").expect_err("credentials"),
        EndpointUrlError::Credentials
    );
    assert!(matches!(
        EndpointUrl::parse("not a url").expect_err("invalid"),
        EndpointUrlError::Invalid(_)
    ));
}

#[test]
fn api_key_accepts_visible_ascii_with_spaces_and_redacts_itself() {
    let key = ApiKey::new("lin_api_abc DEF!~".to_owned()).expect("spaces allowed");
    assert_eq!(format!("{key:?}"), "ApiKey(<redacted>)");
    assert_eq!(key.to_string(), "<redacted>");
    assert_eq!(
        ApiKey::new(String::new()).expect_err("empty"),
        ApiKeyError::Empty
    );
    for (text, index) in [
        ("lin_api\r\nX-Injected: 1", 7),
        ("lin_api\tx", 7),
        ("\u{1b}[31m", 0),
        ("lin_api_é", 8),
        ("lin_api\u{7f}", 7),
    ] {
        assert_eq!(
            ApiKey::new(text.to_owned()).expect_err("rejected"),
            ApiKeyError::InvalidByte { index },
            "{text:?}"
        );
    }
}

#[test]
fn config_values_are_nonzero() {
    assert_eq!(
        Deadline::new(Duration::ZERO).expect_err("zero"),
        ConfigError::ZeroDeadline
    );
    assert_eq!(Deadline::DEFAULT.duration(), Duration::from_secs(30));
    assert_eq!(ResponseCap::DEFAULT.bytes(), 64 * 1024 * 1024);
    assert_eq!(
        ResponseCap::new(0).expect_err("zero"),
        ConfigError::ZeroResponseCap
    );
    assert_eq!(TransportConfig::default().ca_bundle, None);
}

#[test]
fn ca_bundle_must_be_a_readable_pem_file_with_certificates() {
    let dir = std::env::temp_dir().join(format!("linear-ca-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let endpoint = EndpointUrl::parse("https://uploads.linear.app/x").expect("endpoint");
    let build = |path: PathBuf| {
        GraphQlTransport::new(
            endpoint.clone(),
            fake_key(),
            TransportConfig {
                ca_bundle: Some(path),
                ..TransportConfig::default()
            },
        )
    };
    build(tls_fixture("test-ca.pem")).expect("valid bundle");
    assert!(matches!(
        build(dir.join("missing.pem")).expect_err("missing"),
        TransportBuildError::CaRead { .. }
    ));
    assert!(matches!(
        build(dir.clone()).expect_err("directory"),
        TransportBuildError::CaRead { .. }
    ));
    for (name, contents) in [("empty.pem", " \n"), ("garbage.pem", "not a certificate")] {
        let path = dir.join(name);
        std::fs::write(&path, contents).expect("write");
        assert!(matches!(
            build(path).expect_err(name),
            TransportBuildError::CaEmpty { .. }
        ));
    }
    let bogus = dir.join("bogus.pem");
    std::fs::write(
        &bogus,
        "-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----\n",
    )
    .expect("write");
    let error = build(bogus).expect_err("bad DER");
    assert!(
        matches!(error, TransportBuildError::CaInvalid { .. }),
        "{error:?}"
    );
    let app = linear_cli::error::Error::from(error);
    assert!(
        app.to_string().starts_with("SSL_CERT_FILE: CA bundle "),
        "{app}"
    );
    std::fs::remove_dir_all(&dir).expect("cleanup");
}

// ---------------------------------------------------------------------------
// Local-server contracts

#[tokio::test(flavor = "current_thread")]
async fn failures_never_expose_query_tokens_or_the_api_key() {
    // Nothing listens on port 1, so the connection is refused.
    let port = 1;
    let endpoint = EndpointUrl::parse(&format!(
        "http://127.0.0.1:{port}/graphql?signature=SIGNED-SECRET-TOKEN"
    ))
    .expect("endpoint");
    let key = ApiKey::new("lin_api_SECRET_KEY_VALUE".to_owned()).expect("key");
    let transport = GraphQlTransport::new(endpoint, key, config(Duration::from_secs(5), 1024))
        .expect("transport");
    assert!(!format!("{transport:?}").contains("SECRET"));
    let failure = transport
        .send_raw("{ viewer { id } }", None, None)
        .await
        .expect_err("connection refused");
    let TransportFailure::Network { origin, phase, .. } = &failure else {
        panic!("expected Network, got {failure:?}");
    };
    assert_eq!(origin, &format!("http://127.0.0.1:{port}"));
    // The listener is dropped before sending, so the fixture cannot guarantee
    // whether reqwest reports a connection or request failure. Keep both
    // origin-only diagnostics under the same complete redaction checks.
    let action = match phase {
        NetworkPhase::Connect => "connection to",
        NetworkPhase::Request => "request to",
        NetworkPhase::Body | NetworkPhase::Other => {
            panic!("expected connect/request failure, got {failure:?}");
        }
    };
    let display = failure.to_string();
    assert!(
        display.starts_with(&format!("{action} http://127.0.0.1:{port} failed: ")),
        "{display}"
    );
    let debug = format!("{failure:?}");
    let mut chain = Vec::new();
    let mut source: Option<&(dyn std::error::Error + 'static)> =
        std::error::Error::source(&failure);
    while let Some(error) = source {
        chain.push(error.to_string());
        source = error.source();
    }
    assert!(
        chain.len() >= 2,
        "reqwest error and its io source: {chain:?}"
    );
    for text in [display, debug].iter().chain(chain.iter()) {
        assert!(!text.contains("SIGNED-SECRET-TOKEN"), "{text}");
        assert!(!text.contains("signature"), "{text}");
        assert!(!text.contains("SECRET_KEY"), "{text}");
        assert!(!text.contains("/graphql"), "{text}");
    }
    let app: linear_cli::error::Error = failure.into();
    assert!(std::error::Error::source(&app).is_some());
}

#[tokio::test(flavor = "current_thread")]
async fn request_carries_exact_headers_and_envelope_bytes() {
    let server = LocalServer::start(Behavior::Respond {
        status: "200 OK",
        body_len: 0,
    });
    let transport = transport_for(&server.endpoint(), config(Duration::from_secs(5), 1024));
    let mut variables = Map::new();
    variables.insert("after".to_owned(), Value::Null);
    transport
        .send_raw("query($after: String) { x }", Some(variables), Some("Q"))
        .await
        .expect("empty 200");
    let head = server.stop().request_head;
    let (headers, body) = head.split_once("\r\n\r\n").expect("head and body");
    assert!(headers.starts_with("POST /graphql HTTP/1.1\r\n"));
    let header_lines: Vec<&str> = headers.lines().skip(1).collect();
    assert!(
        header_lines.contains(&format!("authorization: {FAKE_KEY}").as_str()),
        "{headers}"
    );
    assert!(
        header_lines.contains(&format!("user-agent: {USER_AGENT_VALUE}").as_str()),
        "{headers}"
    );
    assert!(
        header_lines.contains(&format!("content-type: {CONTENT_TYPE_VALUE}").as_str()),
        "{headers}"
    );
    assert_eq!(USER_AGENT_VALUE, "schpet-linear-cli/3.0.0-alpha.1");
    assert_eq!(
        header_lines
            .iter()
            .filter(|line| line.starts_with("authorization:"))
            .count(),
        1
    );
    assert!(!headers.to_ascii_lowercase().contains("referer"));
    assert_eq!(
        body,
        r#"{"query":"query($after: String) { x }","variables":{"after":null},"operationName":"Q"}"#
    );
}

#[tokio::test(flavor = "current_thread")]
async fn declared_oversized_body_is_rejected_before_reading() {
    let server = LocalServer::start(Behavior::Respond {
        status: "200 OK",
        body_len: 4096,
    });
    let transport = transport_for(&server.endpoint(), config(Duration::from_secs(5), 1024));
    let failure = transport
        .send_raw("{ x }", None, None)
        .await
        .expect_err("too large");
    match &failure {
        TransportFailure::ResponseTooLarge { status, limit } => {
            assert_eq!(status.as_u16(), 200);
            assert_eq!(limit.bytes(), 1024);
        }
        other => panic!("expected ResponseTooLarge, got {other:?}"),
    }
    assert_eq!(
        failure.to_string(),
        "response body exceeds the 1024 byte limit (HTTP 200 OK)"
    );
    server.stop();
}

#[tokio::test(flavor = "current_thread")]
async fn body_exactly_at_the_cap_is_kept_intact() {
    let server = LocalServer::start(Behavior::Respond {
        status: "502 Bad Gateway",
        body_len: 1024,
    });
    let transport = transport_for(&server.endpoint(), config(Duration::from_secs(5), 1024));
    let response = transport
        .send_raw("{ x }", None, None)
        .await
        .expect("at cap");
    assert_eq!(response.status.as_u16(), 502);
    assert_eq!(response.body.len(), 1024);
    assert!(response.body.iter().all(|byte| *byte == b'x'));
    assert_eq!(
        format!("{response:?}"),
        "RawHttpResponse { status: 502, headers: <3 headers>, body: <1024 bytes> }"
    );
    server.stop();
}

/// Response headers can carry session cookies or trace ids; neither a
/// `GraphQl` nor an `Http` failure may print them through `Debug`.
#[test]
fn failure_debug_prints_header_counts_not_header_values() {
    let secret_headers = || {
        let mut headers = HeaderMap::new();
        headers.insert("content-type", HeaderValue::from_static("application/json"));
        headers.insert(
            "set-cookie",
            HeaderValue::from_static("session=COOKIE-SECRET-VALUE; HttpOnly"),
        );
        headers.insert(
            "x-linear-trace",
            HeaderValue::from_static("TRACE-SECRET-VALUE"),
        );
        headers
    };
    let leaks = [
        "COOKIE-SECRET-VALUE",
        "TRACE-SECRET-VALUE",
        "HttpOnly",
        "set-cookie",
        "x-linear-trace",
    ];

    let failure = classify_typed::<Value>(RawHttpResponse {
        status: StatusCode::UNAUTHORIZED,
        headers: secret_headers(),
        body: br#"{"errors":[{"message":"Authentication required"}]}"#.to_vec(),
    })
    .expect_err("errors array");
    let debug = format!("{failure:?}");
    assert!(
        matches!(&failure, TransportFailure::GraphQl { headers, .. } if headers.len() == 3),
        "{debug}"
    );
    assert!(
        debug.starts_with("GraphQl { status: 401, headers: <3 headers>, errors: ["),
        "{debug}"
    );
    assert!(debug.contains("Authentication required"), "{debug}");
    assert!(debug.ends_with("partial_data: false }"), "{debug}");
    for leak in leaks {
        assert!(!debug.contains(leak), "{leak} leaked into {debug}");
    }

    let failure = classify_typed::<Value>(RawHttpResponse {
        status: StatusCode::BAD_GATEWAY,
        headers: secret_headers(),
        body: b"<html>".to_vec(),
    })
    .expect_err("non-2xx without errors");
    let debug = format!("{failure:?}");
    assert!(
        matches!(
            &failure,
            TransportFailure::Http {
                body: HttpBodyShape::Unusable(_),
                ..
            }
        ),
        "{debug}"
    );
    assert!(
        debug.starts_with(
            "Http { response: RawHttpResponse { status: 502, headers: <3 headers>, body: <6 bytes> }, body: Unusable("
        ),
        "{debug}"
    );
    for leak in leaks {
        assert!(!debug.contains(leak), "{leak} leaked into {debug}");
    }
}

#[tokio::test(flavor = "current_thread")]
async fn unbounded_chunked_body_stops_at_the_cap() {
    let server = LocalServer::start(Behavior::ChunkedForever);
    let transport = transport_for(&server.endpoint(), config(Duration::from_secs(5), 2048));
    let started = Instant::now();
    let failure = transport
        .send_raw("{ x }", None, None)
        .await
        .expect_err("too large");
    assert!(
        matches!(failure, TransportFailure::ResponseTooLarge { .. }),
        "{failure:?}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "stopped before the deadline"
    );
    drop(transport);
    let observed = server.stop();
    assert!(observed.accepted);
}

#[tokio::test(flavor = "current_thread")]
async fn silent_server_hits_the_total_deadline() {
    let server = LocalServer::start(Behavior::NeverRespond);
    let transport = transport_for(&server.endpoint(), config(Duration::from_millis(300), 1024));
    let started = Instant::now();
    let failure = transport
        .send_raw("{ x }", None, None)
        .await
        .expect_err("timeout");
    let elapsed = started.elapsed();
    assert!(
        matches!(failure, TransportFailure::Timeout { .. }),
        "{failure:?}"
    );
    assert!(
        elapsed >= Duration::from_millis(250) && elapsed < Duration::from_secs(3),
        "{elapsed:?}"
    );
    assert_eq!(
        failure.to_string(),
        format!(
            "request to http://127.0.0.1:{} did not complete within 300ms",
            server.port
        )
    );
    let observed = server.stop();
    assert!(observed.accepted);
}

#[tokio::test(flavor = "current_thread")]
async fn stalled_body_hits_the_total_deadline_without_partial_data() {
    let server = LocalServer::start(Behavior::PartialBodyThenStall);
    let transport = transport_for(&server.endpoint(), config(Duration::from_millis(300), 4096));
    let started = Instant::now();
    let failure = transport
        .send_raw("{ x }", None, None)
        .await
        .expect_err("timeout");
    assert!(
        matches!(failure, TransportFailure::Timeout { .. }),
        "{failure:?}"
    );
    assert!(started.elapsed() < Duration::from_secs(3));
    let observed = server.stop();
    assert!(observed.accepted);
}

#[test]
fn cancelled_request_completes_promptly_and_releases_the_connection() {
    let server = LocalServer::start(Behavior::PartialBodyThenStall);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let transport = transport_for(&server.endpoint(), config(Duration::from_secs(30), 4096));
    let started = Instant::now();
    let released_while_running = runtime.block_on(async {
        let cancelled = tokio::time::timeout(
            Duration::from_millis(150),
            transport.send_raw("{ x }", None, None),
        )
        .await;
        assert!(
            cancelled.is_err(),
            "outer cancellation wins over the 30 s deadline"
        );
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "cancellation returned promptly"
        );
        drop(transport);
        // A current-thread runtime only polls the connection task while it is
        // running, so wait for the release here, under a hard deadline.
        let window = Instant::now();
        while window.elapsed() < Duration::from_secs(2) && !server.finished() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        server.finished()
    });
    drop(runtime);
    let released =
        released_while_running || wait_until(Duration::from_secs(2), || server.finished());
    let observed = server.stop();
    assert!(observed.accepted);
    assert!(
        released,
        "server saw the client close within the hard deadline"
    );
    assert!(observed.client_closed, "{observed:?}");
    println!(
        "cancellation: connection released while the runtime was running: {released_while_running}"
    );
}

fn wait_until(limit: Duration, mut condition: impl FnMut() -> bool) -> bool {
    let started = Instant::now();
    while started.elapsed() < limit {
        if condition() {
            return true;
        }
        thread::sleep(Duration::from_millis(10));
    }
    condition()
}

// ---------------------------------------------------------------------------
// Scripted GraphQL server

/// One scripted exchange: the request it expects and the response it sends.
struct Step {
    path: &'static str,
    operation: Option<&'static str>,
    variables: Option<Value>,
    status: u16,
    headers: Vec<(&'static str, String)>,
    body: String,
}

impl Step {
    /// A JSON response to a `GetTeams` request with these variables.
    fn teams(variables: Value, status: u16, body: Value) -> Self {
        Self::json(Some("GetTeams"), Some(variables), status, body)
    }

    fn json(
        operation: Option<&'static str>,
        variables: Option<Value>,
        status: u16,
        body: Value,
    ) -> Self {
        Self {
            path: "/graphql",
            operation,
            variables,
            status,
            headers: vec![("content-type", "application/json".to_owned())],
            body: body.to_string(),
        }
    }

    /// A `GetTeams` exchange whose response body is sent verbatim.
    fn teams_raw(status: u16, content_type: &str, body: &str) -> Self {
        Self {
            path: "/graphql",
            operation: Some("GetTeams"),
            variables: Some(json!({"first": 100})),
            status,
            headers: vec![("content-type", content_type.to_owned())],
            body: body.to_owned(),
        }
    }

    fn with_header(mut self, name: &'static str, value: &str) -> Self {
        self.headers.push((name, value.to_owned()));
        self
    }

    fn at(mut self, path: &'static str) -> Self {
        self.path = path;
        self
    }

    fn problems(&self, request: &RecordedRequest) -> Vec<String> {
        let mut problems = Vec::new();
        let mut expect = |what: &str, ok: bool| {
            if !ok {
                problems.push(format!("{what} differs"));
            }
        };
        expect("method", request.method == "POST");
        expect("path", request.path == self.path);
        expect(
            "authorization",
            request.header("authorization") == Some(FAKE_KEY),
        );
        expect(
            "user-agent",
            request.header("user-agent") == Some(USER_AGENT_VALUE),
        );
        expect(
            "content-type",
            request.header("content-type") == Some(CONTENT_TYPE_VALUE),
        );
        let body = request.body.as_ref();
        expect(
            "operation name",
            body.and_then(|body| body.get("operationName"))
                .and_then(Value::as_str)
                == self.operation,
        );
        expect(
            "variables",
            body.and_then(|body| body.get("variables")) == self.variables.as_ref(),
        );
        problems
    }
}

#[derive(Debug)]
struct RecordedRequest {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Option<Value>,
}

impl RecordedRequest {
    fn parse(raw: &str) -> Self {
        let (head, body) = raw.split_once("\r\n\r\n").expect("request head");
        let mut lines = head.lines();
        let mut start = lines.next().expect("request line").split(' ');
        let method = start.next().expect("method").to_owned();
        let path = start.next().expect("path").to_owned();
        let headers = lines
            .map(|line| {
                let (name, value) = line.split_once(':').expect("header line");
                (name.to_ascii_lowercase(), value.trim().to_owned())
            })
            .collect();
        let body = (!body.is_empty()).then(|| serde_json::from_str(body).expect("JSON body"));
        Self {
            method,
            path,
            headers,
            body,
        }
    }

    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

#[derive(Debug, Default)]
struct Report {
    requests: Vec<RecordedRequest>,
    consumed: usize,
    remaining: usize,
    unexpected: usize,
    mismatches: Vec<String>,
}

impl Report {
    fn assert_clean(&self) {
        assert!(
            self.mismatches.is_empty() && self.unexpected == 0 && self.remaining == 0,
            "{self:#?}"
        );
    }
}

struct ScriptedServer {
    port: u16,
    stop: mpsc::Sender<()>,
    handle: JoinHandle<Report>,
}

impl ScriptedServer {
    fn start(steps: Vec<Step>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        listener.set_nonblocking(true).expect("nonblocking");
        let port = listener.local_addr().expect("addr").port();
        let (stop, stopped) = mpsc::channel::<()>();
        let handle = thread::spawn(move || serve_script(&listener, steps, &stopped));
        Self { port, stop, handle }
    }

    fn transport(&self) -> GraphQlTransport {
        transport_for(
            &format!("http://127.0.0.1:{}/graphql", self.port),
            config(Duration::from_secs(10), 1024 * 1024),
        )
    }

    fn finish(self) -> Report {
        let _ = self.stop.send(());
        self.handle.join().expect("scripted server thread")
    }
}

fn serve_script(listener: &TcpListener, steps: Vec<Step>, stopped: &mpsc::Receiver<()>) -> Report {
    let mut steps = std::collections::VecDeque::from(steps);
    let mut report = Report::default();
    loop {
        let stream = match listener.accept() {
            Ok((stream, _)) => stream,
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                if should_stop(stopped) {
                    report.remaining = steps.len();
                    return report;
                }
                continue;
            }
            Err(error) => panic!("accept: {error}"),
        };
        stream.set_nonblocking(false).expect("blocking stream");
        stream
            .set_read_timeout(Some(Duration::from_millis(20)))
            .expect("read timeout");
        let request = RecordedRequest::parse(&read_request(&stream));
        let refusal = |message: &str| {
            (
                500,
                vec![("content-type", "application/json".to_owned())],
                json!({"errors": [{"message": message}]}).to_string(),
            )
        };
        let (status, headers, body) = match steps.front() {
            None => {
                report.unexpected += 1;
                refusal("unexpected request")
            }
            Some(step) => {
                let problems = step.problems(&request);
                if problems.is_empty() {
                    let step = steps.pop_front().expect("front step");
                    report.consumed += 1;
                    (step.status, step.headers, step.body)
                } else {
                    report.mismatches.extend(problems);
                    refusal("fixture mismatch")
                }
            }
        };
        report.requests.push(request);
        let mut stream = &stream;
        let reason = StatusCode::from_u16(status)
            .ok()
            .and_then(|status| status.canonical_reason())
            .unwrap_or("Unknown");
        write!(stream, "HTTP/1.1 {status} {reason}\r\n").expect("status line");
        for (name, value) in &headers {
            write!(stream, "{name}: {value}\r\n").expect("header");
        }
        write!(
            stream,
            "Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .expect("body");
        stream.flush().expect("flush");
    }
}

fn team(id: &str, name: &str, key: &str, description: Option<&str>, archived: bool) -> Value {
    json!({
        "id": id,
        "name": name,
        "key": key,
        "description": description,
        "icon": null,
        "color": "#0000ff",
        "cyclesEnabled": false,
        "createdAt": "2026-01-01T00:00:00.000Z",
        "updatedAt": "2026-01-02T00:00:00.000Z",
        "archivedAt": archived.then_some("2026-02-01T00:00:00.000Z"),
        "organization": {"id": "org-1", "name": "Acme"}
    })
}

fn teams_page(nodes: Vec<Value>, end_cursor: Option<&str>, has_next_page: bool) -> Value {
    json!({"teams": {
        "nodes": nodes,
        "pageInfo": {"hasNextPage": has_next_page, "endCursor": end_cursor}
    }})
}

fn two_team_pages() -> Vec<Step> {
    vec![
        Step::teams(
            json!({"first": 100}),
            200,
            json!({"data": teams_page(
                vec![
                    team("team-1", "Engineering", "ENG", None, false),
                    team("team-2", "Design", "DES", Some("Product design"), false),
                ],
                Some("cursor-a"),
                true,
            )}),
        ),
        Step::teams(
            json!({"first": 100, "after": "cursor-a"}),
            200,
            json!({"data": teams_page(
                vec![team("team-3", "Archived", "ARC", None, true)],
                Some("cursor-b"),
                false,
            )}),
        ),
    ]
}

#[tokio::test(flavor = "current_thread")]
async fn two_pages_paginate_with_cursor_variables() {
    let server = ScriptedServer::start(two_team_pages());
    let transport = server.transport();
    let result = paginate(|after| {
        let transport = &transport;
        async move {
            let request = teams_request(Some(100), after.as_deref());
            let data: GetTeams = transport.execute(&request).await?;
            Ok::<_, TransportFailure>(Page {
                nodes: data.teams.nodes,
                page_info: data.teams.page_info.into(),
            })
        }
    })
    .await
    .expect("two pages");
    let names: Vec<&str> = result.nodes.iter().map(|team| team.name.as_str()).collect();
    assert_eq!(names, ["Engineering", "Design", "Archived"]);
    assert_eq!(
        result.nodes[1].description.as_deref(),
        Some("Product design")
    );
    assert!(result.nodes[2].archived_at.is_some());
    assert!(!result.page_info.has_next_page);
    assert_eq!(result.page_info.end_cursor.as_deref(), Some("cursor-b"));
    let report = server.finish();
    report.assert_clean();
    assert_eq!(report.consumed, 2);
}

#[tokio::test(flavor = "current_thread")]
async fn wrong_variables_are_reported_by_the_server() {
    let server = ScriptedServer::start(two_team_pages());
    let failure = server
        .transport()
        .execute::<GetTeams, _>(&teams_request(Some(50), None))
        .await
        .expect_err("mismatch");
    match &failure {
        TransportFailure::GraphQl {
            status,
            errors,
            partial_data,
            ..
        } => {
            assert_eq!(status.as_u16(), 500);
            assert_eq!(graphql_message(errors).as_deref(), Some("fixture mismatch"));
            assert!(!partial_data);
        }
        other => panic!("expected GraphQl, got {other:?}"),
    }
    let report = server.finish();
    assert_eq!(report.mismatches, ["variables differs"]);
    assert_eq!(report.consumed, 0);
    assert_eq!(report.remaining, 2);
}

#[tokio::test(flavor = "current_thread")]
async fn graphql_errors_classify_ahead_of_http_status() {
    let internal = json!({
        "message": "Something went wrong",
        "extensions": {
            "type": "internal error",
            "code": "INTERNAL_ERROR",
            "userPresentableMessage": "Something went wrong. Please try again."
        }
    });
    let partial = teams_page(
        vec![team("team-1", "Engineering", "ENG", None, false)],
        None,
        false,
    );
    let unauthenticated = r#"{"errors":[{"message":"Authentication required, not authenticated","extensions":{"type":"authentication error","code":"AUTHENTICATION_ERROR"}}]}"#;
    let server = ScriptedServer::start(vec![
        Step::teams(
            json!({"first": 100}),
            200,
            json!({"data": null, "errors": [internal]}),
        ),
        Step::teams(
            json!({"first": 100}),
            200,
            json!({"data": partial, "errors": [internal]}),
        ),
        Step::teams(
            json!({"first": 100}),
            400,
            json!({"data": null, "errors": [{
                "message": "Argument Validation Error",
                "path": ["teams"],
                "locations": [{"line": 2, "column": 3}],
                "extensions": {
                    "type": "invalid input",
                    "code": "INVALID_INPUT",
                    "userPresentableMessage": "The request was invalid."
                }
            }]}),
        ),
        Step::teams_raw(401, "application/json", unauthenticated),
        Step::json(
            None,
            None,
            400,
            json!({"errors": [{
                "message": "Cannot query field \"definitelyMissing\" on type \"User\".",
                "extensions": {"code": "GRAPHQL_VALIDATION_FAILED"}
            }]}),
        ),
    ]);
    let transport = server.transport();
    let page_one = teams_request(Some(100), None);

    let failure = transport
        .execute::<GetTeams, _>(&page_one)
        .await
        .expect_err("errors only");
    let TransportFailure::GraphQl {
        status,
        errors,
        partial_data,
        ..
    } = &failure
    else {
        panic!("{failure:?}");
    };
    assert_eq!(status.as_u16(), 200);
    assert!(!partial_data);
    assert_eq!(errors[0].message, "Something went wrong");
    assert_eq!(
        failure.to_string(),
        "Something went wrong. Please try again."
    );

    let failure = transport
        .execute::<GetTeams, _>(&page_one)
        .await
        .expect_err("partial data");
    assert!(
        matches!(
            &failure,
            TransportFailure::GraphQl { status, partial_data: true, .. } if status.as_u16() == 200
        ),
        "partial data is never returned as success: {failure:?}"
    );

    let failure = transport
        .execute::<GetTeams, _>(&page_one)
        .await
        .expect_err("400 errors");
    let TransportFailure::GraphQl {
        status,
        errors,
        headers,
        ..
    } = &failure
    else {
        panic!("{failure:?}");
    };
    assert_eq!(status.as_u16(), 400);
    assert_eq!(
        headers.get("content-type").and_then(|v| v.to_str().ok()),
        Some("application/json")
    );
    assert_eq!(errors[0].path.as_ref().map(|path| path.len()), Some(1));
    assert!(errors[0].locations.is_some());
    assert_eq!(failure.to_string(), "The request was invalid.");

    let raw = transport
        .send_request(&page_one)
        .await
        .expect("401 bytes captured");
    assert_eq!(raw.status.as_u16(), 401);
    assert_eq!(raw.body, unauthenticated.as_bytes());
    let failure = classify_typed::<GetTeams>(raw).expect_err("401 with errors");
    let TransportFailure::GraphQl { status, errors, .. } = &failure else {
        panic!("{failure:?}");
    };
    assert_eq!(status.as_u16(), 401);
    assert_eq!(
        errors[0].message,
        "Authentication required, not authenticated"
    );

    let raw = transport
        .send_raw("{ viewer { definitelyMissing } }", None, None)
        .await
        .expect("400 validation bytes");
    let failure = classify_typed::<Value>(raw).expect_err("validation errors");
    let TransportFailure::GraphQl { status, errors, .. } = &failure else {
        panic!("{failure:?}");
    };
    assert_eq!(status.as_u16(), 400);
    assert_eq!(
        errors[0].extensions.as_ref().and_then(|e| e.get("code")),
        Some(&Value::from("GRAPHQL_VALIDATION_FAILED"))
    );

    let report = server.finish();
    report.assert_clean();
    assert_eq!(report.consumed, 5);
}

#[tokio::test(flavor = "current_thread")]
async fn http_failures_keep_raw_bytes_and_bad_bodies_classify_separately() {
    let data = json!({"data": teams_page(
        vec![team("team-1", "Engineering", "ENG", None, false)],
        None,
        false,
    )})
    .to_string();
    let server = ScriptedServer::start(vec![
        Step::teams_raw(429, "text/plain", "Too Many Requests\n").with_header("retry-after", "7"),
        Step::teams_raw(502, "text/html", "<html><body>bad gateway</body></html>"),
        Step::teams_raw(500, "application/json", &data),
        Step::teams_raw(200, "text/html", "<html>maintenance</html>"),
        Step::teams_raw(200, "application/json", r#"{"data":"#),
        Step::teams_raw(200, "application/json", ""),
        Step::teams_raw(200, "application/json", r#"{"data":null}"#),
        Step::teams_raw(200, "application/json", r#"{"data":{"teams":"nope"}}"#),
    ]);
    let transport = server.transport();
    let page_one = teams_request(Some(100), None);
    let next = || transport.execute::<GetTeams, _>(&page_one);

    let failure = next().await.expect_err("429");
    let TransportFailure::Http { response, body } = &failure else {
        panic!("{failure:?}");
    };
    assert_eq!(response.status.as_u16(), 429);
    assert_eq!(response.body, b"Too Many Requests\n");
    assert_eq!(
        response
            .headers
            .get("retry-after")
            .and_then(|v| v.to_str().ok()),
        Some("7")
    );
    assert!(
        matches!(body, HttpBodyShape::Unusable(ResponseError::NotJson { .. })),
        "{body:?}"
    );
    assert_eq!(
        failure.to_string(),
        "unexpected HTTP status 429 Too Many Requests"
    );
    let debug = format!("{failure:?}");
    assert!(
        !debug.contains("Too Many Requests\\n"),
        "body bytes stay out of Debug: {debug}"
    );

    let failure = next().await.expect_err("502");
    let TransportFailure::Http { response, body } = &failure else {
        panic!("{failure:?}");
    };
    assert_eq!(response.status.as_u16(), 502);
    assert_eq!(response.body, b"<html><body>bad gateway</body></html>");
    assert!(matches!(
        body,
        HttpBodyShape::Unusable(ResponseError::NotJson { .. })
    ));

    let failure = next().await.expect_err("500 with data");
    assert!(
        matches!(
            &failure,
            TransportFailure::Http { response, body: HttpBodyShape::Data }
                if response.status.as_u16() == 500
        ),
        "valid data under 500 is still an HTTP failure: {failure:?}"
    );

    let failure = next().await.expect_err("HTML under 200");
    assert!(
        matches!(
            &failure,
            TransportFailure::Response(ResponseError::NotJson { .. })
        ),
        "{failure:?}"
    );
    assert_eq!(
        failure.to_string(),
        "Linear returned a non-JSON response (HTTP 200 OK, content type text/html)"
    );

    for case in ["truncated", "empty"] {
        let failure = next().await.expect_err(case);
        assert!(
            matches!(
                &failure,
                TransportFailure::Response(ResponseError::MalformedJson(_))
            ),
            "{case}: {failure:?}"
        );
    }

    let failure = next().await.expect_err("data null");
    assert!(
        matches!(
            &failure,
            TransportFailure::Response(ResponseError::MissingData)
        ),
        "{failure:?}"
    );

    let failure = next().await.expect_err("wrong shape");
    assert!(
        matches!(
            &failure,
            TransportFailure::Response(ResponseError::UnexpectedShape(_))
        ),
        "{failure:?}"
    );

    let report = server.finish();
    report.assert_clean();
    assert_eq!(report.requests.len(), 8, "no retries");
}

#[tokio::test(flavor = "current_thread")]
async fn temporary_redirect_replays_the_request_at_the_new_location() {
    let data = json!({"data": teams_page(vec![], None, false)}).to_string();
    let server = ScriptedServer::start(vec![
        Step::teams_raw(307, "text/plain", "").with_header("location", "/moved"),
        Step::teams_raw(200, "application/json", &data).at("/moved"),
    ]);
    let teams: GetTeams = server
        .transport()
        .execute(&teams_request(Some(100), None))
        .await
        .expect("redirect followed");
    assert!(teams.teams.nodes.is_empty());
    let report = server.finish();
    report.assert_clean();
    assert_eq!(report.requests[1].path, "/moved");
}

#[tokio::test(flavor = "current_thread")]
async fn mutation_payload_failures_are_typed() {
    let issue = |title: &str| {
        json!({
            "id": "issue-1",
            "identifier": "ENG-1",
            "url": "https://linear.app/acme/issue/ENG-1",
            "title": title
        })
    };
    let update = |title: &str, success: bool, issue: Value| {
        Step::json(
            Some("UpdateIssue"),
            Some(json!({"id": "issue-1", "input": {"title": title}})),
            200,
            json!({"data": {"issueUpdate": {"success": success, "issue": issue}}}),
        )
    };
    let server = ScriptedServer::start(vec![
        update("Declined title", false, issue("Old title")),
        update("New title", true, issue("New title")),
        update("Ghost", true, Value::Null),
    ]);
    let transport = server.transport();

    let declined: UpdateIssue = transport
        .execute(&update_request("Declined title"))
        .await
        .expect("success:false is not a transport failure");
    assert!(matches!(
        require_success(declined.issue_update.success),
        Err(ResponseError::MutationRejected)
    ));

    let accepted: UpdateIssue = transport
        .execute(&update_request("New title"))
        .await
        .expect("success");
    require_success(accepted.issue_update.success).expect("success");
    let issue = require_entity(accepted.issue_update.issue).expect("issue");
    assert_eq!(issue.title, "New title");

    let ghost: UpdateIssue = transport
        .execute(&update_request("Ghost"))
        .await
        .expect("success with null issue is a payload failure, not a transport one");
    assert!(matches!(
        require_entity(ghost.issue_update.issue),
        Err(ResponseError::MissingPayloadEntity)
    ));

    let report = server.finish();
    report.assert_clean();
    assert_eq!(report.consumed, 3);
}

fn viewer_step() -> Step {
    Step::json(
        None,
        None,
        200,
        json!({"data": {"viewer": {"id": "user-1"}}}),
    )
}

#[tokio::test(flavor = "current_thread")]
async fn raw_document_without_variables_returns_exact_bytes() {
    let server = ScriptedServer::start(vec![viewer_step()]);
    let response = server
        .transport()
        .send_raw("{ viewer { id } }", None, None)
        .await
        .expect("200");
    assert_eq!(response.status.as_u16(), 200);
    assert_eq!(response.body, br#"{"data":{"viewer":{"id":"user-1"}}}"#);
    let value: Value = classify_typed(response).expect("typed as Value");
    assert_eq!(value, json!({"viewer": {"id": "user-1"}}));
    let report = server.finish();
    report.assert_clean();
    assert!(report.requests[0].body.as_ref().is_some_and(|body| {
        body.get("variables").is_none() && body.get("operationName").is_none()
    }));
}

#[tokio::test(flavor = "current_thread")]
async fn extra_request_after_the_final_step_is_unexpected() {
    let server = ScriptedServer::start(vec![viewer_step()]);
    let transport = server.transport();
    transport
        .send_raw("{ viewer { id } }", None, None)
        .await
        .expect("first");
    let failure = transport
        .execute::<Value, ()>(&GraphQlRequest {
            query: "{ viewer { id } }".to_owned(),
            variables: None,
            operation_name: None,
        })
        .await
        .expect_err("second request is unscripted");
    assert!(matches!(&failure, TransportFailure::GraphQl { status, .. } if status.as_u16() == 500));
    let report = server.finish();
    assert_eq!(report.unexpected, 1);
    assert_eq!(report.consumed, 1);
    assert_eq!(report.requests.len(), 2);
}

#[tokio::test(flavor = "current_thread")]
async fn extra_variables_key_is_a_mismatch() {
    let server = ScriptedServer::start(vec![viewer_step()]);
    let mut variables = Map::new();
    variables.insert("unexpected".to_owned(), Value::from(1));
    let raw = server
        .transport()
        .send_raw("{ viewer { id } }", Some(variables), None)
        .await
        .expect("500 captured");
    assert_eq!(raw.status.as_u16(), 500);
    assert_eq!(raw.body, br#"{"errors":[{"message":"fixture mismatch"}]}"#);
    let report = server.finish();
    assert_eq!(report.mismatches, ["variables differs"]);
    assert_eq!(report.consumed, 0);
}
