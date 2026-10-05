//! Image and attachment downloads, which have their own deadline and cap
//! instead of the GraphQL ones.

use std::num::NonZeroUsize;
use std::time::{Duration, Instant};

use super::server::{Reply, Server};
use super::{USER_AGENT_VALUE, client_for, config};
use crate::client::{ClientConfig, Deadline, LinearClient, ResponseCap};

/// A client whose GraphQL deadline and cap would fail any download.
fn strict_client() -> LinearClient {
    client_for(
        "http://127.0.0.1:9/graphql",
        config(Duration::from_millis(10), 1),
    )
}

fn bytes(body: &[u8]) -> Reply {
    Reply::status(200, "application/octet-stream", body)
}

#[tokio::test(flavor = "current_thread")]
async fn downloads_ignore_the_graphql_deadline_and_cap_and_send_no_api_key() {
    let server = Server::start(vec![
        bytes(&[0, 255, 7]).delayed(Duration::from_millis(50)),
        bytes(&[0, 255, 7]).delayed(Duration::from_millis(50)),
    ]);
    let url = server.url("/image");
    let client = strict_client();
    assert_eq!(
        client.download_markdown_image(&url).await.expect("image"),
        [0, 255, 7]
    );
    assert_eq!(
        client
            .download_issue_attachment(&url)
            .await
            .expect("attachment"),
        [0, 255, 7]
    );
    for request in server.finish() {
        assert_eq!(
            (request.method.as_str(), request.path.as_str()),
            ("GET", "/image")
        );
        assert_eq!(request.header("user-agent"), Some(USER_AGENT_VALUE));
        assert!(
            request
                .header("accept-encoding")
                .is_some_and(|value| value.contains("gzip"))
        );
        assert_eq!(request.header("authorization"), None);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn failures_report_the_http_status_with_a_per_kind_prefix() {
    let server = Server::start(vec![
        Reply::status(500, "text/plain", ""),
        Reply::status(500, "text/plain", ""),
    ]);
    let url = server.url("/image");
    let client = strict_client();
    assert_eq!(
        client
            .download_markdown_image(&url)
            .await
            .expect_err("500")
            .message(),
        "Failed to download image: 500 Internal Server Error"
    );
    assert_eq!(
        client
            .download_issue_attachment(&url)
            .await
            .expect_err("500")
            .message(),
        "Failed to download: 500 Internal Server Error"
    );
    server.finish();
}

#[tokio::test(flavor = "current_thread")]
async fn only_http_urls_are_downloaded() {
    let client = strict_client();
    for (url, scheme) in [
        ("data:application/octet-stream;base64,AP8H", "data"),
        ("file:///work/local.bin", "file"),
    ] {
        assert_eq!(
            client
                .download_markdown_image(url)
                .await
                .expect_err(url)
                .message(),
            format!("Failed to download image: unsupported URL scheme '{scheme}'")
        );
    }
    for url in ["foo.png", "http://[bad"] {
        assert_eq!(
            client
                .download_markdown_image(url)
                .await
                .expect_err(url)
                .message(),
            format!("Invalid URL: '{url}'")
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn url_userinfo_is_sent_as_basic_auth_instead_of_the_api_key() {
    let server = Server::start(vec![bytes(b"FAKE")]);
    let url = server
        .url("/image")
        .replacen("http://", "http://fake-user:fake-password@", 1);
    assert_eq!(
        strict_client()
            .download_markdown_image(&url)
            .await
            .expect("image"),
        b"FAKE"
    );
    let requests = server.finish();
    let [request] = requests.as_slice() else {
        panic!("{requests:?}");
    };
    assert_eq!(request.headers_named("authorization"), 1);
    assert_eq!(
        request.header("authorization"),
        Some("Basic ZmFrZS11c2VyOmZha2UtcGFzc3dvcmQ=")
    );
}

#[tokio::test(flavor = "current_thread")]
async fn downloads_follow_twenty_redirects_and_refuse_the_twenty_first() {
    let hops = |redirects: usize| {
        (0..=redirects)
            .map(|index| {
                if index < redirects {
                    Reply::status(302, "text/plain", "")
                        .header("location", &format!("/hop/{}", index + 1))
                } else {
                    bytes(b"FAKE")
                }
            })
            .collect::<Vec<_>>()
    };
    let client = strict_client();

    let server = Server::start(hops(20));
    assert_eq!(
        client
            .download_issue_attachment(&server.url("/hop/0"))
            .await
            .expect("twenty redirects"),
        b"FAKE"
    );
    let requests = server.finish();
    assert_eq!(requests.len(), 21);
    assert!(
        requests
            .iter()
            .enumerate()
            .all(|(index, request)| request.path == format!("/hop/{index}"))
    );

    let server = Server::start(hops(21));
    let message = client
        .download_markdown_image(&server.url("/hop/0"))
        .await
        .expect_err("too many redirects")
        .message()
        .to_owned();
    assert!(message.starts_with("Failed to download image"), "{message}");
    assert!(message.contains("redirect"), "{message}");
    assert_eq!(server.finish().len(), 21);
}

/// A client whose downloads stop after `cap` bytes or `deadline`.
fn download_client(deadline: Duration, cap: usize) -> LinearClient {
    client_for(
        "http://127.0.0.1:9/graphql",
        ClientConfig {
            download_deadline: Deadline(deadline),
            max_download_bytes: ResponseCap(NonZeroUsize::new(cap).expect("nonzero cap")),
            ..ClientConfig::default()
        },
    )
}

#[tokio::test(flavor = "current_thread")]
async fn downloads_stop_at_the_download_cap_and_deadline() {
    let server = Server::start(vec![bytes(&[7; 8192]), Reply::Stall]);
    let url = server.url("/image");
    let client = download_client(Duration::from_millis(300), 4096);
    let error = client
        .download_markdown_image(&url)
        .await
        .expect_err("too large");
    assert_eq!(
        error.message(),
        "Failed to download image: response exceeds the 4096 byte limit"
    );
    let started = Instant::now();
    let error = client
        .download_issue_attachment(&url)
        .await
        .expect_err("timeout");
    assert_eq!(
        error.message(),
        "Failed to download: did not complete within 300ms"
    );
    assert!(started.elapsed() < Duration::from_secs(30));
    server.finish();
}
