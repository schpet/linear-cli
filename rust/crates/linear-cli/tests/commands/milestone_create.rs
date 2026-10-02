use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

use cynic::MutationBuilder;
use linear_cli::commands::milestone_create::{self, Options};
use linear_cli::graphql::envelope::parse_response;
use linear_cli::graphql::operations::milestone_create::{
    CreateProjectMilestone, CreateProjectMilestoneVariables, ProjectMilestoneCreateInput,
};
use linear_cli::graphql::scalars::TimelessDate;
use linear_cli::graphql::transport::{
    ApiKey, Deadline, EndpointUrl, GraphQlTransport, ResponseCap, TransportConfig,
};
use serde_json::{Value, json};

const PROJECT: &str = "3b9a5c7e-1d2f-4a6b-8c9d-0e1f2a3b4c5d";

fn frozen_step(id: &str) -> Value {
    let path = format!(
        "{}/../../parity/runner/c032-frozen-cases/{id}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let case: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    case["graphql"]["groups"][0]["steps"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .clone()
}

fn options(name: &str, description: Option<&str>, target_date: Option<&str>) -> Options {
    Options {
        name: name.to_owned(),
        description: description.map(str::to_owned),
        target_date: target_date.map(str::to_owned),
    }
}

fn wire(options: &Options) -> Value {
    serde_json::to_value(milestone_create::request(PROJECT, options)).unwrap()
}

#[test]
fn typed_mutation_matches_source_document_and_omits_absent_optionals() {
    let expected = &frozen_step("c032-uuid-minimal")["operation"];
    let actual = wire(&options("Beta", None, None));
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
    assert_eq!(actual["operationName"], "CreateProjectMilestone");
}

#[test]
fn request_preserves_supplied_values_verbatim() {
    for id in ["c032-full-optionals", "c032-whitespace-values"] {
        let expected = &frozen_step(id)["operation"]["variables"]["input"];
        let supplied = options(
            expected["name"].as_str().unwrap(),
            expected["description"].as_str(),
            expected["targetDate"].as_str(),
        );
        assert_eq!(wire(&supplied)["variables"]["input"], *expected, "{id}");
    }
    // The parser rejects empty values, but a supplied empty string is still a
    // value at this boundary and is sent, unlike an absent one.
    assert_eq!(
        wire(&options("Beta", Some(""), Some("")))["variables"]["input"],
        json!({"projectId": PROJECT, "name": "Beta", "description": "", "targetDate": ""})
    );
    let typed = CreateProjectMilestone::build(CreateProjectMilestoneVariables {
        input: ProjectMilestoneCreateInput {
            project_id: PROJECT.to_owned(),
            name: "Beta".to_owned(),
            description: None,
            target_date: Some(TimelessDate("2026-02-30".to_owned())),
        },
    });
    assert_eq!(
        serde_json::to_value(typed.variables).unwrap()["input"],
        json!({"projectId": PROJECT, "name": "Beta", "targetDate": "2026-02-30"})
    );
}

#[test]
fn strict_decode_requires_the_non_null_milestone() {
    let data = frozen_step("c032-full-optionals")["response"]["data"].clone();
    let decoded: CreateProjectMilestone =
        parse_response(json!({ "data": data }).to_string().as_bytes()).unwrap();
    assert_eq!(
        milestone_create::render(&decoded.project_milestone_create.project_milestone),
        "✓ Created milestone: Server Launch\n  ID: 00000000-0000-4000-9000-000000003202\n  Target Date: 2026-10-31\n  Project: Mobile App\n".as_bytes()
    );
    for payload in [
        json!({"success": true, "projectMilestone": null}),
        json!({"success": true}),
    ] {
        let body = json!({"data": {"projectMilestoneCreate": payload}}).to_string();
        assert!(parse_response::<CreateProjectMilestone>(body.as_bytes()).is_err());
    }
}

#[test]
fn render_skips_null_and_empty_target_dates() {
    let data = frozen_step("c032-uuid-minimal")["response"]["data"].clone();
    let decoded: CreateProjectMilestone =
        parse_response(json!({ "data": data }).to_string().as_bytes()).unwrap();
    let mut milestone = decoded.project_milestone_create.project_milestone;
    let without = "✓ Created milestone: Server Beta\n  ID: 00000000-0000-4000-9000-000000003201\n  Project: Mobile App\n";
    assert_eq!(milestone_create::render(&milestone), without.as_bytes());
    milestone.target_date = Some(TimelessDate(String::new()));
    assert_eq!(milestone_create::render(&milestone), without.as_bytes());
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
async fn uncertain_failures_warn_once_and_rejections_do_not() {
    let cases = [
        (
            Reply::Respond(
                200,
                r#"{"data":{"projectMilestoneCreate":{"success":true,"projectMilestone":null}}}"#,
            ),
            Duration::from_secs(5),
            true,
        ),
        (Reply::Silent, Duration::from_millis(50), true),
        (Reply::DropAfterWrite, Duration::from_secs(5), true),
        (
            Reply::Respond(
                200,
                r#"{"data":{"projectMilestoneCreate":{"success":false,"projectMilestone":{"id":"m","name":"n","targetDate":null,"project":{"id":"p","name":"q"}}}}}"#,
            ),
            Duration::from_secs(5),
            false,
        ),
        (
            Reply::Respond(200, r#"{"errors":[{"message":"Entity not found"}]}"#),
            Duration::from_secs(5),
            false,
        ),
    ];
    for (response, deadline, uncertain) in cases {
        let (endpoint, server) = serve_once(response);
        let error = milestone_create::submit(
            &transport(&endpoint, deadline),
            PROJECT,
            &options("Beta", None, None),
        )
        .await
        .unwrap_err();
        assert_eq!(
            error.message().ends_with("; milestone may already exist"),
            uncertain,
            "{response:?}: {}",
            error.message()
        );
        if matches!(response, Reply::Respond(_, body) if body.contains("\"success\":false")) {
            assert_eq!(error.message(), "Failed to create milestone");
        }
        assert!(!server.join().unwrap(), "create must not retry");
    }
}

#[tokio::test]
async fn refused_connection_is_not_reported_as_a_possible_create() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/graphql", listener.local_addr().unwrap());
    drop(listener);
    let error = milestone_create::submit(
        &transport(&endpoint, Duration::from_secs(5)),
        PROJECT,
        &options("Beta", None, None),
    )
    .await
    .unwrap_err();
    assert!(
        error.message().starts_with("connection to "),
        "{}",
        error.message()
    );
    assert!(!error.message().contains("may already exist"));
}
