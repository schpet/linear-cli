//! Image and attachment downloads, independent of the GraphQL cap and deadline.
use linear_cli::graphql::transport::{
    ApiKey, Deadline, EndpointUrl, GraphQlTransport, ResponseCap, TransportConfig, USER_AGENT_VALUE,
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
            ca_bundle: None,
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
async fn image_download_has_no_deadline_or_cap_and_sends_no_api_key() {
    let (url, server) = serve("200 OK", vec![0, 255, 7], Duration::from_millis(50));
    let client = transport(&url);
    assert_eq!(
        client.download_markdown_image(&url).await.unwrap(),
        [0, 255, 7]
    );
    let headers = server.join().unwrap().to_ascii_lowercase();
    assert!(headers.starts_with("get /image http/1.1\r\n"));
    assert!(headers.contains(&format!("user-agent: {USER_AGENT_VALUE}")));
    assert!(!headers.contains("accept-encoding:"));
    assert!(!headers.contains("authorization:"));
}

#[tokio::test]
async fn image_failure_reports_the_http_status() {
    let (url, server) = serve("500 Fixture download failed", Vec::new(), Duration::ZERO);
    let error = transport(&url)
        .download_markdown_image(&url)
        .await
        .unwrap_err();
    assert_eq!(
        error.message,
        "Failed to download image: 500 Internal Server Error"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn only_http_urls_are_downloaded() {
    let client = transport("http://127.0.0.1:9/graphql");
    for (url, scheme) in [
        ("data:application/octet-stream;base64,AP8H", "data"),
        ("file:///work/local.bin", "file"),
    ] {
        assert_eq!(
            client
                .download_markdown_image(url)
                .await
                .unwrap_err()
                .message,
            format!("Failed to download image: unsupported URL scheme '{scheme}'")
        );
    }
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

#[tokio::test]
async fn url_userinfo_is_sent_as_basic_auth_instead_of_the_api_key() {
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
async fn issue_attachment_download_uses_its_own_error_prefix() {
    let (url, server) = serve("200 OK", vec![0, 255, 7], Duration::from_millis(50));
    assert_eq!(
        transport(&url)
            .download_issue_attachment(&url)
            .await
            .unwrap(),
        [0, 255, 7]
    );
    let request = server.join().unwrap().to_ascii_lowercase();
    assert!(!request.contains("accept-encoding:") && !request.contains("authorization:"));
    let (url, server) = serve("500 Fixture download failed", vec![], Duration::ZERO);
    assert_eq!(
        transport(&url)
            .download_issue_attachment(&url)
            .await
            .unwrap_err()
            .message,
        "Failed to download: 500 Internal Server Error"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn downloads_follow_twenty_redirects_and_refuse_the_twenty_first() {
    for attachment in [false, true] {
        for redirects in [20, 21] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let url = format!("{base}/hop/0");
            let worker = thread::spawn(move || {
                for index in 0..=20 {
                    let (mut stream, _) = listener.accept().unwrap();
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    let mut request = Vec::new();
                    while !request.ends_with(b"\r\n\r\n") {
                        let mut byte = [0];
                        stream.read_exact(&mut byte).unwrap();
                        request.push(byte[0]);
                    }
                    let request = String::from_utf8(request).unwrap();
                    assert!(request.starts_with(&format!("GET /hop/{index} HTTP/1.1\r\n")));
                    if index < redirects {
                        write!(stream,"HTTP/1.1 302 Found\r\nLocation: {base}/hop/{}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",index+1).unwrap();
                    } else {
                        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\nFAKE").unwrap();
                    }
                }
            });
            let client = transport(&url);
            let result = if attachment {
                client.download_issue_attachment(&url).await
            } else {
                client.download_markdown_image(&url).await
            };
            if redirects == 20 {
                assert_eq!(result.unwrap(), b"FAKE");
            } else {
                let message = result.unwrap_err().message;
                assert!(message.starts_with("Failed to download"), "{message}");
                assert!(message.contains("redirect"), "{message}");
            }
            worker.join().unwrap();
        }
    }
}
