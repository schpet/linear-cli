//! Loopback transport contracts (F02B Gate 1).
//!
//! Two kinds of server stand in for Linear:
//! - the P03B GraphQL fixture server, driven as a strict Deno child through
//!   `rust/parity/runner/serve-case.ts` ([`FixtureDriver`]); its final report
//!   (consumed/unexpected counts, issues and mismatches) is asserted for
//!   every case, so a fixture-generated HTTP 500 can never pass silently;
//! - a raw `std::net` listener ([`LocalServer`]) for stalls, oversized and
//!   chunked bodies, cancellation and the ambient-proxy control.
//!
//! Deno-driven tests hold [`SERIAL`] so only one fixture child runs at a time.
//! They fail, not skip, when `deno` is unavailable.

use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use cynic::{MutationBuilder, QueryBuilder};
use linear_cli::error::AppErrorKind;
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
    ApiKey, ApiKeyError, CONTENT_TYPE_VALUE, CaMode, ConfigError, Deadline, EndpointUrl,
    EndpointUrlError, GraphQlTransport, HttpBodyShape, NetworkPhase, ProxyMode, ProxyUrl,
    ProxyUrlError, RawHttpResponse, ResponseCap, TransportBuildError, TransportConfig,
    TransportFailure, USER_AGENT_VALUE, classify_typed,
};
use reqwest::StatusCode;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

const FAKE_KEY: &str = "lin_api_fake";
const READY_DEADLINE: Duration = Duration::from_secs(30);
const FINAL_DEADLINE: Duration = Duration::from_secs(30);

static SERIAL: Mutex<()> = Mutex::new(());

/// Runs one Deno-driven test body on its own current-thread runtime while
/// holding the process-wide serial lock (taken outside the runtime, so no
/// guard lives across an await).
fn run_serial(body: impl Future<Output = ()>) {
    let _guard = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(body);
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("repository root")
}

fn fake_key() -> ApiKey {
    ApiKey::new(FAKE_KEY.to_owned()).expect("fake key")
}

fn config(deadline: Duration, cap: usize) -> TransportConfig {
    TransportConfig {
        proxy: ProxyMode::Direct,
        ca: CaMode::PublicRoots,
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
// Deno fixture driver

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ReadyLine {
    event: String,
    port: u16,
    path: String,
    expected_requests: usize,
    #[serde(rename = "expectedGraphQL")]
    expected_graph_ql: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RequestSummary {
    kind: String,
    authorization_matched: bool,
    user_agent: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Mismatch {
    surface: String,
    detail: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct FinalLine {
    event: String,
    requests: Vec<RequestSummary>,
    consumed: usize,
    expected: usize,
    unexpected: usize,
    issues: Vec<String>,
    mismatches: Vec<Mismatch>,
}

#[derive(Debug)]
struct Outcome {
    report: FinalLine,
    exit_code: Option<i32>,
    stderr: String,
}

impl Outcome {
    fn assert_clean(&self) {
        assert_eq!(self.exit_code, Some(0), "driver stderr: {}", self.stderr);
        assert!(
            self.report.mismatches.is_empty(),
            "{:?}",
            self.report.mismatches
        );
        assert!(self.report.issues.is_empty(), "{:?}", self.report.issues);
        assert_eq!(self.report.unexpected, 0);
        assert_eq!(self.report.consumed, self.report.expected);
        assert_eq!(self.report.requests.len(), self.report.expected);
        for request in &self.report.requests {
            assert_eq!(request.kind, "graphql");
            assert!(request.authorization_matched, "{request:?}");
            assert_eq!(request.user_agent.as_deref(), Some(USER_AGENT_VALUE));
        }
    }

    fn assert_mismatch(&self, fragment: &str) {
        assert_eq!(self.exit_code, Some(1), "driver stderr: {}", self.stderr);
        assert!(
            self.report
                .mismatches
                .iter()
                .any(|mismatch| mismatch.surface == "fixture" && mismatch.detail.contains(fragment)),
            "no fixture mismatch containing {fragment:?} in {:?}",
            self.report.mismatches
        );
    }
}

#[derive(Debug)]
enum StartFailure {
    /// The child exited (or closed stdout) before a ready line.
    ExitedBeforeReady {
        exit_code: Option<i32>,
        stderr: String,
    },
    /// No ready line within the deadline; the child was killed and reaped.
    ReadyTimeout { stderr: String },
}

fn expect_start_failure(result: Result<FixtureDriver, StartFailure>) -> StartFailure {
    match result {
        Ok(_) => panic!("driver unexpectedly became ready"),
        Err(failure) => failure,
    }
}

/// A running `serve-case.ts` (or a stand-in program) with its protocol state.
struct FixtureDriver {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    stderr: tokio::task::JoinHandle<String>,
    ready: ReadyLine,
}

fn driver_command(case: &str) -> Command {
    let root = repo_root();
    let parity = root.join("rust/parity");
    let mut command = Command::new("deno");
    command
        .arg("run")
        .arg("--frozen")
        .arg("--cached-only")
        .arg("--no-prompt")
        .arg("--config")
        .arg(parity.join("deno.json"))
        .arg(format!(
            "--allow-read={},{}",
            parity.display(),
            root.join("graphql/schema.graphql").display()
        ))
        .arg("--allow-net=127.0.0.1")
        .arg("--allow-env=NODE_ENV")
        .arg(parity.join("runner/serve-case.ts"))
        .arg(parity.join(format!("runner/transport-cases/{case}.json")));
    command
}

/// A stand-in driver: `deno eval` of an inline script.
fn eval_command(script: &str) -> Command {
    let mut command = Command::new("deno");
    command.arg("eval").arg(script);
    command
}

impl FixtureDriver {
    async fn start(case: &str) -> FixtureDriver {
        match Self::spawn(driver_command(case)).await {
            Ok(driver) => driver,
            Err(failure) => panic!("serve-case.ts did not become ready: {failure:?}"),
        }
    }

    async fn spawn(command: Command) -> Result<FixtureDriver, StartFailure> {
        Self::spawn_with(command, READY_DEADLINE).await
    }

    async fn spawn_with(
        mut command: Command,
        ready_deadline: Duration,
    ) -> Result<FixtureDriver, StartFailure> {
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command
            .spawn()
            .expect("deno must be installed and on PATH for the transport tests");
        let stdin = child.stdin.take().expect("piped stdin");
        let mut stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
        let mut stderr_pipe = child.stderr.take().expect("piped stderr");
        let stderr = tokio::spawn(async move {
            let mut text = String::new();
            let _ = stderr_pipe.read_to_string(&mut text).await;
            text
        });
        let mut line = String::new();
        match tokio::time::timeout(ready_deadline, stdout.read_line(&mut line)).await {
            Ok(Ok(0)) | Ok(Err(_)) => {
                let exit_code = tokio::time::timeout(FINAL_DEADLINE, child.wait())
                    .await
                    .expect("child exits after closing stdout")
                    .expect("wait")
                    .code();
                let stderr = stderr.await.expect("stderr task");
                Err(StartFailure::ExitedBeforeReady { exit_code, stderr })
            }
            Ok(Ok(_)) => {
                let ready: ReadyLine =
                    serde_json::from_str(line.trim_end()).expect("ready line is strict JSON");
                assert_eq!(ready.event, "ready");
                Ok(FixtureDriver {
                    child,
                    stdin: Some(stdin),
                    stdout,
                    stderr,
                    ready,
                })
            }
            Err(_elapsed) => {
                child.kill().await.expect("kill");
                let status = child.wait().await.expect("reap");
                assert!(!status.success(), "killed child cannot report success");
                let stderr = stderr.await.expect("stderr task");
                Err(StartFailure::ReadyTimeout { stderr })
            }
        }
    }

    fn endpoint(&self) -> String {
        format!("http://127.0.0.1:{}{}", self.ready.port, self.ready.path)
    }

    fn transport(&self) -> GraphQlTransport {
        transport_for(
            &self.endpoint(),
            config(Duration::from_secs(10), ResponseCap::DEFAULT_BYTES),
        )
    }

    /// Signals completion (stdin EOF), reads the final line and reaps the child.
    async fn finish(mut self) -> Outcome {
        drop(self.stdin.take());
        let mut line = String::new();
        let read = tokio::time::timeout(FINAL_DEADLINE, self.stdout.read_line(&mut line)).await;
        let exit = match read {
            Ok(Ok(n)) if n > 0 => tokio::time::timeout(FINAL_DEADLINE, self.child.wait()).await,
            _ => {
                self.child.kill().await.expect("kill");
                let _ = self.child.wait().await;
                let stderr = self.stderr.await.expect("stderr task");
                panic!("driver produced no final line; stderr: {stderr}");
            }
        };
        let status = exit
            .expect("child exits after its final line")
            .expect("wait");
        let stderr = self.stderr.await.expect("stderr task");
        let report: FinalLine =
            serde_json::from_str(line.trim_end()).expect("final line is strict JSON");
        assert_eq!(report.event, "final");
        let mut trailing = String::new();
        let extra = tokio::time::timeout(
            Duration::from_secs(5),
            self.stdout.read_to_string(&mut trailing),
        )
        .await
        .expect("stdout closes")
        .expect("read");
        assert_eq!(
            extra, 0,
            "exactly one final line, got trailing {trailing:?}"
        );
        Outcome {
            report,
            exit_code: status.code(),
            stderr,
        }
    }

    /// Kills and reaps the child, returning the exit status observed.
    async fn abort(mut self) -> std::process::ExitStatus {
        self.child.kill().await.expect("kill");
        let status = self.child.wait().await.expect("reap");
        assert!(
            self.child.try_wait().expect("try_wait").is_some(),
            "child is reaped"
        );
        status
    }
}

// ---------------------------------------------------------------------------
// Raw local server for stalls, caps and cancellation

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
fn config_values_are_finite_and_nonzero() {
    assert_eq!(
        Deadline::new(Duration::ZERO).expect_err("zero"),
        ConfigError::ZeroDeadline
    );
    assert_eq!(Deadline::DEFAULT.duration(), Duration::from_secs(30));
    assert_eq!(ResponseCap::DEFAULT.bytes(), 8 * 1024 * 1024);
    assert_eq!(
        ResponseCap::new(0).expect_err("zero"),
        ConfigError::ZeroResponseCap
    );
    assert_eq!(
        ResponseCap::new(ResponseCap::MAX_BYTES)
            .expect("ceiling")
            .bytes(),
        64 * 1024 * 1024
    );
    assert_eq!(
        ResponseCap::new(ResponseCap::MAX_BYTES + 1).expect_err("above ceiling"),
        ConfigError::ResponseCapAboveCeiling {
            requested: ResponseCap::MAX_BYTES + 1,
            ceiling: ResponseCap::MAX_BYTES,
        }
    );
    let direct = TransportConfig::direct();
    assert!(matches!(direct.proxy, ProxyMode::Direct));
    assert!(matches!(direct.ca, CaMode::PublicRoots));
}

#[test]
fn proxy_url_is_loopback_http_without_credentials_path_or_query() {
    for text in [
        "http://127.0.0.1:8080",
        "http://localhost:8080/",
        "http://[::1]:8080",
    ] {
        ProxyUrl::parse(text).expect(text);
    }
    assert_eq!(
        ProxyUrl::parse("http://127.0.0.1:8080")
            .expect("ok")
            .origin(),
        "http://127.0.0.1:8080"
    );
    let cases = [
        ("https://127.0.0.1:8080", ProxyUrlError::Scheme),
        ("http://proxy.example:8080", ProxyUrlError::NotLoopback),
        ("http://10.0.0.1:8080", ProxyUrlError::NotLoopback),
        ("http://user:pw@127.0.0.1:8080", ProxyUrlError::Credentials),
        ("http://127.0.0.1:8080/path", ProxyUrlError::Path),
        ("http://127.0.0.1:8080/?q=1", ProxyUrlError::Query),
        ("http://127.0.0.1:8080/#f", ProxyUrlError::Fragment),
    ];
    for (text, expected) in cases {
        assert_eq!(ProxyUrl::parse(text).expect_err(text), expected, "{text}");
    }
}

#[test]
fn proxy_mode_builds_with_and_without_loopback_bypass() {
    let endpoint = EndpointUrl::parse("https://uploads.linear.app/x").expect("endpoint");
    for bypass_loopback in [true, false] {
        let config = TransportConfig {
            proxy: ProxyMode::HttpsConnect {
                url: ProxyUrl::parse("http://127.0.0.1:1").expect("proxy"),
                bypass_loopback,
            },
            ..TransportConfig::direct()
        };
        GraphQlTransport::new(endpoint.clone(), fake_key(), config).expect("client builds");
    }
}

#[test]
fn ca_bundle_must_be_a_nonempty_regular_pem_file() {
    let dir = std::env::temp_dir().join(format!("f02b-ca-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let endpoint = EndpointUrl::parse("https://uploads.linear.app/x").expect("endpoint");
    let build = |path: PathBuf| {
        GraphQlTransport::new(
            endpoint.clone(),
            fake_key(),
            TransportConfig {
                ca: CaMode::PublicRootsPlusPem(path),
                ..TransportConfig::direct()
            },
        )
        .expect_err("rejected")
    };
    assert!(matches!(
        build(dir.join("missing.pem")),
        TransportBuildError::CaRead { .. }
    ));
    assert!(matches!(
        build(dir.clone()),
        TransportBuildError::CaNotRegularFile { .. }
    ));
    let empty = dir.join("empty.pem");
    std::fs::write(&empty, " \n").expect("write");
    assert!(matches!(build(empty), TransportBuildError::CaEmpty { .. }));
    let garbage = dir.join("garbage.pem");
    std::fs::write(&garbage, "not a certificate").expect("write");
    let error = build(garbage);
    assert!(
        matches!(
            error,
            TransportBuildError::CaPem { .. } | TransportBuildError::CaNoCertificates { .. }
        ),
        "{error:?}"
    );
    let bogus = dir.join("bogus.pem");
    std::fs::write(
        &bogus,
        "-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----\n",
    )
    .expect("write");
    let error = build(bogus);
    assert!(
        matches!(
            error,
            TransportBuildError::CaPem { .. } | TransportBuildError::CaNoCertificates { .. }
        ),
        "{error:?}"
    );
    std::fs::remove_dir_all(&dir).expect("cleanup");
}

// ---------------------------------------------------------------------------
// Local-server contracts

#[tokio::test(flavor = "current_thread")]
async fn failures_never_expose_query_tokens_or_the_api_key() {
    let closed = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = closed.local_addr().expect("addr").port();
    drop(closed);
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
    assert_eq!(*phase, NetworkPhase::Connect);
    let display = failure.to_string();
    assert!(
        display.starts_with(&format!("connection to http://127.0.0.1:{port} failed: ")),
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
    let app: linear_cli::error::AppError = failure.into();
    assert_eq!(app.kind, AppErrorKind::Transport);
    assert!(std::error::Error::source(&app).is_some());
}

#[tokio::test(flavor = "current_thread")]
async fn direct_mode_ignores_ambient_proxy_variables() {
    // Reqwest reads proxy variables (when it does at all) at client build
    // time, so this test re-executes itself with poisoned proxy variables and
    // proves the configured direct transport still reaches loopback.
    if std::env::var_os("F02B_AMBIENT_PROXY_INNER").is_some() {
        for name in [
            "HTTPS_PROXY",
            "HTTP_PROXY",
            "ALL_PROXY",
            "https_proxy",
            "http_proxy",
            "all_proxy",
        ] {
            assert_eq!(
                std::env::var(name).expect("poisoned"),
                "http://127.0.0.1:1",
                "{name}"
            );
        }
        let server = LocalServer::start(Behavior::Respond {
            status: "200 OK",
            body_len: 16,
        });
        let transport = transport_for(&server.endpoint(), config(Duration::from_secs(5), 1024));
        let response = transport
            .send_raw("{ viewer { id } }", None, None)
            .await
            .expect("direct loopback request succeeds despite ambient proxies");
        assert_eq!(response.status.as_u16(), 200);
        assert_eq!(response.body, vec![b'x'; 16]);
        let observed = server.stop();
        assert!(
            observed
                .request_head
                .starts_with("POST /graphql HTTP/1.1\r\n")
        );
        return;
    }
    let mut command = std::process::Command::new(std::env::current_exe().expect("test binary"));
    command
        .arg("transport::direct_mode_ignores_ambient_proxy_variables")
        .arg("--exact")
        .arg("--test-threads=1")
        .env("F02B_AMBIENT_PROXY_INNER", "1");
    for name in [
        "HTTPS_PROXY",
        "HTTP_PROXY",
        "ALL_PROXY",
        "https_proxy",
        "http_proxy",
        "all_proxy",
    ] {
        command.env(name, "http://127.0.0.1:1");
    }
    command.env_remove("NO_PROXY").env_remove("no_proxy");
    let output = command.output().expect("re-exec");
    assert!(
        output.status.success(),
        "inner run failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
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
    let app: linear_cli::error::AppError = failure.into();
    assert_eq!(app.kind, AppErrorKind::Transport);
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
// P03B fixture-driver contracts

#[test]
fn two_pages_paginate_with_exact_identity_and_counts() {
    run_serial(async {
        let driver = FixtureDriver::start("f02b-teams-two-pages").await;
        assert_eq!(driver.ready.expected_requests, 2);
        assert_eq!(driver.ready.expected_graph_ql, 2);
        let transport = driver.transport();
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
        let outcome = driver.finish().await;
        outcome.assert_clean();
        assert_eq!(outcome.report.consumed, 2);
    })
}

#[test]
fn wrong_variable_is_a_fixture_mismatch_in_the_driver_report() {
    run_serial(async {
        let driver = FixtureDriver::start("f02b-teams-two-pages").await;
        let transport = driver.transport();
        let failure = transport
            .execute::<GetTeams, _>(&teams_request(Some(50), None))
            .await
            .expect_err("fixture mismatch");
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
        // The Rust side sees a GraphQL error; only the driver report proves the
        // request itself was wrong, so the test asserts that report.
        let outcome = driver.finish().await;
        // This operation-specific mismatch is reached only after the server's
        // authorization and User-Agent checks have passed.
        outcome.assert_mismatch("operation fields, arguments, directives, or value origin differ");
        outcome.assert_mismatch("expected 2 interactions");
        assert_eq!(outcome.report.consumed, 0);
        assert_eq!(outcome.report.requests.len(), 1);
        // The current matcher marks authorization only after claiming a step;
        // this wrong-variable request claims none, so the summary is false.
        assert!(!outcome.report.requests[0].authorization_matched);
    })
}

#[test]
fn graphql_errors_classify_ahead_of_http_status() {
    run_serial(async {
        let driver = FixtureDriver::start("f02b-graphql-errors").await;
        let transport = driver.transport();
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
        let app: linear_cli::error::AppError = failure.into();
        assert_eq!(app.kind, AppErrorKind::GraphQl);

        let failure = transport
            .execute::<GetTeams, _>(&page_one)
            .await
            .expect_err("partial data");
        let TransportFailure::GraphQl {
            status,
            partial_data,
            ..
        } = &failure
        else {
            panic!("{failure:?}");
        };
        assert_eq!(status.as_u16(), 200);
        assert!(partial_data, "partial data is never returned as success");

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
        assert_eq!(
            errors[0]
                .extensions
                .as_ref()
                .and_then(|e| e.get("userPresentableMessage")),
            Some(&Value::from("The request was invalid."))
        );
        assert_eq!(failure.to_string(), "The request was invalid.");

        let raw = transport
            .send_request(&page_one)
            .await
            .expect("401 bytes captured");
        assert_eq!(raw.status.as_u16(), 401);
        assert_eq!(
        raw.body,
        br#"{"errors":[{"message":"Authentication required, not authenticated","extensions":{"type":"authentication error","code":"AUTHENTICATION_ERROR"}}]}"#
    );
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
        assert_eq!(raw.status.as_u16(), 400);
        let failure = classify_typed::<Value>(raw).expect_err("validation errors");
        let TransportFailure::GraphQl { status, errors, .. } = &failure else {
            panic!("{failure:?}");
        };
        assert_eq!(status.as_u16(), 400);
        assert_eq!(
            errors[0].message,
            "Cannot query field \"definitelyMissing\" on type \"User\"."
        );
        assert_eq!(
            errors[0].extensions.as_ref().and_then(|e| e.get("code")),
            Some(&Value::from("GRAPHQL_VALIDATION_FAILED"))
        );

        let outcome = driver.finish().await;
        outcome.assert_clean();
        assert_eq!(outcome.report.consumed, 5);
    })
}

#[test]
fn http_failures_keep_raw_bytes_and_redirects_are_never_followed() {
    run_serial(async {
        let driver = FixtureDriver::start("f02b-http-statuses").await;
        let transport = driver.transport();
        let page_one = teams_request(Some(100), None);

        let failure = transport
            .execute::<GetTeams, _>(&page_one)
            .await
            .expect_err("429");
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
            matches!(
                body,
                HttpBodyShape::Unusable(ResponseError::NonJsonExecution(_))
            ),
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
        let app: linear_cli::error::AppError = failure.into();
        assert_eq!(app.kind, AppErrorKind::Transport);

        let failure = transport
            .execute::<GetTeams, _>(&page_one)
            .await
            .expect_err("502");
        let TransportFailure::Http { response, body } = &failure else {
            panic!("{failure:?}");
        };
        assert_eq!(response.status.as_u16(), 502);
        assert_eq!(response.body, b"<html><body>bad gateway</body></html>");
        assert!(matches!(
            body,
            HttpBodyShape::Unusable(ResponseError::NonJsonExecution(_))
        ));

        let failure = transport
            .execute::<GetTeams, _>(&page_one)
            .await
            .expect_err("500 with data");
        let TransportFailure::Http { response, body } = &failure else {
            panic!("{failure:?}");
        };
        assert_eq!(response.status.as_u16(), 500);
        assert!(
            matches!(body, HttpBodyShape::Data),
            "valid data under 500 is still an HTTP failure"
        );
        assert!(
            response
                .body
                .starts_with(br#"{"data":{"teams":{"nodes":[{"id":"team-1""#)
        );

        let raw = transport
            .send_request(&page_one)
            .await
            .expect("302 captured");
        assert_eq!(raw.status.as_u16(), 302);
        assert_eq!(
            raw.headers.get("location").and_then(|v| v.to_str().ok()),
            Some(format!("http://127.0.0.1:{}/elsewhere", driver.ready.port).as_str())
        );
        assert!(raw.body.is_empty());
        let failure = classify_typed::<GetTeams>(raw).expect_err("3xx is a status failure");
        assert!(
            matches!(&failure, TransportFailure::Http { response, .. } if response.status.as_u16() == 302)
        );

        let failure = transport
            .execute::<GetTeams, _>(&page_one)
            .await
            .expect_err("truncated");
        assert!(
            matches!(
                &failure,
                TransportFailure::Response(ResponseError::MalformedJson(_))
            ),
            "{failure:?}"
        );
        let app: linear_cli::error::AppError = failure.into();
        assert_eq!(app.kind, AppErrorKind::Transport);

        let failure = transport
            .execute::<GetTeams, _>(&page_one)
            .await
            .expect_err("empty");
        assert!(
            matches!(
                &failure,
                TransportFailure::Response(ResponseError::MalformedJson(_))
            ),
            "{failure:?}"
        );

        let failure = transport
            .execute::<GetTeams, _>(&page_one)
            .await
            .expect_err("data null");
        assert!(
            matches!(
                &failure,
                TransportFailure::Response(ResponseError::MissingData)
            ),
            "{failure:?}"
        );
        let app: linear_cli::error::AppError = failure.into();
        assert_eq!(app.kind, AppErrorKind::GraphQl);

        let failure = transport
            .execute::<GetTeams, _>(&page_one)
            .await
            .expect_err("wrong shape");
        assert!(
            matches!(
                &failure,
                TransportFailure::Response(ResponseError::UnexpectedShape(_))
            ),
            "{failure:?}"
        );
        let app: linear_cli::error::AppError = failure.into();
        assert_eq!(app.kind, AppErrorKind::Invariant);

        let outcome = driver.finish().await;
        outcome.assert_clean();
        assert_eq!(outcome.report.consumed, 8);
        assert_eq!(
            outcome.report.requests.len(),
            8,
            "no retry and no redirect follow"
        );
    })
}

#[test]
fn mutation_payload_failures_are_typed_and_effects_are_counted() {
    run_serial(async {
        let driver = FixtureDriver::start("f02b-issue-update-effects").await;
        let transport = driver.transport();

        let declined: UpdateIssue = transport
            .execute(&update_request("Declined title"))
            .await
            .expect("success:false is not a transport failure");
        assert!(!declined.issue_update.success);
        assert!(matches!(
            require_success(declined.issue_update.success),
            Err(ResponseError::MutationRejected)
        ));
        assert_eq!(
            declined
                .issue_update
                .issue
                .as_ref()
                .map(|issue| issue.title.as_str()),
            Some("Old title")
        );

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
        require_success(ghost.issue_update.success).expect("success");
        assert!(matches!(
            require_entity(ghost.issue_update.issue),
            Err(ResponseError::MissingPayloadEntity)
        ));

        // A clean report means expectedRecords matched: the declined effect was
        // suppressed and the accepted effect applied exactly once.
        let outcome = driver.finish().await;
        outcome.assert_clean();
        assert_eq!(outcome.report.consumed, 3);
    })
}

#[test]
fn raw_document_without_variables_returns_exact_bytes() {
    run_serial(async {
        let driver = FixtureDriver::start("f02b-raw-viewer").await;
        let transport = driver.transport();
        let response = transport
            .send_raw("{ viewer { id } }", None, None)
            .await
            .expect("200");
        assert_eq!(response.status.as_u16(), 200);
        assert_eq!(response.body, br#"{"data":{"viewer":{"id":"user-1"}}}"#);
        assert_eq!(
            response
                .headers
                .get("content-type")
                .and_then(|v| v.to_str().ok()),
            Some("application/json")
        );
        let value: Value = classify_typed(response).expect("typed as Value");
        assert_eq!(value, json!({"viewer": {"id": "user-1"}}));
        let outcome = driver.finish().await;
        outcome.assert_clean();
        assert_eq!(outcome.report.consumed, 1);
    })
}

#[test]
fn extra_request_after_the_final_step_is_unexpected() {
    run_serial(async {
        let driver = FixtureDriver::start("f02b-raw-viewer").await;
        let transport = driver.transport();
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
        assert!(
            matches!(&failure, TransportFailure::GraphQl { status, .. } if status.as_u16() == 500)
        );
        let outcome = driver.finish().await;
        outcome.assert_mismatch("unexpected request after final interaction");
        outcome.assert_mismatch("expected 1 interactions");
        assert_eq!(outcome.report.unexpected, 1);
        assert_eq!(outcome.report.consumed, 1);
        assert_eq!(outcome.report.requests.len(), 2);
    })
}

#[test]
fn extra_variables_key_is_rejected_by_the_fixture() {
    run_serial(async {
        let driver = FixtureDriver::start("f02b-raw-viewer").await;
        let transport = driver.transport();
        let mut variables = Map::new();
        variables.insert("unexpected".to_owned(), Value::from(1));
        let raw = transport
            .send_raw("{ viewer { id } }", Some(variables), None)
            .await
            .expect("500 captured");
        assert_eq!(raw.status.as_u16(), 500);
        assert_eq!(raw.body, br#"{"errors":[{"message":"fixture mismatch"}]}"#);
        let outcome = driver.finish().await;
        outcome.assert_mismatch("variables presence differs");
        assert_eq!(outcome.report.consumed, 0);
    })
}

#[test]
fn driver_rejects_a_case_without_a_graphql_fixture() {
    run_serial(async {
        let failure = expect_start_failure(
            FixtureDriver::spawn(driver_command("f02b-control-no-graphql-fixture")).await,
        );
        let StartFailure::ExitedBeforeReady { exit_code, stderr } = failure else {
            panic!("{failure:?}");
        };
        assert_eq!(exit_code, Some(2));
        assert!(stderr.contains("has no GraphQL fixture"), "{stderr}");
    })
}

#[test]
fn driver_rejects_extra_arguments_and_paths_outside_its_directory() {
    run_serial(async {
        let mut extra = driver_command("f02b-raw-viewer");
        extra.arg("second");
        let failure = expect_start_failure(FixtureDriver::spawn(extra).await);
        let StartFailure::ExitedBeforeReady { exit_code, stderr } = failure else {
            panic!("{failure:?}");
        };
        assert_eq!(exit_code, Some(2));
        assert!(stderr.contains("expected exactly one argument"), "{stderr}");

        let outside = driver_command("f02b-raw-viewer");
        let root = repo_root();
        let mut args: Vec<std::ffi::OsString> =
            outside.as_std().get_args().map(|a| a.to_owned()).collect();
        args.pop();
        args.push(
            root.join("rust/parity/runner/cases/api-loopback-viewer-200.json")
                .into(),
        );
        let mut outside = Command::new("deno");
        outside.args(args);
        let failure = expect_start_failure(FixtureDriver::spawn(outside).await);
        let StartFailure::ExitedBeforeReady { exit_code, stderr } = failure else {
            panic!("{failure:?}");
        };
        assert_eq!(exit_code, Some(2));
        assert!(stderr.contains("directly under"), "{stderr}");
    })
}

#[test]
fn wrong_port_ready_line_fails_bounded_and_the_child_is_reaped() {
    run_serial(async {
        let closed = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = closed.local_addr().expect("addr").port();
        drop(closed);
        let script = format!(
            "console.log(JSON.stringify({{event:'ready',port:{port},path:'/graphql',expectedRequests:1,expectedGraphQL:1}})); await new Promise((resolve) => setTimeout(resolve, 60_000))"
        );
        let driver = FixtureDriver::spawn(eval_command(&script))
            .await
            .expect("fake driver announces readiness");
        assert_eq!(driver.ready.port, port);
        let transport = transport_for(&driver.endpoint(), config(Duration::from_secs(2), 1024));
        let started = Instant::now();
        let failure = transport
            .send_raw("{ x }", None, None)
            .await
            .expect_err("nothing listens on the announced port");
        assert!(started.elapsed() < Duration::from_secs(5));
        assert!(
            matches!(
                &failure,
                TransportFailure::Network {
                    phase: NetworkPhase::Connect,
                    ..
                }
            ),
            "{failure:?}"
        );
        let status = driver.abort().await;
        assert!(!status.success(), "killed child: {status:?}");
    })
}

#[test]
fn driver_that_never_becomes_ready_is_killed_and_reaped() {
    run_serial(async {
        let started = Instant::now();
        let failure = expect_start_failure(
            FixtureDriver::spawn_with(
                eval_command("await new Promise((resolve) => setTimeout(resolve, 60_000))"),
                Duration::from_secs(3),
            )
            .await,
        );
        let StartFailure::ReadyTimeout { stderr } = failure else {
            panic!("{failure:?}");
        };
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "bounded by the ready deadline"
        );
        assert_eq!(stderr, "", "a stand-in that only waits prints nothing");
    })
}
