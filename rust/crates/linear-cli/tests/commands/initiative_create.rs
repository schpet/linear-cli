use std::io::Cursor;
use std::io::Read;
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

use cynic::MutationBuilder;
use linear_cli::commands::initiative_create::{self, Options, PromptResult};
use linear_cli::graphql::envelope::{GraphQlRequest, parse_response};
use linear_cli::graphql::operations::initiative_create::{
    CreateInitiative, CreateInitiativeVariables, InitiativeCreateInput,
};
use linear_cli::graphql::operations::initiatives::InitiativeStatus;
use linear_cli::graphql::scalars::TimelessDate;
use linear_cli::graphql::transport::{
    ApiKey, Deadline, EndpointUrl, GraphQlTransport, ResponseCap, TransportConfig,
};
use linear_cli::platform::prompt::PromptSession;
use serde_json::{Value, json};

fn frozen(id: &str) -> Value {
    let path = format!(
        "{}/../../parity/runner/c039-frozen-cases/{id}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn operation(input: InitiativeCreateInput) -> Value {
    let request =
        GraphQlRequest::with_variables(CreateInitiative::build(CreateInitiativeVariables {
            input,
        }));
    serde_json::to_value(request).unwrap()
}

fn minimal(name: &str) -> InitiativeCreateInput {
    InitiativeCreateInput {
        name: name.to_owned(),
        description: None,
        status: None,
        owner_id: None,
        target_date: None,
        color: None,
        icon: None,
    }
}

#[test]
fn typed_mutation_matches_source_document_and_omits_absent_fields() {
    let source = frozen("c039-minimal-create");
    let expected = &source["graphql"]["groups"][0]["steps"][0]["operation"];
    let actual = operation(minimal("Initiative 701"));
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
    assert_eq!(actual["operationName"], "CreateInitiative");
}

#[test]
fn typed_mutation_preserves_every_supplied_field_and_date_shape() {
    let source = frozen("c039-full-fields-pipe-interactive");
    let expected = &source["graphql"]["groups"][0]["steps"][0]["operation"]["variables"]["input"];
    let input = InitiativeCreateInput {
        name: expected["name"].as_str().unwrap().to_owned(),
        description: Some(expected["description"].as_str().unwrap().to_owned()),
        status: Some(InitiativeStatus::Active),
        owner_id: None,
        target_date: Some(TimelessDate(
            expected["targetDate"].as_str().unwrap().to_owned(),
        )),
        color: Some(expected["color"].as_str().unwrap().to_owned()),
        icon: Some(expected["icon"].as_str().unwrap().to_owned()),
    };
    assert_eq!(operation(input)["variables"]["input"], *expected);
}

#[test]
fn strict_response_requires_selected_fields() {
    let response =
        frozen("c039-minimal-create")["graphql"]["groups"][0]["steps"][0]["response"]["data"]
            .clone();
    let decoded: CreateInitiative =
        parse_response(json!({"data":response}).to_string().as_bytes()).unwrap();
    assert_eq!(decoded.initiative_create.initiative.slug_id, "INI-701");
    let malformed: Result<CreateInitiative, _> = parse_response(
        json!({"data":{"initiativeCreate":{"success":true,"initiative":{"name":"x"}}}})
            .to_string()
            .as_bytes(),
    );
    assert!(malformed.is_err());
}

#[test]
fn validation_order_and_source_preservation() {
    let mut options = Options {
        name: Some("   ".to_owned()),
        status: Some("aCtIvE".to_owned()),
        target_date: Some("2026-02-30".to_owned()),
        ..Options::default()
    };
    assert_eq!(
        initiative_create::validate(&options).unwrap(),
        Some(InitiativeStatus::Active)
    );
    options.status = Some("Canceled".to_owned());
    options.color = Some("bad".to_owned());
    assert!(
        initiative_create::validate(&options)
            .unwrap_err()
            .message
            .starts_with("Invalid status")
    );
    options.status = None;
    assert!(
        initiative_create::validate(&options)
            .unwrap_err()
            .message
            .starts_with("Color must")
    );
    options.color = None;
    options.target_date = Some("tomorrow".to_owned());
    assert!(
        initiative_create::validate(&options)
            .unwrap_err()
            .message
            .starts_with("Target date")
    );
}

#[test]
fn line_script_collects_trimmed_answers_and_defaults() {
    let mut options = Options::default();
    let mut session =
        PromptSession::script(Cursor::new(b"  New  \n description \n\n\n\n\n"), Vec::new());
    assert_eq!(
        initiative_create::prompt(&mut options, &mut session).unwrap(),
        PromptResult::Complete
    );
    assert_eq!(options.name.as_deref(), Some("New"));
    assert_eq!(options.description.as_deref(), Some("description"));
    assert_eq!(options.status.as_deref(), Some("Planned"));
    assert_eq!(options.owner, None);
    assert_eq!(options.target_date, None);
    assert_eq!(options.color, None);
    assert!(
        session
            .into_output()
            .unwrap()
            .starts_with(b"? Initiative name:")
    );
}

#[test]
fn line_script_rejects_raw_invalid_custom_color_before_write() {
    let mut options = Options::default();
    let mut session =
        PromptSession::script(Cursor::new(b"New\n\n\n\n\ncustom\n #ABCDEF\n"), Vec::new());
    let error = initiative_create::prompt(&mut options, &mut session).unwrap_err();
    assert!(error.message.contains("Please enter a valid hex color"));
    assert!(options.color.is_none());
}

#[tokio::test]
async fn timed_out_create_warns_about_ambiguous_write_without_retrying() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/graphql", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let mut request = [0_u8; 4096];
        let size = stream.read(&mut request).unwrap();
        assert!(size > 0);
        thread::sleep(Duration::from_millis(250));
        listener.set_nonblocking(true).unwrap();
        assert!(
            listener.accept().is_err(),
            "create must not retry after timeout"
        );
        1
    });
    let transport = GraphQlTransport::new(
        EndpointUrl::parse(&endpoint).unwrap(),
        ApiKey::new("lin_api_fake".to_owned()).unwrap(),
        TransportConfig {
            ca_bundle: None,
            deadline: Deadline::new(Duration::from_millis(50)).unwrap(),
            max_response_bytes: ResponseCap::new(65536).unwrap(),
        },
    )
    .unwrap();
    let error = initiative_create::submit_create(
        &transport,
        Options {
            name: Some("Ambiguous".to_owned()),
            ..Options::default()
        },
        None,
        None,
    )
    .await
    .unwrap_err();
    assert!(error.message.contains("initiative may already exist"));
    assert_eq!(server.join().unwrap(), 1);
}
