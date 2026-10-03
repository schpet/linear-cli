use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde_json::{Value, json};

/// One HTTP request the CLI sent to the mock.
#[derive(Clone, Debug)]
pub struct Request {
    pub method: String,
    pub path: String,
    /// Header names are lowercased.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// GraphQL operation name: `operationName` from the body, else the name in the document.
    pub operation: Option<String>,
    pub query: String,
    /// `Value::Null` when the request carried no variables.
    pub variables: Value,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

#[derive(Debug)]
enum Route {
    Operation(String),
    Http { method: String, path: String },
}

impl Route {
    fn matches(&self, request: &Request) -> bool {
        match self {
            Self::Operation(name) => request.operation.as_deref() == Some(name.as_str()),
            Self::Http { method, path } => request.method == *method && request.path == *path,
        }
    }
}

#[derive(Debug)]
struct Reply {
    route: Route,
    status: u16,
    /// Response headers besides `Content-Length` and `Connection`.
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

fn headers(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect()
}

#[derive(Default)]
struct State {
    pending: Vec<Reply>,
    requests: Vec<Request>,
    unexpected: Vec<String>,
}

/// A loopback Linear GraphQL API. Replies are queued per route and served first in, first out.
/// Dropping it fails the test if a queued reply was never requested or an unexpected request
/// arrived, or a request worker panicked.
pub struct MockLinear {
    addr: SocketAddr,
    state: Arc<Mutex<State>>,
    shutdown: Arc<AtomicBool>,
    acceptor: Option<JoinHandle<Vec<JoinHandle<()>>>>,
}

impl MockLinear {
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback listener");
        let addr = listener.local_addr().expect("listener address");
        let state = Arc::new(Mutex::new(State::default()));
        let shutdown = Arc::new(AtomicBool::new(false));
        let acceptor = {
            let state = Arc::clone(&state);
            let shutdown = Arc::clone(&shutdown);
            thread::spawn(move || {
                let mut workers = Vec::new();
                for stream in listener.incoming() {
                    if shutdown.load(Ordering::SeqCst) {
                        break;
                    }
                    let stream = stream.expect("accept connection");
                    let state = Arc::clone(&state);
                    workers.push(thread::spawn(move || serve(stream, &state)));
                }
                workers
            })
        };
        Self {
            addr,
            state,
            shutdown,
            acceptor: Some(acceptor),
        }
    }

    /// The GraphQL endpoint URL.
    pub fn url(&self) -> String {
        format!("http://{}/graphql", self.addr)
    }

    /// Base URL for non-GraphQL routes, e.g. signed upload targets.
    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Reply to the next `operation` request with `{"data": data}`.
    pub fn on(&self, operation: &str, data: Value) -> &Self {
        self.on_raw(operation, 200, &json!({ "data": data }).to_string())
    }

    /// Reply to the next `operation` request with a GraphQL error envelope.
    pub fn on_error(&self, operation: &str, message: &str) -> &Self {
        let body = json!({ "data": null, "errors": [{ "message": message }] });
        self.on_raw(operation, 200, &body.to_string())
    }

    /// Reply to the next `operation` request with an arbitrary status and JSON body text.
    pub fn on_raw(&self, operation: &str, status: u16, body: &str) -> &Self {
        self.on_text(operation, status, "application/json", body)
    }

    /// Reply to the next `operation` request with any status, content type and body text.
    pub fn on_text(&self, operation: &str, status: u16, content_type: &str, body: &str) -> &Self {
        self.push(Reply {
            route: Route::Operation(operation.to_owned()),
            status,
            headers: headers(&[("Content-Type", content_type)]),
            body: body.as_bytes().to_vec(),
        })
    }

    /// Answer the next `operation` request with a `302 Found` redirect to `location`.
    pub fn redirect(&self, operation: &str, location: &str) -> &Self {
        self.push(Reply {
            route: Route::Operation(operation.to_owned()),
            status: 302,
            headers: headers(&[("Location", location)]),
            body: Vec::new(),
        })
    }

    /// Reply to the next plain HTTP request for `method path` (non-GraphQL traffic).
    pub fn on_http(&self, method: &str, path: &str, status: u16, body: &[u8]) -> &Self {
        self.on_http_with(
            method,
            path,
            status,
            &[("Content-Type", "application/octet-stream")],
            body,
        )
    }

    /// Like `on_http`, with explicit response headers.
    pub fn on_http_with(
        &self,
        method: &str,
        path: &str,
        status: u16,
        response_headers: &[(&str, &str)],
        body: &[u8],
    ) -> &Self {
        self.push(Reply {
            route: Route::Http {
                method: method.to_owned(),
                path: path.to_owned(),
            },
            status,
            headers: headers(response_headers),
            body: body.to_vec(),
        })
    }

    fn push(&self, reply: Reply) -> &Self {
        self.lock().pending.push(reply);
        self
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().expect("mock state lock")
    }

    /// Every request received so far, in arrival order.
    pub fn requests(&self) -> Vec<Request> {
        self.lock().requests.clone()
    }

    /// GraphQL operation names received so far, in arrival order.
    pub fn operations(&self) -> Vec<String> {
        self.requests()
            .into_iter()
            .map(|request| request.operation.unwrap_or_default())
            .collect()
    }

    /// The single request for `operation`; panics if there were zero or several.
    pub fn request(&self, operation: &str) -> Request {
        let mut matching: Vec<Request> = self
            .requests()
            .into_iter()
            .filter(|request| request.operation.as_deref() == Some(operation))
            .collect();
        assert_eq!(
            matching.len(),
            1,
            "expected exactly one {operation} request, got operations {:?}",
            self.operations()
        );
        matching.remove(0)
    }

    /// Variables of the single request for `operation`.
    pub fn variables(&self, operation: &str) -> Value {
        self.request(operation).variables
    }
}

impl Drop for MockLinear {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        // Wake the acceptor so it observes the shutdown flag.
        let _ = TcpStream::connect(self.addr);
        let mut failure = None;
        if let Some(acceptor) = self.acceptor.take() {
            match acceptor.join() {
                Ok(workers) => {
                    for worker in workers {
                        if let Err(panic) = worker.join()
                            && failure.is_none()
                        {
                            failure = Some(panic);
                        }
                    }
                }
                Err(panic) => failure = Some(panic),
            }
        }
        if thread::panicking() {
            return;
        }
        if let Some(panic) = failure {
            std::panic::resume_unwind(panic);
        }
        let state = self.lock();
        assert!(
            state.unexpected.is_empty(),
            "MockLinear received unexpected requests: {:#?}",
            state.unexpected
        );
        assert!(
            state.pending.is_empty(),
            "MockLinear replies were never requested: {:?}",
            state
                .pending
                .iter()
                .map(|reply| &reply.route)
                .collect::<Vec<_>>()
        );
    }
}

fn serve(stream: TcpStream, state: &Mutex<State>) {
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("set read timeout");
    let Some(request) = read_request(&stream) else {
        // The shutdown wake-up connection and aborted clients send nothing.
        return;
    };
    let reply = {
        let mut state = state.lock().expect("mock state lock");
        let reply = state
            .pending
            .iter()
            .position(|reply| reply.route.matches(&request))
            .map(|index| state.pending.remove(index));
        if reply.is_none() {
            state.unexpected.push(format!(
                "{} {} operation={:?} variables={}",
                request.method, request.path, request.operation, request.variables
            ));
        }
        state.requests.push(request);
        reply
    };
    let (status, headers, body) = match reply {
        Some(reply) => (reply.status, reply.headers, reply.body),
        None => (
            500,
            headers(&[("Content-Type", "application/json")]),
            br#"{"errors":[{"message":"MockLinear: unexpected request"}]}"#.to_vec(),
        ),
    };
    let mut head = format!("HTTP/1.1 {status} Mock\r\n");
    for (name, value) in headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str(&format!(
        "Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    ));
    let mut stream = stream;
    // The client may have given up already (for example after a timeout); that is its problem.
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&body);
    let _ = stream.flush();
}

fn read_request(stream: &TcpStream) -> Option<Request> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if reader.read_line(&mut line).ok()? == 0 {
        return None;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next().expect("request method").to_owned();
    let path = parts.next().expect("request path").to_owned();
    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).expect("read header line");
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break;
        }
        let (name, value) = line.split_once(':').expect("header has a colon");
        headers.push((name.trim().to_ascii_lowercase(), value.trim().to_owned()));
    }
    assert!(
        !headers
            .iter()
            .any(|(name, value)| name == "transfer-encoding" && value.contains("chunked")),
        "MockLinear does not support chunked request bodies"
    );
    let length = headers
        .iter()
        .find(|(name, _)| name == "content-length")
        .map_or(0, |(_, value)| {
            value.parse::<usize>().expect("content-length")
        });
    let mut body = vec![0; length];
    reader.read_exact(&mut body).expect("read request body");

    let envelope: Option<Value> = serde_json::from_slice(&body).ok();
    let field = |name: &str| envelope.as_ref().and_then(|value| value.get(name));
    let query = field("query")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let operation = field("operationName")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| document_operation_name(&query));
    let variables = field("variables").cloned().unwrap_or(Value::Null);
    Some(Request {
        method,
        path,
        headers,
        body,
        operation,
        query,
        variables,
    })
}

fn document_operation_name(query: &str) -> Option<String> {
    let rest = query.trim_start();
    let rest = ["query", "mutation", "subscription"]
        .iter()
        .find_map(|keyword| rest.strip_prefix(keyword))?;
    let name: String = rest
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    (!name.is_empty()).then_some(name)
}
