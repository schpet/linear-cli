//! Fixed-host asset GET contracts (F02B Gate 2), without any fixture or TLS.
//!
//! The confined-lane adapter, the two-host URL type, the redirect resolver
//! and the shared client factory are exercised here. Network tests use only a
//! loopback stand-in proxy that answers `CONNECT` with 403 (so no TLS, no
//! DNS and no external host is ever reached); the TLS/CA/CONNECT-relay
//! behaviour is qualified through the P03C2 lane by
//! `rust/parity/runner/f02b-fixed-host-driver.ts`.

use std::io::{ErrorKind, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use linear_cli::graphql::transport::{
    ApiKey, AssetFailure, AssetHost, AssetHttpTransport, AssetUrl, AssetUrlError, CaMode,
    ConfinedEnvError, ConfinedEnvValues, ConfinedTransportEnv, Deadline, EndpointUrl,
    GraphQlTransport, LOOPBACK_NO_PROXY, MAX_REDIRECTS, NetworkPhase, NoProxyError, ProxyMode,
    ProxyUrl, ProxyUrlError, RedirectRejection, ResponseCap, TransportBuildError, TransportConfig,
    follow_redirect, resolve_redirect,
};
use reqwest::StatusCode;
use reqwest::header::HeaderValue;

const PRIVATE: &str = "https://uploads.linear.app/private.png?token=SIGNED-SECRET-TOKEN";
const PUBLIC: &str = "https://public.linear.app/public.png?token=SIGNED-SECRET-TOKEN";
const SECRETS: [&str; 4] = ["SIGNED-SECRET-TOKEN", "token", "private.png", "SECRET_KEY"];

fn key() -> ApiKey {
    ApiKey::new("lin_api_SECRET_KEY_VALUE".to_owned()).expect("key")
}

fn values() -> ConfinedEnvValues {
    ConfinedEnvValues {
        https_proxy: Some("http://127.0.0.1:43111".to_owned()),
        http_proxy: None,
        all_proxy: None,
        no_proxy: Some("127.0.0.1,localhost".to_owned()),
        ssl_cert_file: Some("/var/tmp/lane/linear-parity-test-ca.pem".to_owned()),
    }
}

fn connect_config(proxy: &str) -> TransportConfig {
    TransportConfig {
        proxy: ProxyMode::HttpsConnect {
            url: ProxyUrl::parse(proxy).expect("proxy"),
            bypass_loopback: true,
        },
        ca: CaMode::PublicRoots,
        deadline: Deadline::new(Duration::from_secs(5)).expect("deadline"),
        max_response_bytes: ResponseCap::new(1024).expect("cap"),
    }
}

fn assert_no_secrets(text: &str) {
    for secret in SECRETS {
        assert!(!text.contains(secret), "{secret} leaked into {text}");
    }
}

// ---------------------------------------------------------------------------
// Runner-env adapter

#[test]
fn confined_env_parses_exact_runner_values_into_connect_plus_pem_config() {
    let env = ConfinedTransportEnv::parse(&values()).expect("runner values");
    assert_eq!(env.proxy().origin(), "http://127.0.0.1:43111");
    assert_eq!(
        env.ca_bundle(),
        PathBuf::from("/var/tmp/lane/linear-parity-test-ca.pem")
    );
    let config = env.into_config(
        Deadline::new(Duration::from_secs(3)).expect("deadline"),
        ResponseCap::new(64).expect("cap"),
    );
    match &config.proxy {
        ProxyMode::HttpsConnect {
            url,
            bypass_loopback,
        } => {
            assert_eq!(url.origin(), "http://127.0.0.1:43111");
            assert!(*bypass_loopback);
        }
        ProxyMode::Direct => panic!("direct"),
    }
    match &config.ca {
        CaMode::PublicRootsPlusPem(path) => {
            assert_eq!(
                path,
                &PathBuf::from("/var/tmp/lane/linear-parity-test-ca.pem")
            );
        }
        CaMode::PublicRoots => panic!("public roots only"),
    }
    assert_eq!(config.deadline.duration(), Duration::from_secs(3));
    assert_eq!(config.max_response_bytes.bytes(), 64);
    assert_eq!(LOOPBACK_NO_PROXY, "127.0.0.1,localhost");
    // Order of the two entries is not significant.
    let mut reordered = values();
    reordered.no_proxy = Some("localhost,127.0.0.1".to_owned());
    ConfinedTransportEnv::parse(&reordered).expect("either order");
}

#[test]
fn confined_env_rejects_absent_empty_unexpected_and_malformed_values() {
    let cases: Vec<(&str, ConfinedEnvValues, ConfinedEnvError)> = vec![
        (
            "missing proxy",
            ConfinedEnvValues {
                https_proxy: None,
                ..values()
            },
            ConfinedEnvError::Missing("HTTPS_PROXY"),
        ),
        (
            "empty proxy",
            ConfinedEnvValues {
                https_proxy: Some(String::new()),
                ..values()
            },
            ConfinedEnvError::Empty("HTTPS_PROXY"),
        ),
        (
            "missing no_proxy",
            ConfinedEnvValues {
                no_proxy: None,
                ..values()
            },
            ConfinedEnvError::Missing("NO_PROXY"),
        ),
        (
            "empty no_proxy",
            ConfinedEnvValues {
                no_proxy: Some(String::new()),
                ..values()
            },
            ConfinedEnvError::Empty("NO_PROXY"),
        ),
        (
            "missing ca",
            ConfinedEnvValues {
                ssl_cert_file: None,
                ..values()
            },
            ConfinedEnvError::Missing("SSL_CERT_FILE"),
        ),
        (
            "empty ca",
            ConfinedEnvValues {
                ssl_cert_file: Some(String::new()),
                ..values()
            },
            ConfinedEnvError::Empty("SSL_CERT_FILE"),
        ),
        (
            "relative ca",
            ConfinedEnvValues {
                ssl_cert_file: Some("certs/test-ca.pem".to_owned()),
                ..values()
            },
            ConfinedEnvError::CaPathNotAbsolute,
        ),
        (
            "http proxy present",
            ConfinedEnvValues {
                http_proxy: Some("http://127.0.0.1:43111".to_owned()),
                ..values()
            },
            ConfinedEnvError::Unexpected("HTTP_PROXY"),
        ),
        (
            "all proxy present",
            ConfinedEnvValues {
                all_proxy: Some("http://127.0.0.1:43111".to_owned()),
                ..values()
            },
            ConfinedEnvError::Unexpected("ALL_PROXY"),
        ),
        (
            "https scheme proxy",
            ConfinedEnvValues {
                https_proxy: Some("https://127.0.0.1:43111".to_owned()),
                ..values()
            },
            ConfinedEnvError::Proxy(ProxyUrlError::Scheme("https".to_owned())),
        ),
        (
            "non-loopback proxy",
            ConfinedEnvValues {
                https_proxy: Some("http://proxy.example:3128".to_owned()),
                ..values()
            },
            ConfinedEnvError::Proxy(ProxyUrlError::NotLoopback),
        ),
        (
            "one entry only",
            ConfinedEnvValues {
                no_proxy: Some("127.0.0.1".to_owned()),
                ..values()
            },
            ConfinedEnvError::NoProxy(NoProxyError::NotConfinedPolicy),
        ),
        (
            "extra loopback entry",
            ConfinedEnvValues {
                no_proxy: Some("127.0.0.1,localhost,::1".to_owned()),
                ..values()
            },
            ConfinedEnvError::NoProxy(NoProxyError::NotConfinedPolicy),
        ),
        (
            "padded entry",
            ConfinedEnvValues {
                no_proxy: Some("127.0.0.1, localhost".to_owned()),
                ..values()
            },
            ConfinedEnvError::NoProxy(NoProxyError::Blank { index: 1 }),
        ),
        (
            "trailing comma",
            ConfinedEnvValues {
                no_proxy: Some("127.0.0.1,localhost,".to_owned()),
                ..values()
            },
            ConfinedEnvError::NoProxy(NoProxyError::Blank { index: 2 }),
        ),
        (
            "non-loopback entry",
            ConfinedEnvValues {
                no_proxy: Some("127.0.0.1,api.linear.app".to_owned()),
                ..values()
            },
            ConfinedEnvError::NoProxy(NoProxyError::NotLoopback { index: 1 }),
        ),
        (
            "wildcard entry",
            ConfinedEnvValues {
                no_proxy: Some("*".to_owned()),
                ..values()
            },
            ConfinedEnvError::NoProxy(NoProxyError::NotLoopback { index: 0 }),
        ),
        (
            "duplicate entry",
            ConfinedEnvValues {
                no_proxy: Some("127.0.0.1,localhost,localhost".to_owned()),
                ..values()
            },
            ConfinedEnvError::NoProxy(NoProxyError::Duplicate { index: 2 }),
        ),
    ];
    for (label, input, expected) in cases {
        let error = ConfinedTransportEnv::parse(&input).expect_err(label);
        assert_eq!(error, expected, "{label}");
        // Messages name the variable, never its value.
        let message = error.to_string();
        assert!(!message.contains("proxy.example"), "{label}: {message}");
        assert!(!message.contains("api.linear.app"), "{label}: {message}");
        assert!(!message.contains("certs/"), "{label}: {message}");
    }
    assert_eq!(
        ConfinedEnvError::Unexpected("HTTP_PROXY").to_string(),
        "HTTP_PROXY is set but the confined mode never honours it"
    );
    assert_eq!(
        ConfinedEnvError::NoProxy(NoProxyError::NotConfinedPolicy).to_string(),
        "NO_PROXY entries are not exactly the confined policy 127.0.0.1,localhost"
    );
}

// ---------------------------------------------------------------------------
// Asset URL type

#[test]
fn asset_url_accepts_only_the_two_fixed_hosts_over_https_and_hides_the_query() {
    let private = AssetUrl::parse(PRIVATE).expect("private");
    assert_eq!(private.host(), AssetHost::Uploads);
    assert!(private.host().authenticated());
    assert_eq!(private.url().as_str(), PRIVATE);
    assert_eq!(private.origin(), "https://uploads.linear.app");
    assert_eq!(private.to_string(), "https://uploads.linear.app");
    assert_eq!(
        format!("{private:?}"),
        "AssetUrl(\"https://uploads.linear.app\")"
    );
    let public = AssetUrl::parse(PUBLIC).expect("public");
    assert_eq!(public.host(), AssetHost::Public);
    assert!(!public.host().authenticated());
    assert_eq!(public.origin(), "https://public.linear.app");
    // Host case and the default port are normalized by the parser.
    let normalized = AssetUrl::parse("https://UPLOADS.linear.app:443/x").expect("normalized");
    assert_eq!(normalized.host(), AssetHost::Uploads);
    assert_eq!(normalized.url().as_str(), "https://uploads.linear.app/x");

    let cases = [
        (
            "http://uploads.linear.app/x",
            AssetUrlError::Scheme("http".to_owned()),
        ),
        (
            "https://api.linear.app/private.png",
            AssetUrlError::HostNotAllowed,
        ),
        (
            "https://uploads.linear.app.evil.example/x",
            AssetUrlError::HostNotAllowed,
        ),
        ("https://127.0.0.1/x", AssetUrlError::HostNotAllowed),
        ("https://uploads.linear.app:8443/x", AssetUrlError::Port),
        (
            "https://user:pw@uploads.linear.app/x",
            AssetUrlError::Credentials,
        ),
        ("https://uploads.linear.app/x#f", AssetUrlError::Fragment),
    ];
    for (text, expected) in cases {
        let error = AssetUrl::parse(text).expect_err(text);
        assert_eq!(error, expected, "{text}");
        assert!(!error.to_string().contains("evil"), "{text}");
    }
    assert!(matches!(
        AssetUrl::parse("not a url").expect_err("invalid"),
        AssetUrlError::Invalid(_)
    ));
    assert_eq!(
        AssetUrlError::HostNotAllowed.to_string(),
        "asset URL host is not a permitted fixed host"
    );
}

// ---------------------------------------------------------------------------
// Redirect resolver

#[test]
fn redirects_follow_only_same_origin_relative_paths_and_never_echo_location() {
    let current = AssetUrl::parse(PRIVATE).expect("private");
    let location = |text: &str| HeaderValue::from_str(text).expect("header");

    let next = resolve_redirect(&current, Some(&location("/private-final.png?token=fake")))
        .expect("same-origin relative");
    assert_eq!(next.host(), AssetHost::Uploads);
    assert_eq!(
        next.url().as_str(),
        "https://uploads.linear.app/private-final.png?token=fake"
    );
    // A relative path with dot segments still resolves within the origin.
    let dotted = resolve_redirect(&current, Some(&location("/a/../b.png"))).expect("dot segments");
    assert_eq!(dotted.url().as_str(), "https://uploads.linear.app/b.png");

    let rejected = [
        ("//evil.example/x", RedirectRejection::NotOriginRelative),
        (
            "https://uploads.linear.app/x",
            RedirectRejection::NotOriginRelative,
        ),
        (
            "https://public.linear.app/x",
            RedirectRejection::NotOriginRelative,
        ),
        ("relative.png", RedirectRejection::NotOriginRelative),
        ("", RedirectRejection::NotOriginRelative),
        ("/\\evil.example/x", RedirectRejection::MalformedLocation),
        ("/x#fragment", RedirectRejection::MalformedLocation),
    ];
    for (text, expected) in rejected {
        let error = resolve_redirect(&current, Some(&location(text))).expect_err(text);
        assert_eq!(error, expected, "{text:?}");
        let message = error.to_string();
        assert!(!message.contains("evil"), "{message}");
        assert!(!message.contains("/x"), "{message}");
    }
    let binary = HeaderValue::from_bytes(b"/\xff").expect("opaque header");
    assert_eq!(
        resolve_redirect(&current, Some(&binary)).expect_err("non-ascii"),
        RedirectRejection::MalformedLocation
    );
    assert_eq!(
        resolve_redirect(&current, None).expect_err("missing"),
        RedirectRejection::MissingLocation
    );

    // Non-redirect statuses are final responses, whatever they are.
    for status in [200, 204, 300, 304, 404, 500] {
        assert!(
            follow_redirect(
                &current,
                0,
                StatusCode::from_u16(status).expect("status"),
                Some(&location("//evil.example/x")),
            )
            .expect("final")
            .is_none(),
            "{status}"
        );
    }
    for status in [301, 302, 303, 307, 308] {
        let hop = follow_redirect(
            &current,
            MAX_REDIRECTS - 1,
            StatusCode::from_u16(status).expect("status"),
            Some(&location("/next.png")),
        )
        .expect("within budget")
        .expect("redirect");
        assert_eq!(hop.url().as_str(), "https://uploads.linear.app/next.png");
        assert_eq!(
            follow_redirect(
                &current,
                MAX_REDIRECTS,
                StatusCode::from_u16(status).expect("status"),
                Some(&location("/next.png")),
            )
            .expect_err("over budget"),
            RedirectRejection::TooManyRedirects {
                limit: MAX_REDIRECTS
            }
        );
    }
    let failure = AssetFailure::Redirect {
        origin: current.origin(),
        request: 2,
        rejection: RedirectRejection::CrossOrigin,
    };
    assert_eq!(
        failure.to_string(),
        "redirect from https://uploads.linear.app (request 2) was rejected: Location resolves to another origin"
    );
    assert!(std::error::Error::source(&failure).is_some());
    assert_no_secrets(&format!("{failure:?}"));
}

#[test]
fn asset_failure_messages_name_only_the_origin() {
    let too_large = AssetFailure::ResponseTooLarge {
        origin: "https://uploads.linear.app".to_owned(),
        status: StatusCode::OK,
        limit: ResponseCap::new(8).expect("cap"),
    };
    assert_eq!(
        too_large.to_string(),
        "response from https://uploads.linear.app exceeds the 8 byte limit (HTTP 200 OK)"
    );
    let timeout = AssetFailure::Timeout {
        origin: "https://public.linear.app".to_owned(),
        deadline: Deadline::new(Duration::from_millis(300)).expect("deadline"),
    };
    assert_eq!(
        timeout.to_string(),
        "request to https://public.linear.app did not complete within 300ms"
    );
}

// ---------------------------------------------------------------------------
// Shared client factory

#[test]
fn both_transports_share_the_factory_and_reject_the_same_bad_ca() {
    let dir = std::env::temp_dir().join(format!("f02b-gate2-ca-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let missing = dir.join("missing.pem");
    let bad = TransportConfig {
        ca: CaMode::PublicRootsPlusPem(missing.clone()),
        ..connect_config("http://127.0.0.1:1")
    };
    let endpoint = EndpointUrl::parse("http://127.0.0.1:1/graphql").expect("endpoint");
    let graphql = GraphQlTransport::new(endpoint.clone(), key(), bad.clone()).expect_err("graphql");
    let asset = AssetHttpTransport::new(key(), bad).expect_err("asset");
    for error in [&graphql, &asset] {
        assert!(
            matches!(error, TransportBuildError::CaRead { path, .. } if path == &missing),
            "{error:?}"
        );
    }
    let good = connect_config("http://127.0.0.1:1");
    GraphQlTransport::new(endpoint, key(), good.clone()).expect("graphql builds");
    let asset = AssetHttpTransport::new(key(), good).expect("asset builds");
    let debug = format!("{asset:?}");
    assert!(debug.contains("ApiKey(<redacted>)"), "{debug}");
    assert_no_secrets(&debug);
    std::fs::remove_dir_all(&dir).expect("cleanup");
}

// ---------------------------------------------------------------------------
// Loopback CONNECT stand-in (no TLS, no DNS)

struct StandInProxy {
    port: u16,
    handle: JoinHandle<String>,
}

impl StandInProxy {
    /// Accepts one connection, reads the request head, answers `status`
    /// with an empty body and closes. Returns the head it observed.
    fn start(status: &'static str) -> StandInProxy {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .expect("read timeout");
            let mut buffer = Vec::new();
            let mut chunk = [0_u8; 1024];
            loop {
                match stream.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => buffer.extend_from_slice(chunk.get(..n).expect("chunk")),
                    Err(error)
                        if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
                    {
                        break;
                    }
                    Err(error) => panic!("read: {error}"),
                }
                if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            write!(stream, "HTTP/1.1 {status}\r\nContent-Length: 0\r\n\r\n").expect("reply");
            stream.flush().expect("flush");
            String::from_utf8_lossy(&buffer).into_owned()
        });
        StandInProxy { port, handle }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn observed(self) -> String {
        self.handle.join().expect("proxy thread")
    }
}

#[tokio::test(flavor = "current_thread")]
async fn private_get_sends_exact_connect_bytes_to_the_loopback_proxy_and_leaks_nothing() {
    let proxy = StandInProxy::start("403 Forbidden");
    let transport =
        AssetHttpTransport::new(key(), connect_config(&proxy.url())).expect("transport");
    let url = AssetUrl::parse(PRIVATE).expect("private");
    let started = Instant::now();
    let failure = transport
        .get(&url)
        .await
        .expect_err("proxy refused the tunnel");
    assert!(started.elapsed() < Duration::from_secs(5));
    let head = proxy.observed();
    let (request_line, headers) = head.split_once("\r\n").expect("request line");
    assert_eq!(request_line, "CONNECT uploads.linear.app:443 HTTP/1.1");
    let lower = headers.to_ascii_lowercase();
    assert!(
        lower.contains("host: uploads.linear.app:443\r\n"),
        "{head:?}"
    );
    assert!(!lower.contains("authorization"), "{head:?}");
    assert_no_secrets(&head);

    let AssetFailure::Network { origin, phase, .. } = &failure else {
        panic!("expected Network, got {failure:?}");
    };
    assert_eq!(origin, "https://uploads.linear.app");
    assert_eq!(*phase, NetworkPhase::Connect);
    let display = failure.to_string();
    assert!(
        display.starts_with("connection to https://uploads.linear.app failed: "),
        "{display}"
    );
    let mut chain = Vec::new();
    let mut source: Option<&(dyn std::error::Error + 'static)> =
        std::error::Error::source(&failure);
    while let Some(error) = source {
        chain.push(error.to_string());
        source = error.source();
    }
    assert!(!chain.is_empty());
    for text in [display, format!("{failure:?}")].iter().chain(chain.iter()) {
        assert_no_secrets(text);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn public_get_connects_to_its_own_host_through_the_same_proxy() {
    let proxy = StandInProxy::start("403 Forbidden");
    let transport =
        AssetHttpTransport::new(key(), connect_config(&proxy.url())).expect("transport");
    let url = AssetUrl::parse(PUBLIC).expect("public");
    let failure = transport
        .get(&url)
        .await
        .expect_err("proxy refused the tunnel");
    let head = proxy.observed();
    assert!(
        head.starts_with("CONNECT public.linear.app:443 HTTP/1.1\r\n"),
        "{head:?}"
    );
    assert_no_secrets(&head);
    assert!(matches!(
        failure,
        AssetFailure::Network {
            phase: NetworkPhase::Connect,
            ..
        }
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn closed_proxy_port_fails_at_connect_within_the_deadline() {
    let closed = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = closed.local_addr().expect("addr").port();
    drop(closed);
    let transport =
        AssetHttpTransport::new(key(), connect_config(&format!("http://127.0.0.1:{port}")))
            .expect("transport");
    let url = AssetUrl::parse(PRIVATE).expect("private");
    let started = Instant::now();
    let failure = transport.get(&url).await.expect_err("nothing listens");
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(
        matches!(
            &failure,
            AssetFailure::Network {
                phase: NetworkPhase::Connect,
                ..
            }
        ),
        "{failure:?}"
    );
    let display = failure.to_string();
    assert!(
        display.starts_with("connection to https://uploads.linear.app failed: "),
        "{display}"
    );
    assert_no_secrets(&display);
}
