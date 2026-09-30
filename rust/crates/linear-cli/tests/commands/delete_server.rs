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

pub fn serve(body: &'static str) -> (GraphQlTransport, thread::JoinHandle<serde_json::Value>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/graphql", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut input = Vec::new();
        let request = loop {
            let mut bytes = [0; 8192];
            let count = stream.read(&mut bytes).unwrap();
            assert!(count > 0);
            input.extend_from_slice(&bytes[..count]);
            if let Some((headers, payload)) =
                std::str::from_utf8(&input).unwrap().split_once("\r\n\r\n")
            {
                let length = headers
                    .lines()
                    .find_map(|line| {
                        let (key, value) = line.split_once(':')?;
                        key.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().unwrap())
                    })
                    .unwrap();
                if payload.len() >= length {
                    break serde_json::from_str(payload).unwrap();
                }
            }
        };
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        drop(stream);
        thread::sleep(Duration::from_millis(50));
        listener.set_nonblocking(true).unwrap();
        assert!(listener.accept().is_err(), "delete must send exactly once");
        request
    });
    let transport = GraphQlTransport::new(
        EndpointUrl::parse(&endpoint).unwrap(),
        ApiKey::new("lin_api_fake".into()).unwrap(),
        TransportConfig {
            proxy: ProxyMode::Direct,
            ca: CaMode::PublicRoots,
            deadline: Deadline::new(Duration::from_secs(2)).unwrap(),
            max_response_bytes: ResponseCap::new(65536).unwrap(),
        },
    )
    .unwrap();
    (transport, server)
}
