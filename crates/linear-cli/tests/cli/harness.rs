//! Self-checks for the test harness.
use std::panic::{AssertUnwindSafe, catch_unwind};

use serde_json::{Value, json};

use crate::support::{API_KEY, Cli, MockLinear, Request, Run, assert_json, nodes};

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

#[test]
fn json_helpers_compare_output_with_flattened_fixtures() {
    let entity = json!({ "id": "a", "labels": { "nodes": [{ "id": "l" }] } });
    assert_eq!(nodes(&json!([entity])), std::slice::from_ref(&entity));
    assert_json(&json!({ "id": "a", "labels": [{ "id": "l" }] }), &entity);
    let wrapped = std::panic::catch_unwind(|| nodes(&json!({ "nodes": [entity] })));
    assert!(wrapped.is_err(), "a wrapped list is not a list");
    let nested = std::panic::catch_unwind(|| assert_json(&entity, &entity));
    assert!(nested.is_err(), "output may not contain connections");
}

#[test]
fn stub_call_reads_report_errors_instead_of_returning_no_calls() {
    let cli = Cli::new();
    std::fs::write(cli.path("calls/invalid"), [0xff, 0x1e]).expect("invalid UTF-8 log");
    std::fs::create_dir(cli.path("calls/directory")).expect("directory log");
    for name in ["invalid", "directory"] {
        let failure = catch_unwind(AssertUnwindSafe(|| cli.calls(name)))
            .expect_err("read failure must not become an empty call list");
        let message = failure.downcast_ref::<String>().expect("panic message");
        assert!(
            message.contains(&cli.path(&format!("calls/{name}")).display().to_string()),
            "{message}"
        );
    }
}

#[test]
fn malformed_request_worker_panics_fail_mock_teardown() {
    let failure = catch_unwind(|| {
        let api = MockLinear::start();
        let base = api.base_url();
        let mut client =
            std::net::TcpStream::connect(base.strip_prefix("http://").expect("HTTP endpoint"))
                .expect("connect client");
        std::io::Write::write_all(&mut client, b"POST /graphql HTTP/1.1\r\nno colon\r\n\r\n")
            .expect("malformed request");
        let result = std::io::Read::read_to_end(&mut client, &mut Vec::new());
        if let Err(error) = result {
            assert_eq!(error.kind(), std::io::ErrorKind::ConnectionReset);
        }
        drop(api);
    })
    .expect_err("malformed request worker must fail test teardown");
    assert!(
        failure
            .downcast_ref::<String>()
            .expect("panic message")
            .contains("header has a colon")
    );
}

#[test]
fn aborted_clients_do_not_fail_mock_teardown() {
    let api = MockLinear::start();
    let base = api.base_url();
    let mut client =
        std::net::TcpStream::connect(base.strip_prefix("http://").expect("HTTP endpoint"))
            .expect("connect client");
    client
        .shutdown(std::net::Shutdown::Write)
        .expect("abort client");
    let mut response = Vec::new();
    std::io::Read::read_to_end(&mut client, &mut response).expect("worker closed client");
    assert!(response.is_empty());
    assert!(api.requests().is_empty());
}

#[test]
fn a_mock_worker_failure_preserves_an_existing_test_panic() {
    let failure = catch_unwind(|| {
        let api = MockLinear::start();
        api.on("AuthStatus", viewer());
        let base = api.base_url();
        let mut client =
            std::net::TcpStream::connect(base.strip_prefix("http://").expect("HTTP endpoint"))
                .expect("connect client");
        std::io::Write::write_all(&mut client, b"POST /graphql HTTP/1.1\r\nno colon\r\n\r\n")
            .expect("malformed request");
        let result = std::io::Read::read_to_end(&mut client, &mut Vec::new());
        if let Err(error) = result {
            assert_eq!(error.kind(), std::io::ErrorKind::ConnectionReset);
        }
        panic!("original test failure");
    })
    .expect_err("original failure must survive teardown");
    assert_eq!(
        failure.downcast_ref::<&str>(),
        Some(&"original test failure")
    );
}

#[test]
fn stalled_request_worker_panics_fail_mock_teardown() {
    let failure = catch_unwind(|| {
        let api = MockLinear::start();
        let base = api.base_url();
        let mut client =
            std::net::TcpStream::connect(base.strip_prefix("http://").expect("HTTP endpoint"))
                .expect("connect client");
        std::io::Write::write_all(&mut client, b"POST /graphql HTTP/1.1\r\n")
            .expect("partial request");
        let result = std::io::Read::read_to_end(&mut client, &mut Vec::new());
        if let Err(error) = result {
            assert_eq!(error.kind(), std::io::ErrorKind::ConnectionReset);
        }
        drop(api);
    })
    .expect_err("request read timeout must fail test teardown");
    assert!(
        failure
            .downcast_ref::<String>()
            .expect("panic message")
            .contains("read header line")
    );
}

#[test]
fn failure_accepts_only_an_ordinary_runtime_error() {
    let run = |code| Run {
        args: vec!["synthetic".to_owned()],
        code,
        stdout: String::new(),
        stderr: String::new(),
    };
    run(1).failure();
    for code in [101, 0, 2, 7, 130] {
        assert!(
            catch_unwind(|| {
                run(code).failure();
            })
            .is_err(),
            "failure accepted exit status {code}"
        );
    }
}

#[test]
fn restubbing_preserves_open_script_readers_and_call_history() {
    let cli = Cli::new().stub_bin("tool", "echo first");
    let first = std::process::Command::new(cli.path("bin/tool"))
        .arg("first arg")
        .output()
        .expect("run first stub");
    assert!(first.status.success());
    assert_eq!(first.stdout, b"first\n");
    let mut reader = std::fs::File::open(cli.path("bin/tool")).expect("open first script");
    let cli = cli.stub_bin("tool", "echo second");
    let mut old_script = String::new();
    std::io::Read::read_to_string(&mut reader, &mut old_script).expect("read first script");
    assert!(
        old_script.ends_with("echo first\n"),
        "an open reader must keep the original script"
    );
    let second = std::process::Command::new(cli.path("bin/tool"))
        .arg("second arg")
        .output()
        .expect("run replaced stub");
    assert!(second.status.success());
    assert_eq!(second.stdout, b"second\n");
    assert_eq!(cli.calls("tool"), [vec!["first arg"], vec!["second arg"]]);
}
