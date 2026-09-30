//! Public GET behavior, independent of the GraphQL/asset cap and deadline.
use linear_cli::graphql::transport::{
    ApiKey, CaMode, Deadline, EndpointUrl, GraphQlTransport, ProxyMode, ResponseCap,
    TransportConfig,
};
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::Duration,
};

fn transport(endpoint: &str) -> GraphQlTransport {
    GraphQlTransport::new(
        EndpointUrl::parse(endpoint).unwrap(),
        ApiKey::new("lin_api_fake".into()).unwrap(),
        TransportConfig {
            proxy: ProxyMode::Direct,
            ca: CaMode::PublicRoots,
            deadline: Deadline::new(Duration::from_millis(10)).unwrap(),
            max_response_bytes: ResponseCap::new(1).unwrap(),
        },
    )
    .unwrap()
}
fn serve(
    status: &'static str,
    body: Vec<u8>,
    delay: Duration,
) -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/image", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut headers = Vec::new();
        while !headers.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap();
            headers.push(byte[0]);
        }
        thread::sleep(delay);
        write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .unwrap();
        stream.write_all(&body).unwrap();
        String::from_utf8(headers).unwrap()
    });
    (url, server)
}

#[tokio::test]
async fn generic_image_client_has_no_total_deadline_cap_or_automatic_request_headers() {
    let (url, server) = serve("200 OK", vec![0, 255, 7], Duration::from_millis(50));
    let client = transport(&url);
    assert_eq!(
        client.download_markdown_image(&url).await.unwrap(),
        [0, 255, 7]
    );
    let headers = server.join().unwrap().to_ascii_lowercase();
    assert!(headers.starts_with("get /image http/1.1\r\n"));
    assert!(!headers.contains("user-agent:"));
    assert!(!headers.contains("accept-encoding:"));
    assert!(!headers.contains("authorization:"));
}

#[tokio::test]
async fn image_failure_preserves_noncanonical_http_reason_phrase() {
    let (url, server) = serve("500 Fixture download failed", Vec::new(), Duration::ZERO);
    let error = transport(&url)
        .download_markdown_image(&url)
        .await
        .unwrap_err();
    assert_eq!(
        error.message,
        "Failed to download image: 500 Fixture download failed"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn data_url_binary_and_percent_decoding_and_missing_file_failure_are_explicit() {
    let client = transport("http://127.0.0.1:9/graphql");
    assert_eq!(
        client
            .download_markdown_image("data:application/octet-stream;base64,AP8H")
            .await
            .unwrap(),
        [0, 255, 7]
    );
    assert_eq!(
        client
            .download_markdown_image("data:text/plain,a%20b%00")
            .await
            .unwrap(),
        b"a b\0"
    );
    assert_eq!(
        client
            .download_markdown_image("file:///work/local.bin")
            .await
            .unwrap_err()
            .message,
        "NetworkError when attempting to fetch resource"
    );
    assert_eq!(
        client
            .download_markdown_image("foo.png")
            .await
            .unwrap_err()
            .message,
        "Invalid URL: 'foo.png'"
    );
    assert_eq!(
        client
            .download_markdown_image("http://[bad")
            .await
            .unwrap_err()
            .message,
        "Invalid URL: 'http://[bad'"
    );
}

// Pinned Deno2.7.9 ordinary fetch accepts URL userinfo and emits Basic, including
// cross-origin redirect userinfo. Keep physical source effects, not a browser assumption.
#[tokio::test]
async fn initial_userinfo_emits_source_basic_header_and_receives_exact_bytes() {
    let (url, server) = serve("200 OK", b"FAKE".to_vec(), Duration::ZERO);
    let credentials_url = url.replacen("http://", "http://fake-user:fake-password@", 1);
    assert_eq!(
        transport(&url)
            .download_markdown_image(&credentials_url)
            .await
            .unwrap(),
        b"FAKE"
    );
    let headers = server.join().unwrap();
    let authorization = headers
        .lines()
        .filter(|line| line.to_ascii_lowercase().starts_with("authorization:"))
        .collect::<Vec<_>>();
    assert_eq!(authorization.len(), 1);
    assert_eq!(
        authorization[0].split_once(':').unwrap().1.trim(),
        "Basic ZmFrZS11c2VyOmZha2UtcGFzc3dvcmQ="
    );
    assert!(!headers.contains("lin_api_fake"));
}

#[tokio::test]
async fn cross_origin_userinfo_redirect_matches_source_basic_without_cli_auth() {
    let (target, target_server) = serve("200 OK", b"FAKE".to_vec(), Duration::ZERO);
    let target = target.replacen("http://", "http://fake-user:fake-password@", 1);
    let first = TcpListener::bind("127.0.0.1:0").unwrap();
    let initial = format!("http://{}/redirect", first.local_addr().unwrap());
    let initial_server = thread::spawn(move || {
        let (mut stream, _) = first.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut headers = Vec::new();
        while !headers.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap();
            headers.push(byte[0]);
        }
        write!(stream, "HTTP/1.1 302 Found\r\nLocation: {target}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
        String::from_utf8(headers).unwrap()
    });
    assert_eq!(
        transport(&initial)
            .download_markdown_image(&initial)
            .await
            .unwrap(),
        b"FAKE"
    );
    let first_headers = initial_server.join().unwrap();
    assert!(
        !first_headers
            .to_ascii_lowercase()
            .contains("authorization:")
    );
    let final_headers = target_server.join().unwrap();
    let authorization = final_headers
        .lines()
        .filter(|line| line.to_ascii_lowercase().starts_with("authorization:"))
        .collect::<Vec<_>>();
    assert_eq!(authorization.len(), 1);
    assert_eq!(
        authorization[0].split_once(':').unwrap().1.trim(),
        "Basic ZmFrZS11c2VyOmZha2UtcGFzc3dvcmQ="
    );
    assert!(!final_headers.contains("lin_api_fake"));
}

#[tokio::test]
async fn readable_file_url_decodes_path_and_returns_exact_bytes_while_missing_file_fails() {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let private = std::env::temp_dir().join(format!(
        "linear-markdown-file-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&private).unwrap();
    let file = private.join("readable fake.bin");
    std::fs::write(&file, b"FAKE\0\xff\n").unwrap();
    let url = reqwest::Url::from_file_path(&file).unwrap();
    assert!(url.as_str().contains("%20"));
    let client = transport("http://127.0.0.1:9/graphql");
    assert_eq!(
        client.download_markdown_image(url.as_str()).await.unwrap(),
        b"FAKE\0\xff\n"
    );
    std::fs::remove_file(&file).unwrap();
    assert_eq!(
        client
            .download_markdown_image(url.as_str())
            .await
            .unwrap_err()
            .message,
        "NetworkError when attempting to fetch resource"
    );
    std::fs::remove_dir(&private).unwrap();
}
