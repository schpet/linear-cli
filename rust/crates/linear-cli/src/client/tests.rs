//! Client behavior against loopback servers: request bytes, response
//! classification, size caps, deadlines, cancellation and redaction.

mod download;
mod server;

use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use cynic::QueryBuilder;
use reqwest::StatusCode;
use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::{Map, Value, json};

use self::server::{Reply, Server};
use super::config::{EndpointUrlError, USER_AGENT_VALUE};
use super::error::NetworkPhase;
use super::{
    ApiKey, ApiKeyError, CONTENT_TYPE_VALUE, ClientBuildError, ClientConfig, Deadline, EndpointUrl,
    HttpBodyShape, LinearClient, RawHttpResponse, RequestError, ResponseCap, classify_typed,
};
use crate::graphql::envelope::{GraphQlRequest, ResponseError};
use crate::graphql::operations::teams::{GetTeams, GetTeamsVariables};

const FAKE_KEY: &str = "lin_api_fake";

fn tls_fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/tls")
        .join(name)
}

fn config(deadline: Duration, cap: usize) -> ClientConfig {
    ClientConfig {
        ca_bundle: None,
        deadline: Deadline(deadline),
        max_response_bytes: ResponseCap(NonZeroUsize::new(cap).expect("nonzero cap")),
    }
}

fn client_for(endpoint: &str, config: ClientConfig) -> LinearClient {
    LinearClient::new(
        EndpointUrl::parse(endpoint).expect("endpoint"),
        ApiKey::new(FAKE_KEY.to_owned()).expect("fake key"),
        config,
    )
    .expect("client")
}

/// A client for `server` with a generous deadline and cap.
fn client_to(server: &Server) -> LinearClient {
    client_for(
        &server.url("/graphql"),
        config(Duration::from_secs(10), 1024 * 1024),
    )
}

fn raw(document: &str, variables: Option<Map<String, Value>>) -> GraphQlRequest {
    GraphQlRequest {
        query: document.to_owned(),
        variables: variables.map(Value::Object),
        operation_name: None,
    }
}

fn teams_variables() -> GetTeamsVariables {
    GetTeamsVariables {
        filter: None,
        first: Some(100),
        after: None,
    }
}

fn teams_request() -> GraphQlRequest {
    GraphQlRequest::new(GetTeams::build(teams_variables())).expect("variables serialize")
}

fn teams_data() -> Value {
    json!({"teams": {
        "nodes": [{
            "id": "team-1",
            "name": "Engineering",
            "key": "ENG",
            "description": null,
            "icon": null,
            "color": "#0000ff",
            "cyclesEnabled": false,
            "createdAt": "2026-01-01T00:00:00.000Z",
            "updatedAt": "2026-01-02T00:00:00.000Z",
            "archivedAt": null,
            "organization": {"id": "org-1", "name": "Acme"}
        }],
        "pageInfo": {"hasNextPage": false, "endCursor": null}
    }})
}

#[test]
fn endpoint_url_keeps_path_and_query_but_displays_only_the_origin() {
    assert_eq!(
        EndpointUrl::parse("https://api.linear.app/graphql?sig=SECRET#frag")
            .expect_err("fragment rejected"),
        EndpointUrlError::Fragment
    );
    let endpoint = EndpointUrl::parse("https://api.linear.app/graphql?sig=SECRET").expect("ok");
    assert_eq!(
        endpoint.url().as_str(),
        "https://api.linear.app/graphql?sig=SECRET"
    );
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
fn ca_bundle_must_be_a_readable_pem_file_with_certificates() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let build = |path: PathBuf| {
        LinearClient::new(
            EndpointUrl::parse("https://uploads.linear.app/x").expect("endpoint"),
            ApiKey::new(FAKE_KEY.to_owned()).expect("fake key"),
            ClientConfig {
                ca_bundle: Some(path),
                ..ClientConfig::default()
            },
        )
    };
    build(tls_fixture("test-ca.pem")).expect("valid bundle");
    assert!(matches!(
        build(dir.path().join("missing.pem")).expect_err("missing"),
        ClientBuildError::CaRead { .. }
    ));
    assert!(matches!(
        build(dir.path().to_owned()).expect_err("directory"),
        ClientBuildError::CaRead { .. }
    ));
    for (name, contents) in [("empty.pem", " \n"), ("garbage.pem", "not a certificate")] {
        let path = dir.path().join(name);
        std::fs::write(&path, contents).expect("write");
        assert!(matches!(
            build(path).expect_err(name),
            ClientBuildError::CaEmpty { .. }
        ));
    }
    let bogus = dir.path().join("bogus.pem");
    std::fs::write(
        &bogus,
        "-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----\n",
    )
    .expect("write");
    let error = build(bogus).expect_err("bad DER");
    assert!(
        matches!(error, ClientBuildError::CaInvalid { .. }),
        "{error:?}"
    );
    let app = crate::error::Error::from(error);
    assert!(
        app.to_string().starts_with("SSL_CERT_FILE: CA bundle "),
        "{app}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn network_failures_never_expose_the_path_query_or_api_key() {
    // Nothing listens on port 1, so the connection is refused.
    let endpoint = EndpointUrl::parse("http://127.0.0.1:1/graphql?signature=SIGNED-SECRET-TOKEN")
        .expect("endpoint");
    let key = ApiKey::new("lin_api_SECRET_KEY_VALUE".to_owned()).expect("key");
    let client =
        LinearClient::new(endpoint, key, config(Duration::from_secs(5), 1024)).expect("client");
    assert!(!format!("{client:?}").contains("SECRET"));
    let failure = client
        .send_request(&raw("{ viewer { id } }", None))
        .await
        .expect_err("connection refused");
    let RequestError::Network { origin, phase, .. } = &failure else {
        panic!("expected Network, got {failure:?}");
    };
    assert_eq!(origin, "http://127.0.0.1:1");
    // Whether reqwest reports this as a connection or a request failure
    // varies; both get the same redaction.
    let action = match phase {
        NetworkPhase::Connect => "connection to",
        NetworkPhase::Request => "request to",
        NetworkPhase::Body | NetworkPhase::Other => {
            panic!("expected connect/request failure, got {failure:?}");
        }
    };
    let display = failure.to_string();
    assert!(
        display.starts_with(&format!("{action} http://127.0.0.1:1 failed: ")),
        "{display}"
    );
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
    for text in [display, format!("{failure:?}")].iter().chain(&chain) {
        for secret in ["SIGNED-SECRET-TOKEN", "signature", "SECRET_KEY", "/graphql"] {
            assert!(!text.contains(secret), "{secret} leaked into {text}");
        }
    }
    let app: crate::error::Error = failure.into();
    assert!(std::error::Error::source(&app).is_some());
}

#[tokio::test(flavor = "current_thread")]
async fn request_carries_exact_headers_and_envelope_bytes() {
    let server = Server::start(vec![Reply::status(200, "application/json", "")]);
    let mut variables = Map::new();
    variables.insert("after".to_owned(), Value::Null);
    let request = GraphQlRequest {
        operation_name: Some("Q".to_owned()),
        ..raw("query($after: String) { x }", Some(variables))
    };
    client_to(&server)
        .send_request(&request)
        .await
        .expect("empty 200");
    let requests = server.finish();
    let [request] = requests.as_slice() else {
        panic!("{requests:?}");
    };
    assert_eq!(
        (request.method.as_str(), request.path.as_str()),
        ("POST", "/graphql")
    );
    assert_eq!(request.header("authorization"), Some(FAKE_KEY));
    assert_eq!(request.headers_named("authorization"), 1);
    assert_eq!(request.header("user-agent"), Some(USER_AGENT_VALUE));
    assert_eq!(request.header("content-type"), Some(CONTENT_TYPE_VALUE));
    assert_eq!(request.header("referer"), None);
    assert_eq!(
        String::from_utf8_lossy(&request.body),
        r#"{"query":"query($after: String) { x }","variables":{"after":null},"operationName":"Q"}"#
    );
}

#[tokio::test(flavor = "current_thread")]
async fn declared_oversized_body_is_rejected_before_reading() {
    let server = Server::start(vec![Reply::status(
        200,
        "application/json",
        vec![b'x'; 4096],
    )]);
    let failure = client_for(
        &server.url("/graphql"),
        config(Duration::from_secs(5), 1024),
    )
    .send_request(&raw("{ x }", None))
    .await
    .expect_err("too large");
    assert!(
        matches!(&failure, RequestError::ResponseTooLarge { status, limit }
            if status.as_u16() == 200 && limit.bytes() == 1024),
        "{failure:?}"
    );
    assert_eq!(
        failure.to_string(),
        "response body exceeds the 1024 byte limit (HTTP 200 OK)"
    );
    server.finish();
}

#[tokio::test(flavor = "current_thread")]
async fn body_exactly_at_the_cap_is_kept_intact() {
    let server = Server::start(vec![Reply::status(
        502,
        "application/json",
        vec![b'x'; 1024],
    )]);
    let response = client_for(
        &server.url("/graphql"),
        config(Duration::from_secs(5), 1024),
    )
    .send_request(&raw("{ x }", None))
    .await
    .expect("at cap");
    assert_eq!(response.status.as_u16(), 502);
    assert_eq!(response.body, vec![b'x'; 1024]);
    assert_eq!(
        format!("{response:?}"),
        "RawHttpResponse { status: 502, headers: <3 headers>, body: <1024 bytes> }"
    );
    server.finish();
}

#[tokio::test(flavor = "current_thread")]
async fn endless_chunked_body_stops_at_the_cap() {
    let server = Server::start(vec![Reply::EndlessChunks]);
    let client = client_for(
        &server.url("/graphql"),
        config(Duration::from_secs(5), 2048),
    );
    let started = Instant::now();
    let failure = client
        .send_request(&raw("{ x }", None))
        .await
        .expect_err("too large");
    assert!(
        matches!(failure, RequestError::ResponseTooLarge { .. }),
        "{failure:?}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "stopped before the deadline"
    );
    drop(client);
    assert_eq!(server.finish().len(), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn silent_server_hits_the_total_deadline() {
    let server = Server::start(vec![Reply::Silent]);
    let started = Instant::now();
    let failure = client_for(
        &server.url("/graphql"),
        config(Duration::from_millis(300), 1024),
    )
    .send_request(&raw("{ x }", None))
    .await
    .expect_err("timeout");
    let elapsed = started.elapsed();
    assert!(
        matches!(failure, RequestError::Timeout { .. }),
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
            server.port()
        )
    );
    server.finish();
}

#[tokio::test(flavor = "current_thread")]
async fn stalled_body_hits_the_total_deadline_without_partial_data() {
    let server = Server::start(vec![Reply::Stall]);
    let started = Instant::now();
    let failure = client_for(
        &server.url("/graphql"),
        config(Duration::from_millis(300), 4096),
    )
    .send_request(&raw("{ x }", None))
    .await
    .expect_err("timeout");
    assert!(
        matches!(failure, RequestError::Timeout { .. }),
        "{failure:?}"
    );
    assert!(started.elapsed() < Duration::from_secs(3));
    server.finish();
}

#[test]
fn cancelled_request_completes_promptly_and_releases_the_connection() {
    let server = Server::start(vec![Reply::Stall]);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let client = client_for(
        &server.url("/graphql"),
        config(Duration::from_secs(30), 4096),
    );
    let started = Instant::now();
    runtime.block_on(async {
        let cancelled = tokio::time::timeout(
            Duration::from_millis(150),
            client.send_request(&raw("{ x }", None)),
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
        drop(client);
        // A current-thread runtime only drives the connection's shutdown
        // while it runs, so wait for the release here.
        let window = Instant::now();
        while window.elapsed() < Duration::from_secs(2) && !server.is_done() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });
    drop(runtime);
    let window = Instant::now();
    while window.elapsed() < Duration::from_secs(2) && !server.is_done() {
        std::thread::sleep(Duration::from_millis(10));
    }
    let requests = server.finish();
    assert!(
        requests
            .first()
            .is_some_and(|request| request.client_closed),
        "server saw the client close the connection: {requests:?}"
    );
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
async fn graphql_errors_classify_ahead_of_http_status() {
    let internal = json!({
        "message": "Something went wrong",
        "extensions": {
            "code": "INTERNAL_ERROR",
            "userPresentableMessage": "Something went wrong. Please try again."
        }
    });
    let unauthenticated = r#"{"errors":[{"message":"Authentication required, not authenticated","extensions":{"code":"AUTHENTICATION_ERROR"}}]}"#;
    let server = Server::start(vec![
        Reply::json(&json!({"data": null, "errors": [internal]})),
        Reply::json(&json!({"data": teams_data(), "errors": [internal]})),
        Reply::status(
            400,
            "application/json",
            json!({"data": null, "errors": [{
                "message": "Argument Validation Error",
                "path": ["teams"],
                "locations": [{"line": 2, "column": 3}],
                "extensions": {"userPresentableMessage": "The request was invalid."}
            }]})
            .to_string(),
        ),
        Reply::status(401, "application/json", unauthenticated),
    ]);
    let client = client_to(&server);
    let request = teams_request();

    let failure = client
        .query::<GetTeams, _>(teams_variables())
        .await
        .expect_err("errors only");
    let RequestError::GraphQl {
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

    let failure = client
        .query::<GetTeams, _>(teams_variables())
        .await
        .expect_err("partial data");
    assert!(
        matches!(
            &failure,
            RequestError::GraphQl { status, partial_data: true, .. } if status.as_u16() == 200
        ),
        "partial data is never returned as success: {failure:?}"
    );

    let failure = client
        .query::<GetTeams, _>(teams_variables())
        .await
        .expect_err("400 errors");
    let RequestError::GraphQl {
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
    assert_eq!(errors[0].path.as_ref().map(Vec::len), Some(1));
    assert!(errors[0].locations.is_some());
    assert_eq!(failure.to_string(), "The request was invalid.");

    let raw = client
        .send_request(&request)
        .await
        .expect("401 bytes captured");
    assert_eq!(raw.status.as_u16(), 401);
    assert_eq!(raw.body, unauthenticated.as_bytes());
    let failure = classify_typed::<GetTeams>(raw).expect_err("401 with errors");
    assert!(
        matches!(&failure, RequestError::GraphQl { status, errors, .. }
            if status.as_u16() == 401
                && errors[0].message == "Authentication required, not authenticated"),
        "{failure:?}"
    );

    assert_eq!(server.finish().len(), 4);
}

#[tokio::test(flavor = "current_thread")]
async fn http_failures_keep_raw_bytes_and_bad_bodies_classify_separately() {
    let data = json!({"data": teams_data()}).to_string();
    let server = Server::start(vec![
        Reply::status(429, "text/plain", "Too Many Requests\n").header("retry-after", "7"),
        Reply::status(502, "text/html", "<html><body>bad gateway</body></html>"),
        Reply::status(500, "application/json", data),
        Reply::status(200, "text/html", "<html>maintenance</html>"),
        Reply::status(200, "application/json", r#"{"data":"#),
        Reply::status(200, "application/json", ""),
        Reply::status(200, "application/json", r#"{"data":null}"#),
        Reply::status(200, "application/json", r#"{"data":{"teams":"nope"}}"#),
    ]);
    let client = client_to(&server);
    let next = || client.query::<GetTeams, _>(teams_variables());

    let failure = next().await.expect_err("429");
    let RequestError::Http { response, body } = &failure else {
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
    assert!(
        matches!(&failure, RequestError::Http { response, body: HttpBodyShape::Unusable(ResponseError::NotJson { .. }) }
            if response.status.as_u16() == 502
                && response.body == b"<html><body>bad gateway</body></html>"),
        "{failure:?}"
    );

    let failure = next().await.expect_err("500 with data");
    assert!(
        matches!(
            &failure,
            RequestError::Http { response, body: HttpBodyShape::Data }
                if response.status.as_u16() == 500
        ),
        "valid data under 500 is still an HTTP failure: {failure:?}"
    );

    let failure = next().await.expect_err("HTML under 200");
    assert!(
        matches!(
            &failure,
            RequestError::Response(ResponseError::NotJson { .. })
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
                RequestError::Response(ResponseError::MalformedJson(_))
            ),
            "{case}: {failure:?}"
        );
    }

    let failure = next().await.expect_err("data null");
    assert!(
        matches!(&failure, RequestError::Response(ResponseError::MissingData)),
        "{failure:?}"
    );

    let failure = next().await.expect_err("wrong shape");
    assert!(
        matches!(
            &failure,
            RequestError::Response(ResponseError::UnexpectedShape(_))
        ),
        "{failure:?}"
    );

    assert_eq!(server.finish().len(), 8, "no retries");
}

#[tokio::test(flavor = "current_thread")]
async fn temporary_redirect_replays_the_request_at_the_new_location() {
    let server = Server::start(vec![
        Reply::status(307, "text/plain", "").header("location", "/moved"),
        Reply::json(&json!({"data": teams_data()})),
    ]);
    let teams: GetTeams = client_to(&server)
        .query(teams_variables())
        .await
        .expect("redirect followed");
    assert_eq!(teams.teams.nodes.len(), 1);
    let requests = server.finish();
    let [first, second] = requests.as_slice() else {
        panic!("{requests:?}");
    };
    assert_eq!(
        (second.method.as_str(), second.path.as_str()),
        ("POST", "/moved")
    );
    assert_eq!(second.body, first.body);
    assert_eq!(second.header("authorization"), Some(FAKE_KEY));
}

#[tokio::test(flavor = "current_thread")]
async fn raw_document_without_variables_returns_exact_bytes() {
    let server = Server::start(vec![Reply::json(
        &json!({"data": {"viewer": {"id": "user-1"}}}),
    )]);
    let response = client_to(&server)
        .send_request(&raw("{ viewer { id } }", None))
        .await
        .expect("200");
    assert_eq!(response.status.as_u16(), 200);
    assert_eq!(response.body, br#"{"data":{"viewer":{"id":"user-1"}}}"#);
    let value: Value = classify_typed(response).expect("typed as Value");
    assert_eq!(value, json!({"viewer": {"id": "user-1"}}));
    let requests = server.finish();
    assert_eq!(
        requests.first().map(|request| request.body.as_slice()),
        Some(br#"{"query":"{ viewer { id } }"}"#.as_slice())
    );
}
