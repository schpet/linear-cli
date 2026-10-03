//! A loopback HTTP server that answers each connection with the next scripted
//! [`Reply`] and records what the client sent.

use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// What the server does with one connection.
pub(crate) enum Reply {
    /// A complete response with `Content-Length`, then close.
    Http {
        status: u16,
        headers: Vec<(&'static str, String)>,
        body: Vec<u8>,
        delay: Duration,
    },
    /// Headers and a body prefix, then hold the connection open.
    Stall,
    /// Read the request and never answer.
    Silent,
    /// A chunked body that never ends.
    EndlessChunks,
}

impl Reply {
    pub(super) fn status(status: u16, content_type: &str, body: impl Into<Vec<u8>>) -> Self {
        Self::Http {
            status,
            headers: vec![("content-type", content_type.to_owned())],
            body: body.into(),
            delay: Duration::ZERO,
        }
    }

    pub(crate) fn json(body: &serde_json::Value) -> Self {
        Self::status(200, "application/json", body.to_string())
    }

    pub(super) fn header(mut self, name: &'static str, value: &str) -> Self {
        if let Self::Http { headers, .. } = &mut self {
            headers.push((name, value.to_owned()));
        }
        self
    }

    pub(super) fn delayed(mut self, by: Duration) -> Self {
        if let Self::Http { delay, .. } = &mut self {
            *delay = by;
        }
        self
    }
}

/// One request as the server received it.
#[derive(Debug)]
pub(crate) struct Request {
    pub method: String,
    pub path: String,
    /// Header names are lowercased.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// Whether the client closed a connection the server held open.
    pub client_closed: bool,
}

impl Request {
    pub(super) fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    pub(super) fn headers_named(&self, name: &str) -> usize {
        self.headers.iter().filter(|(key, _)| key == name).count()
    }
}

pub(crate) struct Server {
    port: u16,
    stop: mpsc::Sender<()>,
    handle: JoinHandle<Vec<Request>>,
}

impl Server {
    /// Serves one connection per reply, in order, then stops listening.
    pub(crate) fn start(replies: Vec<Reply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        listener
            .set_nonblocking(true)
            .expect("nonblocking listener");
        let port = listener.local_addr().expect("local address").port();
        let (stop, stopped) = mpsc::channel();
        let handle = thread::spawn(move || {
            let mut requests = Vec::new();
            for reply in replies {
                let Some(stream) = accept(&listener, &stopped) else {
                    break;
                };
                requests.push(answer(&stream, reply, &stopped));
            }
            requests
        });
        Self { port, stop, handle }
    }

    pub(super) fn port(&self) -> u16 {
        self.port
    }

    pub(super) fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }

    /// Whether every reply has been served and every held connection closed.
    pub(super) fn is_done(&self) -> bool {
        self.handle.is_finished()
    }

    /// Stops the server and returns the requests it received.
    pub(crate) fn finish(self) -> Vec<Request> {
        let _ = self.stop.send(());
        self.handle.join().expect("server thread")
    }
}

fn should_stop(stopped: &mpsc::Receiver<()>) -> bool {
    !matches!(
        stopped.recv_timeout(Duration::from_millis(10)),
        Err(RecvTimeoutError::Timeout)
    )
}

fn accept(listener: &TcpListener, stopped: &mpsc::Receiver<()>) -> Option<TcpStream> {
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false).expect("blocking stream");
                stream
                    .set_read_timeout(Some(Duration::from_millis(20)))
                    .expect("read timeout");
                return Some(stream);
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                if should_stop(stopped) {
                    return None;
                }
            }
            Err(error) => panic!("accept: {error}"),
        }
    }
}

fn answer(stream: &TcpStream, reply: Reply, stopped: &mpsc::Receiver<()>) -> Request {
    let mut request = read_request(stream);
    let mut stream = stream;
    match reply {
        Reply::Http {
            status,
            headers,
            body,
            delay,
        } => {
            thread::sleep(delay);
            let reason = reqwest::StatusCode::from_u16(status)
                .ok()
                .and_then(|status| status.canonical_reason())
                .unwrap_or("Unknown");
            let mut head = format!("HTTP/1.1 {status} {reason}\r\n");
            for (name, value) in &headers {
                head.push_str(&format!("{name}: {value}\r\n"));
            }
            head.push_str(&format!(
                "Content-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            ));
            // The client may legitimately hang up early (e.g. an oversized body).
            let _ = stream
                .write_all(head.as_bytes())
                .and_then(|()| stream.write_all(&body))
                .and_then(|()| stream.flush());
            return request;
        }
        Reply::Stall => {
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 1000\r\n\r\n{{\"data\":"
            )
            .expect("partial response");
            stream.flush().expect("flush");
        }
        Reply::Silent => {}
        Reply::EndlessChunks => {
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n"
            )
            .expect("chunked head");
            let chunk = vec![b'y'; 512];
            loop {
                if should_stop(stopped) {
                    return request;
                }
                let written = write!(stream, "{:x}\r\n", chunk.len())
                    .and_then(|()| stream.write_all(&chunk))
                    .and_then(|()| stream.write_all(b"\r\n"))
                    .and_then(|()| stream.flush());
                if written.is_err() {
                    request.client_closed = true;
                    return request;
                }
            }
        }
    }
    // Hold the connection open until the client closes it or the test stops.
    let mut sink = [0_u8; 64];
    loop {
        if should_stop(stopped) {
            return request;
        }
        match stream.read(&mut sink) {
            Ok(0) => {
                request.client_closed = true;
                return request;
            }
            Ok(_) => {}
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(_) => {
                request.client_closed = true;
                return request;
            }
        }
    }
}

/// Reads the request head and its `Content-Length` body.
fn read_request(mut stream: &TcpStream) -> Request {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 1024];
    let started = Instant::now();
    let (head_end, length) = loop {
        match stream.read(&mut chunk) {
            Ok(0) => panic!("client closed before sending a request"),
            Ok(n) => buffer.extend_from_slice(chunk.get(..n).expect("read within buffer")),
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(error) => panic!("read request: {error}"),
        }
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "request never completed"
        );
        let Some(head_end) = buffer.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        let head = String::from_utf8_lossy(&buffer[..head_end]);
        let length = head
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().expect("content length"))
            })
            .unwrap_or(0);
        if buffer.len() >= head_end + 4 + length {
            break (head_end, length);
        }
    };
    let head = String::from_utf8(buffer[..head_end].to_vec()).expect("ASCII request head");
    let mut lines = head.lines();
    let mut start = lines.next().expect("request line").split(' ');
    let method = start.next().expect("method").to_owned();
    let path = start.next().expect("path").to_owned();
    let headers = lines
        .map(|line| {
            let (name, value) = line.split_once(':').expect("header line");
            (name.to_ascii_lowercase(), value.trim().to_owned())
        })
        .collect();
    Request {
        method,
        path,
        headers,
        body: buffer[head_end + 4..head_end + 4 + length].to_vec(),
        client_closed: false,
    }
}
