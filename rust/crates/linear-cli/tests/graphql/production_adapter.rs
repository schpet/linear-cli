//! Network adapter checks with private process inputs, files, and listeners.
use std::ffi::OsString;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use linear_cli::app::{AppContext, block_on_network, write_final_error};
use linear_cli::auth::file::{CredentialFileSource, CredentialReadFailure};
use linear_cli::auth::keyring::UnsupportedKeyringReader;
use linear_cli::config::{
    FileKind, FileSource, GitProbeResult, GitRootProbe, OsFamily, ProcessEnvSnapshot,
    TransportEnvInputs,
};
use linear_cli::error::{AppError, AppErrorKind, ExitStatus};
use linear_cli::graphql::transport::{
    ApiKey, CaMode, Deadline, EndpointUrl, GraphQlTransport, ProxyMode, RawHttpResponse,
    ResponseCap, TransportBuildError, TransportConfig, classify_typed,
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
            "linear-r02b4-{}-{tick}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir(&path).expect("private test dir");
        Self(path)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for PrivateDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../parity/runner/certs")
        .join(name)
}

fn fake_key() -> ApiKey {
    ApiKey::new("lin_api_fake_r02b4".to_owned()).expect("key")
}

fn config(ca: CaMode, deadline: Duration) -> TransportConfig {
    TransportConfig {
        proxy: ProxyMode::Direct,
        ca,
        deadline: Deadline::new(deadline).expect("deadline"),
        max_response_bytes: ResponseCap::DEFAULT,
    }
}

fn loopback_config(proxy: &str, ca: PathBuf) -> TransportConfig {
    let path = ca.to_str().expect("private test path UTF-8");
    let snapshot = ProcessEnvSnapshot::from_vars_os(
        PathBuf::from("/work"),
        OsFamily::Unix,
        [
            (OsString::from("HTTPS_PROXY"), OsString::from(proxy)),
            (
                OsString::from("NO_PROXY"),
                OsString::from("127.0.0.1,localhost"),
            ),
            (OsString::from("SSL_CERT_FILE"), OsString::from(path)),
            (OsString::from("DENO_CERT"), OsString::from(path)),
        ],
    )
    .expect("synthetic process snapshot");
    TransportEnvInputs::from_process(&snapshot)
        .resolve(
            Deadline::new(Duration::from_secs(2)).expect("deadline"),
            ResponseCap::DEFAULT,
        )
        .expect("production policy")
}

fn build(config: TransportConfig) -> Result<GraphQlTransport, TransportBuildError> {
    GraphQlTransport::new(
        EndpointUrl::parse("https://uploads.linear.app/graphql").expect("endpoint"),
        fake_key(),
        config,
    )
}

#[test]
fn pem_is_checked_once_at_client_build_after_key_selection() {
    let dir = PrivateDir::new();
    let good = dir.path("good.pem");
    std::fs::copy(fixture("test-ca.pem"), &good).expect("copy CA fixture");
    assert!(
        build(config(
            CaMode::PublicRootsPlusPem(good),
            Duration::from_secs(1)
        ))
        .is_ok()
    );

    let empty = dir.path("empty.pem");
    std::fs::write(&empty, "").expect("empty CA");
    assert!(matches!(
        build(config(
            CaMode::PublicRootsPlusPem(empty),
            Duration::from_secs(1)
        )),
        Err(TransportBuildError::CaEmpty { .. })
    ));
    let empty = dir.path("empty.pem");
    let mapped = AppError::from(
        build(config(
            CaMode::PublicRootsPlusPem(empty),
            Duration::from_secs(1),
        ))
        .expect_err("empty CA must fail at construction"),
    );
    assert_eq!(mapped.kind, AppErrorKind::Validation);
    assert!(
        mapped
            .display_message()
            .starts_with("SSL_CERT_FILE: CA bundle ")
    );

    let huge = dir.path("huge.pem");
    let file = std::fs::File::create(&huge).expect("create large CA");
    file.set_len(4 * 1024 * 1024 + 1).expect("extend CA");
    assert!(matches!(
        build(config(
            CaMode::PublicRootsPlusPem(huge),
            Duration::from_secs(1)
        )),
        Err(TransportBuildError::CaTooLarge { .. })
    ));
}

#[cfg(unix)]
#[test]
fn symlinked_regular_bundle_is_accepted_but_nonfiles_are_refused() {
    use std::os::unix::fs::symlink;
    let dir = PrivateDir::new();
    let good = dir.path("good.pem");
    std::fs::copy(fixture("test-ca.pem"), &good).expect("copy CA fixture");
    let link = dir.path("ca-link.pem");
    symlink(&good, &link).expect("file symlink");
    assert!(
        build(config(
            CaMode::PublicRootsPlusPem(link),
            Duration::from_secs(1)
        ))
        .is_ok()
    );

    for (name, target) in [
        ("dir-link", dir.0.clone()),
        ("dangling-link", dir.path("missing")),
    ] {
        let link = dir.path(name);
        symlink(target, &link).expect("symlink");
        let result = build(config(
            CaMode::PublicRootsPlusPem(link),
            Duration::from_secs(1),
        ));
        match name {
            "dir-link" => assert!(matches!(
                result,
                Err(TransportBuildError::CaNotRegularFile { .. })
            )),
            "dangling-link" => assert!(matches!(result, Err(TransportBuildError::CaRead { .. }))),
            _ => unreachable!("fixed test cases"),
        }
    }
    let fifo = dir.path("fifo");
    let status = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .expect("mkfifo present");
    assert!(status.success());
    let link = dir.path("fifo-link");
    symlink(&fifo, &link).expect("FIFO symlink");
    assert!(matches!(
        build(config(
            CaMode::PublicRootsPlusPem(link),
            Duration::from_secs(1)
        )),
        Err(TransportBuildError::CaNotRegularFile { .. })
    ));
}

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
        let body = b"{\"data\":{\"ok\":true}}";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.write_all(body);
    });
    (url, handle)
}

#[test]
fn current_thread_runtime_has_io_and_a_test_deadline() {
    let (url, server) = serve_once(Duration::ZERO);
    let transport = GraphQlTransport::new(
        EndpointUrl::parse(&url).expect("endpoint"),
        fake_key(),
        config(CaMode::PublicRoots, Duration::from_secs(2)),
    )
    .expect("client");
    let response = block_on_network(async move {
        transport
            .send_raw("query { ok }", None, None)
            .await
            .map_err(AppError::from)
    })
    .expect("loopback response");
    assert_eq!(response.body, b"{\"data\":{\"ok\":true}}");
    server.join().expect("server");

    let (url, server) = serve_once(Duration::from_millis(350));
    let transport = GraphQlTransport::new(
        EndpointUrl::parse(&url).expect("endpoint"),
        fake_key(),
        config(CaMode::PublicRoots, Duration::from_millis(150)),
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

fn tls_proxy() -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("private CONNECT listener");
    let proxy = format!("http://{}", listener.local_addr().expect("address"));
    let handle = thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test TLS runtime");
        runtime.block_on(async move {
            let (stream, _) = listener.accept().expect("CONNECT");
            stream.set_nonblocking(true).expect("nonblocking stream");
            let mut stream = tokio::net::TcpStream::from_std(stream).expect("Tokio stream");
            let mut head = Vec::new();
            loop {
                let mut byte = [0_u8; 1];
                let count = stream.read(&mut byte).await.expect("CONNECT read");
                assert_eq!(count, 1, "CONNECT head closed");
                head.push(byte[0]);
                if head.ends_with(b"\r\n\r\n") { break; }
                assert!(head.len() < 4096, "CONNECT head bounded");
            }
            stream.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
                .await.expect("CONNECT reply");
            let cert = CertificateDer::from_pem_file(fixture("leaf.pem")).expect("leaf certificate");
            let key = PrivateKeyDer::from_pem_file(fixture("leaf.key")).expect("leaf private key");
            let server = ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(vec![cert], key)
                .expect("server certificate");
            let acceptor = TlsAcceptor::from(std::sync::Arc::new(server));
            let mut tls = match acceptor.accept(stream).await {
                Ok(tls) => tls,
                Err(_) => return String::from_utf8(head).expect("CONNECT text"),
            };
            let mut request = [0_u8; 4096];
            let count = tls.read(&mut request).await.expect("TLS request");
            assert!(count > 0);
            let body = b"{\"data\":{\"ok\":true}}";
            let response = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n", body.len());
            tls.write_all(response.as_bytes()).await.expect("TLS response headers");
            tls.write_all(body).await.expect("TLS response body");
            String::from_utf8(head).expect("CONNECT text")
        })
    });
    (proxy, handle)
}

#[test]
fn exact_loopback_proxy_and_matching_ca_complete_a_tls_request() {
    let dir = PrivateDir::new();
    let ca = dir.path("test-ca.pem");
    std::fs::copy(fixture("test-ca.pem"), &ca).expect("copy CA");
    let (proxy, server) = tls_proxy();
    let transport = build(loopback_config(&proxy, ca)).expect("transport");
    let response = block_on_network(async move {
        transport
            .send_raw("query { ok }", None, None)
            .await
            .map_err(AppError::from)
    })
    .expect("TLS response");
    assert_eq!(response.body, b"{\"data\":{\"ok\":true}}");
    let connect = server.join().expect("TLS proxy");
    assert!(connect.starts_with("CONNECT uploads.linear.app:443 HTTP/1.1\r\n"));
}

#[test]
fn wrong_ca_fails_at_tls_handshake_and_closed_port_fails_at_connect() {
    let dir = PrivateDir::new();
    let wrong = dir.path("wrong.pem");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/graphql/fixtures/r02b4-wrong-ca.pem"),
        &wrong,
    )
    .expect("copy wrong CA");
    let (proxy, server) = tls_proxy();
    let transport = build(loopback_config(&proxy, wrong.clone())).expect("valid wrong CA parses");
    let error = block_on_network(async move {
        transport
            .send_raw("query { ok }", None, None)
            .await
            .map_err(AppError::from)
    })
    .expect_err("wrong CA");
    assert_eq!(error.kind, AppErrorKind::Transport);
    assert!(error.display_message().contains("failed"), "{error}");
    assert!(format!("{error:?}").contains("UnknownIssuer"), "{error:?}");
    let connect = server.join().expect("TLS proxy");
    assert!(connect.starts_with("CONNECT uploads.linear.app:443 HTTP/1.1\r\n"));

    let listener = TcpListener::bind("127.0.0.1:0").expect("reserve closed port");
    let closed_proxy = format!("http://{}", listener.local_addr().expect("address"));
    drop(listener);
    let transport = build(loopback_config(&closed_proxy, wrong)).expect("transport builds");
    let started = Instant::now();
    let error = block_on_network(async move {
        transport
            .send_raw("query { ok }", None, None)
            .await
            .map_err(AppError::from)
    })
    .expect_err("closed proxy port");
    assert_eq!(error.kind, AppErrorKind::Transport);
    assert!(error.display_message().contains("failed"), "{error}");
    assert!(!format!("{error:?}").contains("UnknownIssuer"));
    assert!(started.elapsed() < Duration::from_secs(2));
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
struct NoGit;
impl GitRootProbe for NoGit {
    fn probe(&self) -> GitProbeResult {
        GitProbeResult::SpawnFailure
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
        &NoGit,
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
        headers: HeaderMap::new(),
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
        config(CaMode::PublicRoots, Duration::from_millis(200)),
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
