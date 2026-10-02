use cynic::QueryBuilder;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

use linear_cli::commands::initiative::list::{
    opening, render_json, render_text, run, status_filter, validate_owner,
};
use linear_cli::graphql::envelope::{GraphQlRequest, parse_response};
use linear_cli::graphql::operations::initiatives::{
    GetInitiatives, GetInitiativesPage, GetInitiativesPageVariables, GetInitiativesVariables,
    GetViewerForInitiatives, GetViewerId, GetViewerIdVariables, InitiativeConnection, LookupUser,
    LookupUserVariables,
};
use linear_cli::graphql::transport::{
    ApiKey, Deadline, EndpointUrl, GraphQlTransport, ResponseCap, TransportConfig,
};
use serde_json::{Value, json};

fn frozen(id: &str) -> Value {
    let path = format!(
        "{}/../../parity/runner/c037-frozen-cases/{id}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_slice(&std::fs::read(path).expect("frozen case")).expect("case JSON")
}

fn frozen_connection(id: &str) -> InitiativeConnection {
    let case = frozen(id);
    let data = &case["graphql"]["groups"][0]["steps"][0]["response"]["data"];
    let typed: GetInitiatives = parse_response(json!({"data":data}).to_string().as_bytes())
        .expect("typed initiative connection");
    typed.initiatives.expect("connection")
}

fn compact(query: &str) -> String {
    query
        .chars()
        .filter(|ch| !ch.is_whitespace() && *ch != ',')
        .collect()
}

#[test]
fn first_document_and_wire_variables_match_frozen_source() {
    for (id, filter, archived) in [
        (
            "c037-default-json",
            json!({"status":{"eq":"Active"}}),
            false,
        ),
        ("c037-all-statuses-json", Value::Null, false),
        (
            "c037-archived-json",
            json!({"status":{"eq":"Active"}}),
            true,
        ),
    ] {
        let case = frozen(id);
        let source = &case["graphql"]["groups"][0]["steps"][0]["operation"];
        let filter = if filter.is_null() {
            None
        } else {
            Some(
                linear_cli::graphql::operations::initiatives::InitiativeFilter {
                    status: Some(linear_cli::graphql::operations::teams::StringComparator {
                        eq: Some("Active".to_owned()),
                        ..Default::default()
                    }),
                    owner: None,
                },
            )
        };
        let request =
            GraphQlRequest::with_variables(GetInitiatives::build(GetInitiativesVariables {
                filter,
                include_archived: Some(archived),
            }));
        assert_eq!(
            request.operation_name.as_deref(),
            Some("GetInitiatives"),
            "{id}"
        );
        assert_eq!(
            compact(&request.query),
            compact(source["document"].as_str().unwrap()),
            "{id}"
        );
        assert_eq!(
            serde_json::to_value(&request).unwrap()["variables"],
            source["variables"],
            "{id}"
        );
        assert!(!request.query.contains("first:"), "{id}");
        assert!(!request.query.contains("after:"), "{id}");
    }
}

#[test]
fn page_document_adds_only_nullable_after_and_owner_documents_keep_source_shape() {
    let case = frozen("c037-first-page-more-json");
    let first = &case["graphql"]["groups"][0]["steps"][0]["operation"];
    let cursor = case["graphql"]["groups"][0]["steps"][0]["response"]["data"]
        ["initiatives"]["pageInfo"]["endCursor"].as_str().unwrap();
    let request =
        GraphQlRequest::with_variables(GetInitiativesPage::build(GetInitiativesPageVariables {
            filter: Some(
                linear_cli::graphql::operations::initiatives::InitiativeFilter {
                    status: Some(linear_cli::graphql::operations::teams::StringComparator {
                        eq: Some("Active".to_owned()),
                        ..Default::default()
                    }),
                    owner: None,
                },
            ),
            include_archived: Some(false),
            after: Some(cursor.to_owned()),
        }));
    assert!(request.query.contains("$after: String"));
    assert!(request.query.contains("after: $after"));
    assert!(!request.query.contains("first:"));
    assert_eq!(
        serde_json::to_value(&request).unwrap()["variables"],
        json!({
            "filter":{"status":{"eq":"Active"}},"includeArchived":false,"after":cursor
        })
    );
    assert!(
        first["document"]
            .as_str()
            .unwrap()
            .contains("GetInitiatives")
    );

    let viewer_id = GraphQlRequest::with_variables(GetViewerId::build(GetViewerIdVariables {}));
    assert_eq!(compact(&viewer_id.query), "queryGetViewerId{viewer{id}}");
    assert_eq!(
        serde_json::to_value(&viewer_id).unwrap()["variables"],
        json!({})
    );
    let lookup = GraphQlRequest::with_variables(LookupUser::build(LookupUserVariables {
        input: "Ada".to_owned(),
    }));
    let source = frozen("c037-owner-display-first");
    let lookup_source = &source["graphql"]["groups"][0]["steps"][0]["operation"];
    assert_eq!(
        compact(&lookup.query),
        compact(lookup_source["document"].as_str().unwrap())
    );
    assert_eq!(
        serde_json::to_value(&lookup).unwrap()["variables"],
        json!({"input":"Ada"})
    );
    let browser = GraphQlRequest::without_variables(GetViewerForInitiatives::build(()));
    assert_eq!(
        compact(&browser.query),
        "queryGetViewerForInitiatives{viewer{organization{urlKey}}}"
    );
    assert!(
        serde_json::to_value(&browser)
            .unwrap()
            .get("variables")
            .is_none()
    );
}

#[test]
fn frozen_json_and_text_keep_selected_names_and_columns() {
    let case = frozen("c037-default-json");
    let connection = frozen_connection("c037-default-json");
    let output = render_json(&connection.nodes, &connection.page_info).expect("JSON");
    assert_eq!(
        String::from_utf8(output).unwrap(),
        case["expected"]["stdout"]["utf8"].as_str().unwrap()
    );
    let case = frozen("c037-default-text");
    let connection = frozen_connection("c037-default-text");
    assert_eq!(
        render_text(&connection.nodes, 120, false),
        case["expected"]["stdout"]["utf8"].as_str().unwrap()
    );
}

#[test]
fn status_owner_and_browser_precedence_are_explicit() {
    assert_eq!(
        status_filter(None, false).unwrap().as_deref(),
        Some("Active")
    );
    assert_eq!(status_filter(None, true).unwrap(), None);
    assert_eq!(
        status_filter(Some("pLaNnEd"), true).unwrap().as_deref(),
        Some("Planned")
    );
    assert!(status_filter(Some("Paused"), false).is_err());
    assert!(validate_owner(Some("https://linear.app/acme/profiles/abc")).is_err());
    assert!(validate_owner(Some("@me")).is_ok());
    let (url, line) = opening("acme", true);
    assert_eq!(url, "https://linear.app/acme/initiatives");
    assert_eq!(
        line,
        b"Opening https://linear.app/acme/initiatives in Linear.app\n"
    );
}

#[test]
fn pipe_percent_content_is_literal_under_v3_policy() {
    let mut case = frozen("c037-default-text");
    let node = &mut case["graphql"]["groups"][0]["steps"][0]["response"]["data"]["initiatives"]["nodes"]
        [0];
    node["slugId"] = json!("%s-%d-%%");
    node["name"] = json!("Project %c %o");
    let data = &case["graphql"]["groups"][0]["steps"][0]["response"]["data"];
    let page: GetInitiatives =
        parse_response(json!({"data":data}).to_string().as_bytes()).expect("typed page");
    let text = render_text(&page.initiatives.unwrap().nodes, 120, false);
    assert!(text.contains("%s-%d-%%"));
    assert!(text.contains("Project %c %o"));
}

fn serve_pages(responses: Vec<Value>) -> (String, thread::JoinHandle<Vec<Value>>) {
    serve_envelopes(
        responses
            .into_iter()
            .map(|data| json!({"data": data}))
            .collect(),
    )
}

fn serve_envelopes(responses: Vec<Value>) -> (String, thread::JoinHandle<Vec<Value>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("localhost listener");
    let endpoint = format!("http://{}/graphql", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        for response in responses {
            let (mut stream, _) = listener.accept().expect("expected page request");
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut chunk = [0u8; 4096];
            let (head_end, length) = loop {
                let count = stream.read(&mut chunk).expect("HTTP request");
                assert!(count > 0, "request closed before headers");
                bytes.extend_from_slice(&chunk[..count]);
                assert!(bytes.len() < 65536, "bounded request");
                if let Some(offset) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                    let end = offset + 4;
                    let header = std::str::from_utf8(&bytes[..end]).unwrap();
                    let length = header
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().unwrap())
                        })
                        .expect("content length");
                    break (end, length);
                }
            };
            while bytes.len() - head_end < length {
                let count = stream.read(&mut chunk).expect("HTTP body");
                assert!(count > 0, "request closed before body");
                bytes.extend_from_slice(&chunk[..count]);
            }
            requests.push(serde_json::from_slice(&bytes[head_end..head_end + length]).unwrap());
            let response = response.to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}", response.len()).unwrap();
        }
        requests
    });
    (endpoint, server)
}

fn fake_transport(endpoint: &str) -> GraphQlTransport {
    GraphQlTransport::new(
        EndpointUrl::parse(endpoint).unwrap(),
        ApiKey::new("lin_api_fake".to_owned()).unwrap(),
        TransportConfig {
            ca_bundle: None,
            deadline: Deadline::new(Duration::from_secs(5)).unwrap(),
            max_response_bytes: ResponseCap::new(65536).unwrap(),
        },
    )
    .unwrap()
}

#[tokio::test]
async fn localhost_transport_emits_exact_first_and_page_documents_and_aggregates() {
    let frozen = frozen("c037-first-page-more-json");
    let path = format!(
        "{}/../../parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1/c037-first-page-more-json.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let golden: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let first = &frozen["graphql"]["groups"][0]["steps"][0];
    let next = &golden["candidate"]["graphqlPages"]["appendedSteps"][0];
    let (endpoint, server) = serve_pages(vec![
        first["response"]["data"].clone(),
        next["response"]["data"].clone(),
    ]);
    let transport = fake_transport(&endpoint);
    let output = run(&transport, Some("Active"), None, false, true, 120, false)
        .await
        .unwrap();
    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        compact(requests[0]["query"].as_str().unwrap()),
        compact(first["operation"]["document"].as_str().unwrap())
    );
    assert_eq!(requests[0]["variables"], first["operation"]["variables"]);
    assert!(!requests[0]["query"].as_str().unwrap().contains("after:"));
    assert!(!requests[0]["query"].as_str().unwrap().contains("first:"));
    assert_eq!(
        compact(requests[1]["query"].as_str().unwrap()),
        compact(next["operation"]["document"].as_str().unwrap())
    );
    assert_eq!(requests[1]["variables"], next["operation"]["variables"]);
    assert_eq!(
        output,
        golden["candidate"]["expected"]["stdout"]["utf8"]
            .as_str()
            .unwrap()
            .as_bytes()
    );
}

#[tokio::test]
async fn later_page_failures_name_the_page_without_returning_partial_output() {
    let frozen = frozen("c037-first-page-more-json");
    let first = frozen["graphql"]["groups"][0]["steps"][0]["response"]["data"].clone();
    for (second, expected) in [
        (
            json!({"errors":[{"message":"second page unavailable"}]}),
            "second page unavailable",
        ),
        (
            json!({"data":{"initiatives":null}}),
            "null initiatives connection",
        ),
    ] {
        let (endpoint, server) = serve_envelopes(vec![json!({"data": first.clone()}), second]);
        let transport = fake_transport(&endpoint);
        let error = run(&transport, Some("Active"), None, false, true, 120, false)
            .await
            .expect_err("later page must fail before returning output");
        let message = error.to_string();
        assert!(message.contains("page 2"), "{message}");
        assert!(message.contains(expected), "{message}");
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[1]["operationName"], "GetInitiativesPage");
    }
}
