use linear_cli::commands::milestone_update::{self, Options};
use linear_cli::error::AppErrorKind;
use linear_cli::graphql::envelope::parse_response;
use linear_cli::graphql::operations::milestone_update::UpdateProjectMilestone;
use linear_cli::graphql::scalars::TimelessDate;
use linear_cli::graphql::transport::{
    ApiKey, Deadline, EndpointUrl, GraphQlTransport, ResponseCap, TransportConfig,
};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

const ID: &str = "00000000-0000-4000-9000-000000003301";

fn frozen_step(id: &str) -> Value {
    let case: Value = serde_json::from_slice(
        &std::fs::read(format!(
            "{}/../../parity/runner/c033-frozen-cases/{id}.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap(),
    )
    .unwrap();
    case["graphql"]["groups"][0]["steps"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .clone()
}

fn name_options() -> Options {
    Options {
        name: Some("Launch".to_owned()),
        ..Options::default()
    }
}

fn wire(options: &Options) -> Value {
    serde_json::to_value(milestone_update::request(ID, options).unwrap()).unwrap()
}

#[test]
fn typed_request_matches_source_selection_and_direct_id() {
    let expected = &frozen_step("c033-name-only")["operation"];
    let actual = wire(&name_options());
    let compact = |value: &str| {
        value
            .chars()
            .filter(|ch| !ch.is_whitespace() && *ch != ',')
            .collect::<String>()
    };
    assert_eq!(
        compact(actual["query"].as_str().unwrap()),
        compact(expected["document"].as_str().unwrap())
    );
    assert_eq!(actual["variables"], expected["variables"]);
    assert_eq!(actual["operationName"], "UpdateProjectMilestone");
    let direct =
        serde_json::to_value(milestone_update::request("arbitrary-id", &name_options()).unwrap())
            .unwrap();
    assert_eq!(direct["variables"]["id"], "arbitrary-id");
}

#[test]
fn truthy_optionals_zero_and_server_dates_follow_source() {
    let options = Options {
        name: Some(String::new()),
        description: Some(" ".to_owned()),
        target_date: Some("2026-02-30".to_owned()),
        sort_order: Some(-0.0),
        project_id: Some(String::new()),
    };
    assert_eq!(
        wire(&options)["variables"]["input"],
        json!({"description":" ","targetDate":"2026-02-30","sortOrder":0.0})
    );
    assert!(
        !String::from_utf8(
            serde_json::to_vec(&milestone_update::request(ID, &options).unwrap()).unwrap()
        )
        .unwrap()
        .contains("\"sortOrder\":-0")
    );
    let empty = Options {
        name: Some(String::new()),
        description: Some(String::new()),
        target_date: Some(String::new()),
        project_id: Some(String::new()),
        sort_order: None,
    };
    let error = milestone_update::request(ID, &empty).unwrap_err();
    assert_eq!(error.kind, AppErrorKind::Validation);
    assert_eq!(error.context, None);
    assert_eq!(
        error.suggestion.as_deref(),
        Some("Use --name, --description, --target-date, --sort-order, or --project")
    );
}

#[test]
fn public_request_rejects_nonfinite_typed_values_before_json_null() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let options = Options {
            sort_order: Some(value),
            ..name_options()
        };
        let error = milestone_update::request(ID, &options).unwrap_err();
        assert_eq!(error.kind, AppErrorKind::Validation);
        assert_eq!(error.message, "Sort order must be a finite number");
    }
}

#[test]
fn strict_decode_and_js_number_output_use_the_returned_fields() {
    let data = frozen_step("c033-sort-negative-zero")["response"]["data"].clone();
    let decoded: UpdateProjectMilestone =
        parse_response(json!({"data":data}).to_string().as_bytes()).unwrap();
    let mut milestone = decoded.project_milestone_update.project_milestone;
    assert_eq!(milestone_update::render(&milestone).unwrap(), format!("✓ Updated milestone: Existing\n  ID: {ID}\n  Target Date: 2026-10-01\n  Sort Order: 1e+21\n  Project: Mobile App\n").as_bytes());
    for date in [None, Some(TimelessDate(String::new()))] {
        milestone.target_date = date;
        milestone.sort_order = -0.0;
        assert_eq!(milestone_update::render(&milestone).unwrap(), format!("✓ Updated milestone: Existing\n  ID: {ID}\n  Sort Order: 0\n  Project: Mobile App\n").as_bytes());
    }
    for payload in [
        json!({"success":true,"projectMilestone":null}),
        json!({"success":true}),
    ] {
        assert!(
            parse_response::<UpdateProjectMilestone>(
                json!({"data":{"projectMilestoneUpdate":payload}})
                    .to_string()
                    .as_bytes()
            )
            .is_err()
        );
    }
    milestone.sort_order = f64::INFINITY;
    assert!(milestone_update::render(&milestone).is_err());
}

#[derive(Clone, Copy, Debug)]
enum Reply {
    Respond(u16, &'static str),
    /// Keep the connection open past the client deadline.
    Silent,
    /// Close after reading the whole request, as if the server wrote the
    /// milestone and the connection died before the response.
    DropAfterWrite,
}

/// Reads one complete request, answers it as `reply` says, and reports
/// whether a second connection arrived afterwards.
fn serve_once(reply: Reply) -> (String, thread::JoinHandle<bool>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/graphql", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut request = Vec::new();
        let mut chunk = [0_u8; 8192];
        loop {
            let count = stream.read(&mut chunk).unwrap();
            assert!(count > 0, "request closed early");
            request.extend_from_slice(&chunk[..count]);
            let text = String::from_utf8_lossy(&request);
            if let Some((head, body)) = text.split_once("\r\n\r\n") {
                let length = head
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().unwrap())
                    })
                    .unwrap();
                if body.len() >= length {
                    break;
                }
            }
        }
        match reply {
            Reply::Respond(status, body) => {
                let head = format!(
                    "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(head.as_bytes()).unwrap();
                stream.write_all(body.as_bytes()).unwrap();
            }
            Reply::Silent => thread::sleep(Duration::from_millis(250)),
            Reply::DropAfterWrite => {}
        }
        drop(stream);
        thread::sleep(Duration::from_millis(100));
        listener.set_nonblocking(true).unwrap();
        listener.accept().is_ok()
    });
    (endpoint, server)
}

fn transport(endpoint: &str, deadline: Duration) -> GraphQlTransport {
    GraphQlTransport::new(
        EndpointUrl::parse(endpoint).unwrap(),
        ApiKey::new("lin_api_fake".to_owned()).unwrap(),
        TransportConfig {
            ca_bundle: None,
            deadline: Deadline::new(deadline).unwrap(),
            max_response_bytes: ResponseCap::new(65536).unwrap(),
        },
    )
    .unwrap()
}

#[tokio::test]
async fn update_failures_never_retry_or_append_create_uncertainty() {
    let cases = [
        (
            Reply::Respond(
                200,
                r#"{"data":{"projectMilestoneUpdate":{"success":true,"projectMilestone":null}}}"#,
            ),
            Duration::from_secs(5),
        ),
        (Reply::Silent, Duration::from_millis(50)),
        (Reply::DropAfterWrite, Duration::from_secs(5)),
        (
            Reply::Respond(
                200,
                r#"{"data":{"projectMilestoneUpdate":{"success":false,"projectMilestone":{"id":"m","name":"n","targetDate":null,"sortOrder":5,"project":{"id":"p","name":"q"}}}}}"#,
            ),
            Duration::from_secs(5),
        ),
        (
            Reply::Respond(200, r#"{"errors":[{"message":"Entity not found"}]}"#),
            Duration::from_secs(5),
        ),
    ];
    for (response, deadline) in cases {
        let (endpoint, server) = serve_once(response);
        let error = milestone_update::submit(&transport(&endpoint, deadline), ID, &name_options())
            .await
            .unwrap_err();
        assert!(
            !error.message.contains("may already"),
            "{response:?}: {}",
            error.message
        );
        if matches!(response, Reply::Respond(_, body) if body.contains("\"success\":false")) {
            assert_eq!(error.message, "Failed to update milestone");
            assert_eq!(error.kind, AppErrorKind::GraphQl);
        }
        assert!(!server.join().unwrap(), "update must not retry");
    }
}
