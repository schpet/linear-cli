//! Self-checks for the test harness.
use std::panic::{AssertUnwindSafe, catch_unwind};

use serde_json::{Value, json};

use crate::support::{API_KEY, Cli, MockLinear, Request, Run};

pub fn viewer() -> Value {
    json!({
        "viewer": {
            "id": "user-1", "name": "Alice Example", "displayName": "alice",
            "email": "alice@example.com", "admin": false, "guest": false,
            "organization": { "name": "Acme", "urlKey": "acme", "logoUrl": null }
        }
    })
}

#[test]
fn records_operation_variables_and_auth_header() {
    let api = MockLinear::start();
    api.on("AuthStatus", viewer());
    Cli::for_api(&api)
        .run(&["auth", "whoami"])
        .success()
        .stdout_has("Acme");
    let request = api.request("AuthStatus");
    assert_eq!(request.method, "POST");
    assert_eq!(request.header("authorization"), Some(API_KEY));
    assert!(request.query.contains("viewer"));
}

#[test]
fn unconsumed_reply_fails_the_test() {
    let result = catch_unwind(|| {
        let api = MockLinear::start();
        api.on("AuthStatus", viewer());
    });
    assert!(result.is_err());
}

#[test]
fn unexpected_request_fails_the_test() {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let api = MockLinear::start();
        Cli::for_api(&api).run(&["auth", "whoami"]).failure();
    }));
    assert!(result.is_err());
}

#[test]
fn replies_are_first_in_first_out_per_operation() {
    let api = MockLinear::start();
    let mut second = viewer();
    second["viewer"]["organization"]["name"] = json!("Second");
    api.on("AuthStatus", viewer()).on("AuthStatus", second);
    let cli = Cli::for_api(&api);
    cli.run(&["auth", "whoami"]).success().stdout_has("Acme");
    cli.run(&["auth", "whoami"]).success().stdout_has("Second");
    assert_eq!(api.operations(), ["AuthStatus", "AuthStatus"]);
}

#[test]
fn stubs_record_argv_and_run_their_script() {
    let cli = Cli::new().stub_bin("tool", "echo stubbed; exit 3");
    let output = std::process::Command::new(cli.path("bin/tool"))
        .args(["a b", "multi\nline", ""])
        .output()
        .expect("run stub");
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(output.stdout, b"stubbed\n");
    assert_eq!(cli.calls("tool"), [vec!["a b", "multi\nline", ""]]);
    assert!(cli.calls("missing").is_empty());
}

#[test]
fn plain_http_routes_are_served_and_recorded() {
    let api = MockLinear::start();
    api.on_http("PUT", "/upload/1", 200, b"");
    let base = api.base_url();
    let addr = base.strip_prefix("http://").expect("http base url");
    let mut stream = std::net::TcpStream::connect(addr).expect("connect to mock");
    std::io::Write::write_all(
        &mut stream,
        b"PUT /upload/1 HTTP/1.1\r\nHost: mock\r\nContent-Length: 5\r\n\r\nhello",
    )
    .expect("send request");
    let mut response = String::new();
    std::io::Read::read_to_string(&mut stream, &mut response).expect("read response");
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    let requests: Vec<Request> = api.requests();
    assert_eq!(requests[0].body, b"hello");
    assert_eq!(requests[0].variables, Value::Null);
}

#[test]
fn run_helpers_parse_json_and_classify_usage_errors() {
    Cli::new().run(&["auth", "token", "--bogus"]).usage_error();
    let api = MockLinear::start();
    api.on("Probe", json!({ "viewer": { "id": "user-1" } }));
    let run: Run = Cli::for_api(&api).run(&[
        "api",
        "query Probe($n: Int) { viewer { id } }",
        "--variable",
        "n=3",
    ]);
    assert_eq!(run.success().json()["data"]["viewer"]["id"], "user-1");
    assert_eq!(api.variables("Probe"), json!({ "n": 3 }));
}
