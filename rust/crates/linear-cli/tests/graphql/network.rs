//! Client construction, TLS, proxy and error-rendering checks against
//! private listeners and files.
use std::ffi::OsString;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use linear_cli::app::{AppContext, block_on_network, write_final_error};
use linear_cli::auth::file::{CredentialFileSource, CredentialReadFailure};
use linear_cli::auth::keyring::UnsupportedKeyringReader;
use linear_cli::config::{FileKind, FileSource, OsFamily, ProcessEnvSnapshot};
use linear_cli::error::{AppError, AppErrorKind, ExitStatus};
use linear_cli::graphql::transport::{
    ApiKey, Deadline, EndpointUrl, GraphQlTransport, RawHttpResponse, ResponseCap, TransportConfig,
    classify_typed,
};
use linear_cli::startup::{AppStartupReport, load};
use reqwest::StatusCode;
use reqwest::header::HeaderMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_rustls::TlsAcceptor;
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct PrivateDir(PathBuf);

impl PrivateDir {
    fn new() -> Self {
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let sequence = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "linear-network-{}-{tick}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir(&path).expect("private test dir");
        Self(path)
    }
}

impl Drop for PrivateDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/tls")
        .join(name)
}

fn fake_key() -> ApiKey {
    ApiKey::new("lin_api_fake_network".to_owned()).expect("key")
}

fn config(deadline: Duration) -> TransportConfig {
    TransportConfig {
        ca_bundle: None,
        deadline: Deadline::new(deadline).expect("deadline"),
        max_response_bytes: ResponseCap::DEFAULT,
    }
}

const OK_BODY: &[u8] = b"{\"data\":{\"ok\":true}}";

fn serve_once(delay: Duration) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("private listener");
    let url = format!("http://{}/graphql", listener.local_addr().expect("address"));
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("one request");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("read timeout");
        let mut request = [0_u8; 2048];
        let _ = stream.read(&mut request);
        thread::sleep(delay);
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n",
            OK_BODY.len()
        );
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.write_all(OK_BODY);
    });
    (url, handle)
}

#[test]
fn current_thread_runtime_has_io_and_a_test_deadline() {
    let (url, server) = serve_once(Duration::ZERO);
    let transport = GraphQlTransport::new(
        EndpointUrl::parse(&url).expect("endpoint"),
        fake_key(),
        config(Duration::from_secs(2)),
    )
    .expect("client");
    let response = block_on_network(async move {
        transport
            .send_raw("query { ok }", None, None)
            .await
            .map_err(AppError::from)
    })
    .expect("loopback response");
    assert_eq!(response.body, OK_BODY);
    server.join().expect("server");

    let (url, server) = serve_once(Duration::from_millis(350));
    let transport = GraphQlTransport::new(
        EndpointUrl::parse(&url).expect("endpoint"),
        fake_key(),
        config(Duration::from_millis(150)),
    )
    .expect("client");
    let started = Instant::now();
    let error = block_on_network(async move {
        transport
            .send_raw("query { ok }", None, None)
            .await
            .map_err(AppError::from)
    })
    .expect_err("deadline must fail");
    assert_eq!(error.kind, AppErrorKind::Transport);
    assert!(error.display_message().contains("did not complete"));
    assert!(started.elapsed() < Duration::from_secs(2));
    server.join().expect("server");
}

/// What a TLS listener saw: the proxy `CONNECT` head, if it acted as a
/// proxy, and whether the TLS handshake completed.
struct TlsObserved {
    connect: Option<String>,
    handshake: bool,
}

/// Accepts one connection and answers one HTTPS request with the leaf
/// certificate (valid for `uploads.linear.app`, `localhost` and
/// `127.0.0.1`). As a proxy, it first accepts a `CONNECT` request.
fn tls_listener(proxy: bool) -> (u16, thread::JoinHandle<TlsObserved>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("private TLS listener");
    let port = listener.local_addr().expect("address").port();
    let handle = thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test TLS runtime");
        runtime.block_on(async move {
            let (stream, _) = listener.accept().expect("connection");
            stream.set_nonblocking(true).expect("nonblocking stream");
            let mut stream = tokio::net::TcpStream::from_std(stream).expect("Tokio stream");
            let mut connect = None;
            if proxy {
                let mut head = Vec::new();
                while !head.ends_with(b"\r\n\r\n") {
                    let mut byte = [0_u8; 1];
                    let count = stream.read(&mut byte).await.expect("CONNECT read");
                    assert_eq!(count, 1, "CONNECT head closed");
                    head.push(byte[0]);
                    assert!(head.len() < 4096, "CONNECT head bounded");
                }
                stream
                    .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
                    .await
                    .expect("CONNECT reply");
                connect = Some(String::from_utf8(head).expect("CONNECT text"));
            }
            let cert = CertificateDer::from_pem_file(fixture("leaf.pem")).expect("leaf certificate");
            let key = PrivateKeyDer::from_pem_file(fixture("leaf.key")).expect("leaf private key");
            let server = ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(vec![cert], key)
                .expect("server certificate");
            let acceptor = TlsAcceptor::from(std::sync::Arc::new(server));
            let Ok(mut tls) = acceptor.accept(stream).await else {
                return TlsObserved {
                    connect,
                    handshake: false,
                };
            };
            let mut request = [0_u8; 4096];
            let count = tls.read(&mut request).await.expect("TLS request");
            assert!(count > 0);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n",
                OK_BODY.len()
            );
            tls.write_all(response.as_bytes()).await.expect("TLS response headers");
            tls.write_all(OK_BODY).await.expect("TLS response body");
            tls.shutdown().await.expect("TLS close");
            TlsObserved {
                connect,
                handshake: true,
            }
        })
    });
    (port, handle)
}

fn tls_request(port: u16, ca_bundle: PathBuf) -> Result<RawHttpResponse, AppError> {
    let transport = GraphQlTransport::new(
        EndpointUrl::parse(&format!("https://localhost:{port}/graphql")).expect("endpoint"),
        fake_key(),
        TransportConfig {
            ca_bundle: Some(ca_bundle),
            ..config(Duration::from_secs(5))
        },
    )
    .expect("transport");
    block_on_network(async move {
        transport
            .send_raw("query { ok }", None, None)
            .await
            .map_err(AppError::from)
    })
}

#[test]
fn ca_bundle_roots_verify_a_private_certificate() {
    let (port, server) = tls_listener(false);
    let response = tls_request(port, fixture("test-ca.pem")).expect("TLS response");
    assert_eq!(response.body, OK_BODY);
    assert!(server.join().expect("TLS server").handshake);
}

#[test]
fn an_unrelated_ca_bundle_fails_the_handshake() {
    let (port, server) = tls_listener(false);
    let error = tls_request(port, fixture("wrong-ca.pem")).expect_err("wrong CA");
    assert_eq!(error.kind, AppErrorKind::Transport);
    assert!(format!("{error:?}").contains("UnknownIssuer"), "{error:?}");
    assert!(!server.join().expect("TLS server").handshake);
}

/// Runs `linear api` against `endpoint` with only the given environment.
fn run_api(endpoint: &str, env: &[(&str, &Path)]) -> Output {
    let home = PrivateDir::new();
    let mut command = Command::new(env!("CARGO_BIN_EXE_linear"));
    command
        .args(["api", "query { ok }"])
        .current_dir(&home.0)
        .env_clear()
        .env("HOME", &home.0)
        .env("XDG_CONFIG_HOME", &home.0)
        .env("APPDATA", &home.0)
        .env("NO_COLOR", "1")
        .env("LINEAR_IGNORE_ENV_FILE", "1")
        .env("LINEAR_API_KEY", "lin_api_fake_network")
        .env("LINEAR_GRAPHQL_ENDPOINT", endpoint)
        .stdin(Stdio::null());
    for (name, value) in env {
        command.env(name, value);
    }
    command.output().expect("run linear")
}

#[test]
fn https_proxy_variables_route_requests_through_connect() {
    for (proxy_var, cert_var) in [
        ("HTTPS_PROXY", "SSL_CERT_FILE"),
        ("https_proxy", "DENO_CERT"),
    ] {
        let (port, server) = tls_listener(true);
        let proxy = PathBuf::from(format!("http://127.0.0.1:{port}"));
        let ca = fixture("test-ca.pem");
        let output = run_api(
            "https://uploads.linear.app/graphql",
            &[(proxy_var, &proxy), (cert_var, &ca)],
        );
        assert!(
            output.status.success(),
            "{proxy_var}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, OK_BODY);
        let observed = server.join().expect("TLS proxy");
        assert!(
            observed
                .connect
                .as_deref()
                .is_some_and(|head| head.starts_with("CONNECT uploads.linear.app:443 HTTP/1.1\r\n")),
            "{proxy_var}"
        );
    }
}

#[test]
fn no_proxy_bypasses_the_proxy_for_listed_hosts() {
    let closed = TcpListener::bind("127.0.0.1:0").expect("reserve closed port");
    let proxy = PathBuf::from(format!("http://{}", closed.local_addr().expect("address")));
    drop(closed);
    let (url, server) = serve_once(Duration::ZERO);
    let output = run_api(
        &url,
        &[("HTTP_PROXY", &proxy), ("NO_PROXY", Path::new("127.0.0.1"))],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, OK_BODY);
    server.join().expect("server");

    let output = run_api(&url, &[("HTTP_PROXY", &proxy)]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "unlisted host uses the dead proxy"
    );
}

struct EmptyFiles;
impl FileSource for EmptyFiles {
    fn kind(&self, _path: &Path) -> std::io::Result<Option<FileKind>> {
        Ok(None)
    }
    fn read_bounded(&self, _path: &Path, _max_bytes: u64) -> std::io::Result<Vec<u8>> {
        Err(std::io::Error::from(std::io::ErrorKind::NotFound))
    }
}

struct EmptyCredentials;
impl CredentialFileSource for EmptyCredentials {
    fn read_credentials(&self, _path: &Path) -> Result<Option<Vec<u8>>, CredentialReadFailure> {
        Ok(None)
    }
}

fn empty_startup(snapshot: &ProcessEnvSnapshot) -> AppStartupReport {
    load(
        snapshot,
        &EmptyFiles,
        &EmptyCredentials,
        &UnsupportedKeyringReader,
    )
}

fn render_graphql_failure(debug: Option<&str>) -> String {
    let variables = debug
        .into_iter()
        .map(|value| (OsString::from("LINEAR_DEBUG"), OsString::from(value)))
        .chain([(OsString::from("NO_COLOR"), OsString::from("1"))]);
    let snapshot =
        ProcessEnvSnapshot::from_vars_os(PathBuf::from("/work"), OsFamily::Unix, variables)
            .expect("test env");
    let startup = empty_startup(&snapshot);
    let response = RawHttpResponse {
        status: StatusCode::BAD_REQUEST,
        headers: HeaderMap::from_iter([(reqwest::header::CONTENT_TYPE, reqwest::header::HeaderValue::from_static("application/json"))]),
        body: b"{\"errors\":[{\"message\":\"Backend failed\",\"extensions\":{\"userPresentableMessage\":\"Try again\"}}]}".to_vec(),
    };
    let failure = classify_typed::<serde_json::Value>(response).expect_err("GraphQL errors");
    let error = AppError::from(failure).with_context("Failed to get user info");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut context = AppContext {
        startup,
        cwd: PathBuf::from("/work"),
        stdout: &mut stdout,
        stderr: &mut stderr,
        stdin_tty: false,
        stdout_tty: false,
        stderr_tty: false,
        stdout_finalization: None,
    };
    assert_eq!(
        write_final_error(&mut context, &error).expect("render"),
        ExitStatus::HandledFailure
    );
    assert!(stdout.is_empty());
    String::from_utf8(stderr).expect("UTF-8 diagnostic")
}

#[test]
fn graphql_debug_presentation_is_exact_for_absent_one_and_true() {
    let ordinary = "✗ Failed to get user info: Try again\n";
    assert_eq!(render_graphql_failure(None), ordinary);
    let debug = concat!(
        "✗ Failed to get user info: Try again\n",
        "  debug: GraphQL HTTP 400 Bad Request; errors=1; partial_data=false\n",
    );
    assert_eq!(render_graphql_failure(Some("1")), debug);
    assert_eq!(render_graphql_failure(Some("true")), debug);
    for sentinel in ["lin_api_fake", "Authorization", "Backend failed"] {
        assert!(!debug.contains(sentinel));
    }
}

#[test]
fn transport_debug_includes_a_source_chain() {
    let snapshot = ProcessEnvSnapshot::from_vars_os(
        PathBuf::from("/work"),
        OsFamily::Unix,
        [(OsString::from("LINEAR_DEBUG"), OsString::from("1"))],
    )
    .expect("test env");
    let startup = empty_startup(&snapshot);
    let error = AppError::new(AppErrorKind::Transport, "request failed")
        .with_source(std::io::Error::other("synthetic connection failure"));
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut context = AppContext {
        startup,
        cwd: PathBuf::from("/work"),
        stdout: &mut stdout,
        stderr: &mut stderr,
        stdin_tty: false,
        stdout_tty: false,
        stderr_tty: false,
        stdout_finalization: None,
    };
    assert_eq!(
        write_final_error(&mut context, &error).expect("render"),
        ExitStatus::HandledFailure
    );
    assert_eq!(
        String::from_utf8(stderr).expect("UTF-8"),
        "✗ request failed\n  caused by: synthetic connection failure\n"
    );
}

#[test]
fn real_network_failure_debug_omits_endpoint_query_and_api_key() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("reserve closed endpoint");
    let endpoint = format!(
        "http://{}/graphql?sentinel_query=private",
        listener.local_addr().expect("address")
    );
    drop(listener);
    let transport = GraphQlTransport::new(
        EndpointUrl::parse(&endpoint).expect("endpoint"),
        fake_key(),
        config(Duration::from_millis(200)),
    )
    .expect("client");
    let error = block_on_network(async move {
        transport
            .send_raw("query { ok }", None, None)
            .await
            .map_err(AppError::from)
    })
    .expect_err("closed endpoint");
    assert_eq!(error.kind, AppErrorKind::Transport);
    let snapshot = ProcessEnvSnapshot::from_vars_os(
        PathBuf::from("/work"),
        OsFamily::Unix,
        [(OsString::from("LINEAR_DEBUG"), OsString::from("1"))],
    )
    .expect("debug env");
    let startup = empty_startup(&snapshot);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut context = AppContext {
        startup,
        cwd: PathBuf::from("/work"),
        stdout: &mut stdout,
        stderr: &mut stderr,
        stdin_tty: false,
        stdout_tty: false,
        stderr_tty: false,
        stdout_finalization: None,
    };
    assert_eq!(
        write_final_error(&mut context, &error).expect("render"),
        ExitStatus::HandledFailure
    );
    assert!(stdout.is_empty());
    let rendered = String::from_utf8(stderr).expect("UTF-8");
    assert!(
        rendered.starts_with("✗ connection to http://127.0.0.1:"),
        "{rendered}"
    );
    assert!(
        rendered.contains("  caused by: error sending request\n"),
        "{rendered}"
    );
    assert!(!rendered.contains("sentinel_query"), "{rendered}");
    assert!(!rendered.contains("lin_api_fake"), "{rendered}");
}
