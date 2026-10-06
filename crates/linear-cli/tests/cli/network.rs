//! TLS trust, `SSL_CERT_FILE` and proxy environment variables.
use std::net::TcpListener;
use std::path::Path;
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_rustls::TlsAcceptor;
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};

use crate::support::{Cli, MockLinear};

const QUERY: &str = "query Probe { ok }";
const OK_BODY: &str = r#"{"data":{"ok":true}}"#;

fn tls_fixture(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/tls")
        .join(name)
        .display()
        .to_string()
}

/// What the TLS listener saw: the proxy `CONNECT` head when it acted as a
/// proxy, and whether the TLS handshake completed.
struct TlsObserved {
    connect: Option<String>,
    handshake: bool,
}

fn ok_response() -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{OK_BODY}",
        OK_BODY.len()
    )
}

/// Accepts one connection and answers one HTTPS request with `response`,
/// using the leaf certificate (valid for `uploads.linear.app`, `localhost`
/// and `127.0.0.1`). As a proxy, it first accepts a `CONNECT` request.
fn tls_listener(proxy: bool, response: String) -> (u16, JoinHandle<TlsObserved>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let port = listener.local_addr().expect("address").port();
    let handle = thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        runtime.block_on(async move {
            let (stream, _) = listener.accept().expect("connection");
            stream.set_nonblocking(true).expect("nonblocking stream");
            let mut stream = tokio::net::TcpStream::from_std(stream).expect("tokio stream");
            let mut connect = None;
            if proxy {
                let mut head = Vec::new();
                while !head.ends_with(b"\r\n\r\n") {
                    let mut byte = [0_u8; 1];
                    let count = stream.read(&mut byte).await.expect("CONNECT read");
                    assert_eq!(count, 1, "CONNECT head closed");
                    head.extend_from_slice(&byte);
                    assert!(head.len() < 4096, "CONNECT head bounded");
                }
                stream
                    .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
                    .await
                    .expect("CONNECT reply");
                connect = Some(String::from_utf8(head).expect("CONNECT text"));
            }
            let cert = CertificateDer::from_pem_file(tls_fixture("leaf.pem")).expect("leaf cert");
            let key = PrivateKeyDer::from_pem_file(tls_fixture("leaf.key")).expect("leaf key");
            let config = ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(vec![cert], key)
                .expect("server certificate");
            let Ok(mut tls) = TlsAcceptor::from(Arc::new(config)).accept(stream).await else {
                return TlsObserved {
                    connect,
                    handshake: false,
                };
            };
            let mut request = [0_u8; 4096];
            let count = tls.read(&mut request).await.expect("TLS request");
            assert!(count > 0);
            tls.write_all(response.as_bytes())
                .await
                .expect("TLS response");
            tls.shutdown().await.expect("TLS close");
            TlsObserved {
                connect,
                handshake: true,
            }
        })
    });
    (port, handle)
}

fn cli(endpoint: &str) -> Cli {
    Cli::new()
        .env("LINEAR_API_KEY", "lin_api_fake")
        .env("LINEAR_GRAPHQL_ENDPOINT", endpoint)
}

#[test]
fn ssl_cert_file_roots_verify_a_private_certificate() {
    let (port, server) = tls_listener(false, ok_response());
    let run = cli(&format!("https://localhost:{port}/graphql"))
        .env("SSL_CERT_FILE", &tls_fixture("test-ca.pem"))
        .run(&["api", QUERY]);
    assert_eq!(run.success().stdout.trim_end(), OK_BODY);
    assert!(server.join().expect("TLS server").handshake);
}

#[test]
fn an_unrelated_ca_bundle_fails_the_handshake() {
    let (port, server) = tls_listener(false, ok_response());
    cli(&format!("https://localhost:{port}/graphql"))
        .env("SSL_CERT_FILE", &tls_fixture("wrong-ca.pem"))
        .run(&["api", QUERY])
        .unavailable()
        .stderr_has("certificate");
    assert!(!server.join().expect("TLS server").handshake);
}

#[test]
fn https_proxy_variables_route_requests_through_connect() {
    for proxy_var in ["HTTPS_PROXY", "https_proxy"] {
        let (port, server) = tls_listener(true, ok_response());
        let run = cli("https://uploads.linear.app/graphql")
            .env(proxy_var, &format!("http://127.0.0.1:{port}"))
            .env("SSL_CERT_FILE", &tls_fixture("test-ca.pem"))
            .run(&["api", QUERY]);
        assert_eq!(run.success().stdout.trim_end(), OK_BODY, "{proxy_var}");
        let connect = server.join().expect("TLS proxy").connect;
        assert!(
            connect
                .as_deref()
                .is_some_and(|head| head.starts_with("CONNECT uploads.linear.app:443 HTTP/1.1\r\n")),
            "{proxy_var}: {connect:?}"
        );
    }
}

#[test]
fn a_redirect_from_https_to_plain_http_is_refused() {
    let api = MockLinear::start();
    let (port, server) = tls_listener(
        false,
        format!(
            "HTTP/1.1 307 Temporary Redirect\r\nLocation: {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            api.url()
        ),
    );
    cli(&format!("https://localhost:{port}/graphql"))
        .env("SSL_CERT_FILE", &tls_fixture("test-ca.pem"))
        .run(&["api", QUERY])
        .unavailable()
        .stderr_has("refusing to follow a redirect from HTTPS to plain HTTP");
    assert!(server.join().expect("TLS server").handshake);
    assert!(api.requests().is_empty());
}

#[test]
fn no_proxy_bypasses_the_proxy_for_listed_hosts() {
    // Nothing listens on port 1, so a request through this proxy fails.
    let dead_proxy = "http://127.0.0.1:1";
    let api = MockLinear::start();
    api.on("Probe", json!({ "ok": true }));
    let run = Cli::for_api(&api)
        .env("HTTP_PROXY", dead_proxy)
        .env("NO_PROXY", "127.0.0.1")
        .run(&["api", QUERY]);
    assert_eq!(run.success().json(), json!({ "data": { "ok": true } }));

    Cli::for_api(&api)
        .env("HTTP_PROXY", dead_proxy)
        .run(&["api", QUERY])
        .unavailable();
    assert_eq!(api.operations(), ["Probe"]);
}
