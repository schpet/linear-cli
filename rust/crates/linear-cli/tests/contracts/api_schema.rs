//! Actual CLI raw wire, response stages and runtime/file effects; all synthetic.
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::TcpListener,
    process::{Command, Output, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
struct Reply {
    status: u16,
    headers: String,
    bytes: Vec<u8>,
}
impl Reply {
    fn text(status: u16, text: &str) -> Self {
        Self {
            status,
            headers: "Content-Type: text/plain\r\n".into(),
            bytes: text.as_bytes().to_vec(),
        }
    }
}
struct Server {
    url: String,
    stop: Arc<AtomicBool>,
    join: thread::JoinHandle<Vec<String>>,
}
impl Server {
    fn new(replies: Vec<Reply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}/graphql", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let done = stop.clone();
        let join = thread::spawn(move || {
            let mut requests = vec![];
            let start = Instant::now();
            while !done.load(Ordering::SeqCst) {
                let (mut stream, _) = match listener.accept() {
                    Ok(v) => v,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(start.elapsed() < Duration::from_secs(12));
                        thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(e) => panic!("{e}"),
                };
                stream
                    .set_nonblocking(false)
                    .expect("blocking accepted mock stream");
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut bytes = vec![];
                loop {
                    let mut chunk = [0; 8192];
                    let n = stream.read(&mut chunk).unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&chunk[..n]);
                    let text = std::str::from_utf8(&bytes).unwrap();
                    if let Some((headers, body)) = text.split_once("\r\n\r\n") {
                        let length = headers
                            .lines()
                            .find_map(|line| {
                                let (k, v) = line.split_once(':')?;
                                k.eq_ignore_ascii_case("content-length")
                                    .then(|| v.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        if body.len() >= length {
                            break;
                        }
                    }
                }
                requests.push(String::from_utf8(bytes).unwrap());
                let reply = &replies[requests.len() - 1];
                write!(
                    stream,
                    "HTTP/1.1 {} Fixture\r\n{}Content-Length: {}\r\nConnection: close\r\n\r\n",
                    reply.status,
                    reply.headers,
                    reply.bytes.len()
                )
                .unwrap();
                stream.write_all(&reply.bytes).unwrap();
            }
            requests
        });
        Self { url, stop, join }
    }
    fn finish(self) -> Vec<String> {
        self.stop.store(true, Ordering::SeqCst);
        self.join.join().unwrap()
    }
}
static SERIAL: AtomicUsize = AtomicUsize::new(0);
struct Home(std::path::PathBuf);
impl Home {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "linear-api-schema-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn command(&self, endpoint: &str, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_linear"));
        command
            .args(args)
            .current_dir(&self.0)
            .env_clear()
            .env("HOME", &self.0)
            .env("XDG_CONFIG_HOME", &self.0)
            .env("APPDATA", &self.0)
            .env("PATH", "")
            .env("NO_COLOR", "1")
            .env("CI", "1")
            .env("LINEAR_IGNORE_ENV_FILE", "1")
            .env("LINEAR_API_KEY", "lin_api_fake")
            .env("LINEAR_GRAPHQL_ENDPOINT", endpoint)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }
    fn run(&self, endpoint: &str, args: &[&str], input: &str) -> Output {
        let mut child = self.command(endpoint, args).spawn().unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    }
}
impl Drop for Home {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn body(wire: &str) -> &str {
    wire.split_once("\r\n\r\n").unwrap().1
}
#[test]
fn api_wire_is_verbatim_ordered_and_response_errors_do_not_erase_envelope() {
    let home = Home::new();
    let server = Server::new(vec![Reply::text(
        200,
        r#"{"data":{"mutation":{"success":false}},"extensions":{"x":2.0}}"#,
    )]);
    let out = home.run(
        &server.url,
        &[
            "api",
            " mutation M { anything } ",
            "--variables-json",
            r#"{"z":1,"2":"two","__proto__":1}"#,
            "--variable",
            "z=2",
            "--variable",
            "1=null",
        ],
        "",
    );
    assert!(out.status.success());
    assert_eq!(
        out.stdout,
        br#"{"data":{"mutation":{"success":false}},"extensions":{"x":2.0}}"#
    );
    assert!(out.stderr.is_empty());
    let requests = server.finish();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        body(&requests[0]),
        r#"{"query":" mutation M { anything } ","variables":{"z":2,"2":"two","__proto__":1,"1":null}}"#
    );
    assert!(
        requests[0]
            .to_ascii_lowercase()
            .contains("accept-encoding: gzip")
    );
}
#[test]
fn api_response_stage_table_preserves_raw_body_status_silent_and_typed_refusal() {
    for (status, text, silent, code, stdout, stderr) in [
        (500, " raw error ", false, 1, "", " raw error \n"),
        (500, "error", true, 1, "", ""),
        (200, " nonJSON ", false, 1, " nonJSON \n", ""),
        (200, " \"str\" \n", false, 0, " \"str\" \n", ""),
        (200, " null ", false, 0, " null \n", ""),
        (
            200,
            r#"{"errors":[{"message":"boom"}],"data":{"partial":true},"extensions":{"x":1}}"#,
            false,
            1,
            r#"{"errors":[{"message":"boom"}],"data":{"partial":true},"extensions":{"x":1}}"#,
            "",
        ),
        (
            200,
            r#"{"errors":[{}],"data":{"partial":true}}"#,
            true,
            1,
            "",
            "",
        ),
    ] {
        let home = Home::new();
        let server = Server::new(vec![Reply::text(status, text)]);
        let mut args = vec!["api", "mutation { opaque }"];
        if silent {
            args.push("--silent");
        }
        let out = home.run(&server.url, &args, "");
        assert_eq!(out.status.code(), Some(code));
        assert_eq!(String::from_utf8(out.stdout).unwrap(), stdout);
        assert_eq!(String::from_utf8(out.stderr).unwrap(), stderr);
        assert_eq!(server.finish().len(), 1);
    }
    for text in [r#""\ud800""#, "1e400", r#" {"a":"\ud800", "#] {
        let home = Home::new();
        let server = Server::new(vec![Reply::text(200, text)]);
        let out = home.run(&server.url, &["api", "mutation { opaque }", "--silent"], "");
        assert_eq!(out.status.code(), Some(1));
        assert!(out.stderr.is_empty());
        assert!(out.stdout.is_empty());
        assert_eq!(server.finish().len(), 1);
    }
}
#[test]
fn pagination_sends_each_cursor_and_stops_before_repeating_one() {
    let home = Home::new();
    let pages = [
        json!({"data":{"x":{"nodes":[1],"pageInfo":{"hasNextPage":true,"endCursor":"c1"}}}}),
        json!({"data":{"x":{"nodes":[2],"pageInfo":{"hasNextPage":true,"endCursor":"c2"}}}}),
        json!({"data":{"x":{"nodes":[3],"pageInfo":{"hasNextPage":true,"endCursor":"c2"}}}}),
    ];
    let server = Server::new(
        pages
            .iter()
            .map(|v| Reply::text(200, &v.to_string()))
            .collect(),
    );
    let out = home.run(
        &server.url,
        &[
            "api",
            "mutation { pages }",
            "--paginate",
            "--silent",
            "--variables-json",
            r#"{"before":1,"after":"old","last":2}"#,
        ],
        "",
    );
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert_eq!(
        String::from_utf8(out.stderr).unwrap(),
        "✗ API request failed: Repeated pagination cursor; request not sent, prior requests may have had effects\n"
    );
    let requests = server.finish();
    assert_eq!(requests.len(), 3);
    for (i, cursor) in ["null", r#""c1""#, r#""c2""#].iter().enumerate() {
        assert_eq!(
            body(&requests[i]),
            format!(
                r#"{{"query":"mutation {{ pages }}","variables":{{"before":1,"after":{cursor},"last":2}}}}"#
            )
        );
    }
}
#[test]
fn pagination_finds_the_first_connection_and_falls_back_to_the_raw_page() {
    for (pages, expected, code) in [
        (
            vec![
                r#"{"data":{"x":{"nodes":false,"pageInfo":[]}},"extensions":{"nodes":[9],"pageInfo":{"hasNextPage":false}}}"#,
            ],
            "[]",
            0,
        ),
        (
            vec![
                r#"{"data":{"x":{"nodes":[1],"pageInfo":{"hasNextPage":true,"endCursor":"c1"}}}}"#,
                r#"{"data":{"noConnection":2}}"#,
            ],
            r#"{"data":{"noConnection":2}}"#,
            0,
        ),
        (
            vec![
                r#"{"data":{"x":{"nodes":[1],"pageInfo":{"hasNextPage":true,"endCursor":"c1"}}}}"#,
                " late nonJSON ",
            ],
            " late nonJSON \n",
            1,
        ),
    ] {
        let home = Home::new();
        let server = Server::new(pages.into_iter().map(|v| Reply::text(200, v)).collect());
        let out = home.run(&server.url, &["api", "q", "--paginate"], "");
        assert_eq!(out.status.code(), Some(code));
        assert_eq!(String::from_utf8(out.stdout).unwrap(), expected);
        assert!(out.stderr.is_empty());
        server.finish();
    }
}
#[test]
fn schema_json_root_runtime_sdl_and_file_output_are_independent() {
    let home = Home::new();
    std::fs::write(home.0.join("schema.json"), "old").unwrap();
    let server = Server::new(vec![Reply {
        status: 200,
        headers: "Content-Type: application/json\r\n".into(),
        bytes: br#"{"data":{"future":2}}"#.to_vec(),
    }]);
    let out = home.run(
        &server.url,
        &["schema", "--json", "--output", "./schema.json"],
        "",
    );
    assert!(out.status.success());
    assert_eq!(out.stdout, b"Schema written to ./schema.json\n");
    assert_eq!(
        std::fs::read(home.0.join("schema.json")).unwrap(),
        b"{\n  \"future\": 2\n}\n"
    );
    let requests = server.finish();
    let request: Value = serde_json::from_str(body(&requests[0])).unwrap();
    assert_eq!(request["operationName"], "IntrospectionQuery");
    assert!(request.get("variables").is_none());
    assert_eq!(
        request["query"],
        linear_cli::graphql::schema_introspection::QUERY
    );
    let server = Server::new(vec![Reply {
        status: 200,
        headers: "Content-Type: application/json\r\n".into(),
        bytes: format!(
            "{{\"data\":{}}}",
            include_str!("../commands/fixtures/api-schema/synthetic.json")
        )
        .into_bytes(),
    }]);
    let out = home.run(&server.url, &["schema"], "");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        out.stdout,
        include_bytes!("../commands/fixtures/api-schema/synthetic.graphql")
    );
    server.finish();
    let server = Server::new(vec![Reply {
        status: 200,
        headers: "Content-Type: application/json\r\n".into(),
        bytes: br#"{"data":{"number":1e400}}"#.to_vec(),
    }]);
    let out = home.run(
        &server.url,
        &["schema", "--json", "--output", "./schema.json"],
        "",
    );
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert!(!out.stderr.is_empty());
    assert_eq!(
        std::fs::read(home.0.join("schema.json")).unwrap(),
        b"{\n  \"future\": 2\n}\n"
    );
    assert_eq!(server.finish().len(), 1);
}
#[test]
fn stdin_query_consumes_stream_before_stdin_variable_and_input_failures_send_zero_requests() {
    let home = Home::new();
    let server = Server::new(vec![]);
    let out = home.run(&server.url, &["api", "-", "--variable", "x=@-"], "q");
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        out.stderr,
        "✗ API request failed: No data on stdin for @- value\n".as_bytes()
    );
    assert!(server.finish().is_empty());
    for value in ["1e400", r#"{"x":"\ud800"}"#] {
        let server = Server::new(vec![]);
        let out = home.run(&server.url, &["api", "q", "--variables-json", value], "");
        assert_eq!(out.status.code(), Some(1));
        assert!(
            String::from_utf8(out.stderr)
                .unwrap()
                .contains("Invalid JSON for --variables-json")
        );
        assert!(server.finish().is_empty());
    }
}
#[test]
fn raw_fetch_redirects_post_to_get_strips_cross_origin_auth_and_decodes_gzip() {
    // `{"data":true}`, gzip-compressed.
    const GZIP_BODY: [u8; 33] = [
        31, 139, 8, 0, 0, 0, 0, 0, 2, 255, 171, 86, 74, 73, 44, 73, 84, 178, 42, 41, 42, 77, 173,
        5, 0, 116, 98, 198, 157, 13, 0, 0, 0,
    ];
    let home = Home::new();
    let target = Server::new(vec![Reply {
        status: 200,
        headers: "Content-Encoding: gzip\r\n".into(),
        bytes: GZIP_BODY.to_vec(),
    }]);
    let first = Server::new(vec![Reply {
        status: 302,
        headers: format!("Location: {}\r\n", target.url),
        bytes: vec![],
    }]);
    let out = home.run(&first.url, &["api", "mutation { sent }"], "");
    assert!(out.status.success());
    assert_eq!(out.stdout, b"{\"data\":true}");
    let a = first.finish();
    let b = target.finish();
    assert!(a[0].starts_with("POST "));
    assert!(
        a[0].to_ascii_lowercase()
            .contains("authorization: lin_api_fake")
    );
    assert!(b[0].starts_with("GET "));
    assert!(!b[0].to_ascii_lowercase().contains("authorization:"));
    assert_eq!(body(&b[0]), "");
}

#[test]
fn raw_api_unknown_content_encoding_preserves_normal_json_rendering() {
    let home = Home::new();
    let server = Server::new(vec![Reply {
        status: 200,
        headers: "Content-Type: application/json\r\nContent-Encoding: zstd\r\n".into(),
        bytes: b" {\"data\":{\"viewer\":null}} \n".to_vec(),
    }]);
    let out = home.run(&server.url, &["api", "query { viewer { id } }"], "");
    assert!(out.status.success());
    assert_eq!(out.stdout, b"{\"data\":{\"viewer\":null}}");
    assert!(out.stderr.is_empty());
    let requests = server.finish();
    assert_eq!(requests.len(), 1);
    assert_eq!(body(&requests[0]), r#"{"query":"query { viewer { id } }"}"#);
}
