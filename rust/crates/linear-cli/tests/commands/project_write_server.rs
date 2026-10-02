//! Bounded loopback fixture shared by public command tests.
use linear_cli::graphql::transport::{
    ApiKey, Deadline, EndpointUrl, GraphQlTransport, ResponseCap, TransportConfig,
};
use serde_json::Value;
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::{Duration, Instant},
};
pub fn serve_with_content_types(
    replies: Vec<(Option<&'static str>, String)>,
) -> (GraphQlTransport, thread::JoinHandle<Vec<Value>>) {
    let count = replies.len();
    let mut replies = replies.into_iter();
    serve_responses(count, move |_| {
        replies.next().expect("one reply per expected request")
    })
}
pub fn serve_responses(
    count: usize,
    mut reply: impl FnMut(&Value) -> (Option<&'static str>, String) + Send + 'static,
) -> (GraphQlTransport, thread::JoinHandle<Vec<Value>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let thread = thread::spawn(move || {
        let mut requests = Vec::new();
        for _ in 0..count {
            let start = Instant::now();
            let mut socket = loop {
                match listener.accept() {
                    Ok((socket, _)) => break socket,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            start.elapsed() < Duration::from_secs(4),
                            "expected request did not arrive"
                        );
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(error) => panic!("fixture accept failed: {error}"),
                }
            };
            socket
                .set_nonblocking(false)
                .expect("blocking accepted mock stream");
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut bytes = Vec::new();
            let boundary = loop {
                let mut byte = [0];
                socket.read_exact(&mut byte).unwrap();
                bytes.push(byte[0]);
                if bytes.ends_with(b"\r\n\r\n") {
                    break bytes.len();
                }
            };
            let header = std::str::from_utf8(&bytes).unwrap();
            let length = header
                .lines()
                .find_map(|line| {
                    line.split_once(':').and_then(|(name, value)| {
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().unwrap())
                    })
                })
                .unwrap();
            bytes.resize(boundary + length, 0);
            socket.read_exact(&mut bytes[boundary..]).unwrap();
            let request = serde_json::from_slice(&bytes[boundary..]).unwrap();
            let (mime, body) = reply(&request);
            requests.push(request);
            let content_type = mime
                .map(|value| format!("Content-Type: {value}\r\n"))
                .unwrap_or_default();
            write!(socket,"HTTP/1.1 200 OK\r\n{content_type}Content-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
        }
        requests
    });
    let transport = GraphQlTransport::new(
        EndpointUrl::parse(&format!("http://{address}/graphql")).unwrap(),
        ApiKey::new("fixture-key".to_owned()).unwrap(),
        TransportConfig {
            ca_bundle: None,
            deadline: Deadline::new(Duration::from_secs(2)).unwrap(),
            max_response_bytes: ResponseCap::new(65536).unwrap(),
        },
    )
    .unwrap();
    (transport, thread)
}
