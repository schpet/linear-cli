//! public vectors; no test/candidate claims.
use cynic::{MutationBuilder, QueryBuilder};
use linear_cli::{
    commands::{project_create as create, project_write as shared},
    graphql::{
        envelope::{GraphQlRequest, parse_response},
        operations::project_write::*,
        transport::RawHttpResponse,
    },
};
use serde_json::json;

#[test]
fn full_scalar_body_and_interactive_boundaries() {
    for (input, want) in [
        ("none", 0),
        ("URGENT", 1),
        ("high", 2),
        ("medium", 3),
        ("low", 4),
    ] {
        assert_eq!(shared::priority(input).unwrap(), want);
    }
    assert!(shared::priority("1").is_err());
    for value in [
        "planned",
        "started",
        "in progress",
        "paused",
        "completed",
        "canceled",
        "backlog",
    ] {
        assert!(shared::status_type(value).is_ok());
    }
    for (value, valid) in [
        ("2026-99-99", true),
        ("2026-1-01", false),
        ("２０２６-01-01", false),
        ("", true),
    ] {
        assert_eq!(shared::date(Some(value), "Start").is_ok(), valid);
    }
    assert_eq!(
        shared::description(Some(&"😀".repeat(127)), None)
            .unwrap()
            .unwrap()
            .encode_utf16()
            .count(),
        254
    );
    assert!(shared::description(Some(&"😀".repeat(128)), None).is_err());
    assert_eq!(
        shared::content(Some("\u{feff}# raw\r\n"), None).unwrap(),
        Some("\u{feff}# raw\r\n".to_owned())
    );
    assert!(shared::content(Some(""), Some("unused")).is_err());
    let fields = create::Fields::default();
    assert!(create::interactive(&fields, false, true));
    assert!(!create::interactive(&fields, true, false));
    assert!(!create::interactive(
        &create::Fields {
            name: Some("n".to_owned()),
            ..Default::default()
        },
        false,
        true
    ));
    assert!(!create::interactive(
        &create::Fields {
            teams: vec!["T".to_owned()],
            ..Default::default()
        },
        false,
        true
    ));
}
#[test]
fn source_client_error_envelope_and_plain_nonjson_class_are_distinct() {
    let message = "line one\nline two: {\"metadata\":true}";
    assert_eq!(
        create::warning(message, true),
        format!("\nWarning: Failed to add project to initiative: ClientError: {message}\n")
    );
    assert_eq!(
        create::warning(message, false),
        format!("\nWarning: Failed to add project to initiative: {message}\n")
    );
    let mut response = RawHttpResponse {
        status: reqwest::StatusCode::OK,
        headers: reqwest::header::HeaderMap::new(),
        body: b"plain".to_vec(),
    };
    assert!(!create::client_error_branch(&response));
    response.headers.insert(
        reqwest::header::CONTENT_TYPE,
        reqwest::header::HeaderValue::from_static("application/json"),
    );
    assert!(create::client_error_branch(&response));
    response.headers.clear();
    response.headers.insert(
        reqwest::header::CONTENT_TYPE,
        reqwest::header::HeaderValue::from_static("text/plain"),
    );
    response.headers.append(
        reqwest::header::CONTENT_TYPE,
        reqwest::header::HeaderValue::from_bytes(b"Application/JSON; note=\xff").unwrap(),
    );
    assert!(create::client_error_branch(&response));
    response.status = reqwest::StatusCode::BAD_GATEWAY;
    response.headers.clear();
    assert!(create::client_error_branch(&response));
}
#[test]
fn raw_observer_preserves_server_metadata_numbers_and_body() {
    let request =
        GraphQlRequest::with_variables(AddProjectToInitiativeForCreate::build(LinkVariables {
            input: InitiativeLinkInput {
                initiative_id: "i".to_owned(),
                project_id: "p".to_owned(),
            },
        }));
    let body = r#"{"data":null,"errors":[{"message":"line\nbreak","extensions":{"10":1,"2":2,"float":1e-7,"large":9007199254740993,"negativeZero":-0}}],"extensions":{"x":0.000001}}"#;
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::CONTENT_TYPE,
        reqwest::header::HeaderValue::from_static("application/json"),
    );
    let response = RawHttpResponse {
        status: reqwest::StatusCode::OK,
        headers,
        body: body.as_bytes().to_vec(),
    };
    let message = linear_cli::graphql::bulk_error::source_error(&response, &request)
        .unwrap_or_else(|error| panic!("{}", error.into_error()))
        .unwrap();
    assert!(message.starts_with("line\nbreak: {\"response\":{\"data\":null,\"errors\":"));
    assert!(message.contains("\"2\":2,\"10\":1"));
    assert!(message.contains("\"large\":9007199254740993"));
    assert!(message.contains("\"float\":1e-7"));
    assert!(message.contains("\"x\":1e-6"));
    assert!(message.contains("\"headers\":{},\"body\":"));
    assert!(message.contains("\"request\":{\"query\":"));
    assert!(
        message.contains("\"variables\":{\"input\":{\"initiativeId\":\"i\",\"projectId\":\"p\"}}")
    );
    let response = RawHttpResponse {
        status: reqwest::StatusCode::BAD_GATEWAY,
        headers: reqwest::header::HeaderMap::new(),
        body: vec![b'e', 0xff],
    };
    let message = linear_cli::graphql::bulk_error::source_error(&response, &request)
        .unwrap_or_else(|error| panic!("{}", error.into_error()))
        .unwrap();
    assert!(message.starts_with(
        "GraphQL Error (Code: 502): {\"response\":{\"status\":502,\"headers\":{},\"body\":\"e�\"}"
    ));
}
#[tokio::test]
async fn all_caught_postcreate_join_failures_warn_instead_of_becoming_fatal() {
    for body in [
        r#"{"data":{"initiativeToProjectCreate":null}}"#,
        r#"{"data":{"initiativeToProjectCreate":{"success":"true"}}}"#,
        "not json",
        r#"{"errors":{"message":"shape"}}"#,
    ] {
        let (transport, server) = super::delete_server::serve(body);
        let result = create::join(
            &transport,
            InitiativeLinkInput {
                initiative_id: "i".to_owned(),
                project_id: "p".to_owned(),
            },
        )
        .await;
        assert!(
            matches!(result,create::JoinOutcome::Warning(message) if message.starts_with("\nWarning: Failed to add project to initiative: ")&&!message.contains("ClientError: "))
        );
        server.join().unwrap();
    }
    let (transport, server) =
        super::delete_server::serve_with_content_type("plain invalid", "text/plain");
    let result = create::join(
        &transport,
        InitiativeLinkInput {
            initiative_id: "i".to_owned(),
            project_id: "p".to_owned(),
        },
    )
    .await;
    assert!(
        matches!(result,create::JoinOutcome::Warning(message) if message.contains("Invalid execution result")&&!message.contains("ClientError: "))
    );
    server.join().unwrap();
}
#[test]
fn typed_create_payload_allows_only_schema_nulls_and_fields_are_omitted_precisely() {
    let body = json!({"data":{"projectCreate":{"success":true,"project":null}}});
    assert!(parse_response::<CreateProject>(body.to_string().as_bytes()).is_ok());
    for bad in [
        json!(null),
        json!({}),
        json!({"success":"true","project":null}),
    ] {
        assert!(
            parse_response::<CreateProject>(
                json!({"data":{"projectCreate":bad}}).to_string().as_bytes()
            )
            .is_err()
        );
    }
    let input = ProjectCreateInput {
        name: "n".to_owned(),
        team_ids: vec!["t".to_owned()],
        priority: Some(0),
        content: Some("".to_owned()),
        ..Default::default()
    };
    let wire = serde_json::to_value(input).unwrap();
    assert_eq!(
        wire,
        json!({"name":"n","teamIds":["t"],"content":"","priority":0})
    );
    let request = GraphQlRequest::without_variables(GetProjectStatuses::build(()));
    assert!(
        serde_json::to_value(request)
            .unwrap()
            .get("variables")
            .is_none()
    );
}

#[tokio::test]
async fn caught_nonclient_join_preserves_entire_success_json_and_exit_contract() {
    let payload = CreatedProjectPayload {
        success: true,
        project: Some(CreatedProject {
            id: cynic::Id::new("project"),
            slug_id: "slug".to_owned(),
            name: "Created".to_owned(),
            url: "url".to_owned(),
        }),
    };
    let mut expected = serde_json::to_vec_pretty(&payload).unwrap();
    expected.push(b'\n');
    let api_key = linear_cli::auth::ApiKeyInput::Absent;
    let scope = linear_cli::refs::WorkspaceScope {
        cli_workspace: None,
        sourced_workspace: None,
        default_workspace: None,
        api_key: api_key.clone(),
    };
    for body in [r#"{"data":{"initiativeToProjectCreate":null}}"#, "bad json"] {
        let (transport, server) = super::delete_server::serve(body);
        let output = create::followup_and_output(
            &transport,
            &scope,
            &payload,
            Some("00000000-0000-4000-9000-000000000001"),
            true,
        )
        .await
        .unwrap();
        assert_eq!(output.stdout, expected);
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .starts_with("\nWarning: Failed to add project to initiative: ")
        );
        // Completion is Ok, so app returns source success instead of a fatal typed error.
        assert_eq!(
            server.join().unwrap()["variables"]["input"]["projectId"],
            "project"
        );
    }
}

#[tokio::test]
async fn native_network_and_response_cap_failures_use_nonclient_warning_envelope() {
    let input = || InitiativeLinkInput {
        initiative_id: "i".to_owned(),
        project_id: "p".to_owned(),
    };
    let (transport, server) = super::project_write_server::serve(vec![]);
    server.join().unwrap();
    assert!(
        matches!(create::join(&transport,input()).await,create::JoinOutcome::Warning(message) if message.starts_with("\nWarning: Failed to add project to initiative: ")&&!message.contains("ClientError: "))
    );
    let (transport, server) = super::project_write_server::serve(vec!["x".repeat(70000)]);
    assert!(
        matches!(create::join(&transport,input()).await,create::JoinOutcome::Warning(message) if message.contains("limit")&&!message.contains("ClientError: "))
    );
    server.join().unwrap();
}

#[tokio::test]
async fn duplicate_status_types_keep_both_choices_and_refetch_first_type_id() {
    use linear_cli::platform::prompt::{PromptOutcome, PromptSession};
    let statuses = json!({"data":{"projectStatuses":{"nodes":[
        {"id":"first-planned","name":"Early plan","type":"planned"},
        {"id":"second-planned","name":"Later plan","type":"planned"},
        {"id":"completed","name":"Done","type":"completed"}
    ]}}});
    let (transport, server) = super::project_write_server::serve(vec![
        statuses.to_string(),
        json!({"data":{"teams":{"nodes":[{"id":"team-id","key":"SRC","name":"Source"}]}}})
            .to_string(),
        json!({"data":{"viewer":{"id":"lead-id"}}}).to_string(),
        statuses.to_string(),
    ]);
    let fields = create::Fields {
        name: Some("New".to_owned()),
        description: Some("Summary".to_owned()),
        teams: vec!["SRC".to_owned()],
        lead: Some("@me".to_owned()),
        start_date: Some("2026-01-01".to_owned()),
        target_date: Some("2026-02-01".to_owned()),
        ..Default::default()
    };
    let mut prompt = PromptSession::script(&b"2\n"[..], Vec::new());
    let outcome = create::prompt(&mut prompt, &transport, fields, None)
        .await
        .unwrap();
    let PromptOutcome::Submitted(fields) = outcome else {
        panic!("status selection did not complete")
    };
    assert_eq!(fields.status.as_deref(), Some("planned"));
    let api_key = linear_cli::auth::ApiKeyInput::Absent;
    let scope = linear_cli::refs::WorkspaceScope {
        cli_workspace: None,
        sourced_workspace: None,
        default_workspace: None,
        api_key: api_key.clone(),
    };
    let input = create::input(&transport, &scope, &fields, None)
        .await
        .unwrap();
    assert_eq!(input.status_id.as_deref(), Some("first-planned"));
    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 4);
    assert!(
        requests[0]["query"]
            .as_str()
            .unwrap()
            .contains("GetProjectStatuses")
    );
    assert!(
        requests[3]["query"]
            .as_str()
            .unwrap()
            .contains("GetProjectStatuses")
    );
}
