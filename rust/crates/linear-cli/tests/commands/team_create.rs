use std::io::{Cursor, Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

use linear_cli::commands::team_create::{self, Mode, Options, PromptResult};
use linear_cli::graphql::envelope::parse_response;
use linear_cli::graphql::operations::team_create::CreateTeam;
use linear_cli::graphql::transport::{
    ApiKey, Deadline, EndpointUrl, GraphQlTransport, ResponseCap, TransportConfig,
};
use linear_cli::platform::prompt::PromptSession;
use serde_json::{Value, json};

fn frozen_step(id: &str) -> Value {
    let path = format!(
        "{}/../../parity/runner/c012-frozen-cases/{id}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let case: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    case["graphql"]["groups"][0]["steps"][0].clone()
}

fn options(
    name: Option<&str>,
    description: Option<&str>,
    key: Option<&str>,
    private: bool,
) -> Options {
    Options {
        name: name.map(str::to_owned),
        description: description.map(str::to_owned),
        key: key.map(str::to_owned),
        private,
    }
}

fn wire(options: &Options) -> Value {
    serde_json::to_value(team_create::request(options).unwrap()).unwrap()
}

#[test]
fn typed_mutation_matches_source_document_and_frozen_variables() {
    let compact = |value: &str| {
        value
            .chars()
            .filter(|ch| !ch.is_whitespace() && *ch != ',')
            .collect::<String>()
    };
    for (id, supplied) in [
        (
            "c012-minimal-public",
            options(Some("Public 1202"), None, None, false),
        ),
        (
            "c012-all-flags",
            options(
                Some("Platform 1201"),
                Some("Owns **builds**"),
                Some("plt"),
                true,
            ),
        ),
        (
            "c012-null-team",
            options(Some("Ghost 1206"), None, Some("GHO"), false),
        ),
    ] {
        let expected = &frozen_step(id)["operation"];
        let actual = wire(&supplied);
        assert_eq!(
            compact(actual["query"].as_str().unwrap()),
            compact(expected["document"].as_str().unwrap()),
            "{id}"
        );
        assert_eq!(actual["variables"], expected["variables"], "{id}");
        assert_eq!(actual["operationName"], "CreateTeam");
    }
}

#[test]
fn empty_optionals_and_false_private_are_omitted() {
    assert_eq!(
        wire(&options(Some("A"), Some(""), Some(""), false))["variables"]["input"],
        json!({"name": "A"})
    );
    assert_eq!(
        wire(&options(Some("A"), None, None, true))["variables"]["input"],
        json!({"name": "A", "private": true})
    );
}

#[test]
fn only_an_interactive_run_without_create_flags_prompts() {
    assert!(team_create::interactive(false, true));
    assert!(!team_create::interactive(true, true));
    assert!(!team_create::interactive(false, false));
    let none = Options::default();
    assert_eq!(team_create::mode(&none, true), Mode::Prompt);
    assert_eq!(team_create::mode(&none, false), Mode::Flags);
    for flagged in [
        options(Some("A"), None, None, false),
        options(None, Some("d"), None, false),
        options(None, None, Some("K"), false),
        options(None, None, None, true),
    ] {
        assert_eq!(
            team_create::mode(&flagged, true),
            Mode::Flags,
            "{flagged:?}"
        );
    }
}

#[test]
fn flag_mode_requires_a_name_before_announcing() {
    for missing in [
        Options::default(),
        options(None, None, None, true),
        options(Some(""), None, None, false),
    ] {
        let error = team_create::required_name(&missing).unwrap_err();
        assert_eq!(
            error.message(),
            "Team name is required when not using interactive mode"
        );
        assert_eq!(
            error.hint(),
            Some("Use --name or run without any flags for interactive mode.")
        );
        assert!(team_create::request(&missing).is_err());
    }
    assert_eq!(
        team_create::announcement("Ops", Mode::Flags),
        b"Creating team \"Ops\"\n"
    );
    assert_eq!(
        team_create::announcement("Ops", Mode::Prompt),
        b"\nCreating team \"Ops\"...\n"
    );
}

fn scripted(
    input: &str,
) -> (
    Result<PromptResult, linear_cli::error::Error>,
    Options,
    String,
) {
    let mut options = Options::default();
    let mut session = PromptSession::script(Cursor::new(input.as_bytes().to_vec()), Vec::new());
    let result = team_create::prompt(&mut options, &mut session);
    let output = String::from_utf8(session.into_output().unwrap()).unwrap();
    (result, options, output)
}

#[test]
fn prompts_trim_answers_omit_empty_optionals_and_default_to_public() {
    let (result, answered, transcript) = scripted("  Ops  \nRuns things\nops\nprivate\n");
    assert_eq!(result.unwrap(), PromptResult::Complete);
    assert_eq!(
        answered,
        options(Some("Ops"), Some("Runs things"), Some("ops"), true)
    );
    assert_eq!(
        transcript,
        "? Team name:\n? Team name: › Ops\n\
         ? Team description (optional):\n? Team description (optional): › Runs things\n\
         ? Team key (optional, will be generated from name if not provided):\n\
         ? Team key (optional, will be generated from name if not provided): › ops\n\
         ? Team visibility: (Public)\n? Team visibility: › Private\n"
    );
    let (result, answered, _) = scripted("Ops\n  \n\n\n");
    assert_eq!(result.unwrap(), PromptResult::Complete);
    assert_eq!(answered, options(Some("Ops"), None, None, false));
    assert_eq!(
        wire(&answered)["variables"]["input"],
        json!({"name": "Ops"})
    );
    let (_, answered, _) = scripted("Ops\n\n\n1\n");
    assert!(!answered.private);
}

#[test]
fn blank_name_fails_validation_and_eof_stops_prompting() {
    let (result, _, _) = scripted("   \n");
    let error = result.unwrap_err();
    assert_eq!(error.message(), "Team name is required");
    for partial in ["", "Ops\n", "Ops\n\n", "Ops\n\n\n"] {
        let (result, _, _) = scripted(partial);
        assert_eq!(result.unwrap(), PromptResult::EndOfInput, "{partial:?}");
    }
    let (result, _, _) = scripted("Ops\n\n\nsecret\n");
    assert!(result.is_err());
}

fn decode(payload: Value) -> CreateTeam {
    parse_response(
        json!({"data": {"teamCreate": payload}})
            .to_string()
            .as_bytes(),
    )
    .unwrap()
}

#[test]
fn render_checks_success_before_the_nullable_team() {
    let created = frozen_step("c012-all-flags")["response"]["data"]["teamCreate"].clone();
    assert_eq!(
        team_create::render(&decode(created).team_create).unwrap(),
        "✓ Created team PLT: Server Platform\n".as_bytes()
    );
    let refused = frozen_step("c012-false-success")["response"]["data"]["teamCreate"].clone();
    for (payload, message) in [
        (refused, "Team creation failed"),
        (
            json!({"success": false, "team": null}),
            "Team creation failed",
        ),
        (
            json!({"success": true, "team": null}),
            "Team creation failed - no team returned",
        ),
    ] {
        let error = team_create::render(&decode(payload).team_create).unwrap_err();
        assert_eq!(error.message(), message);
    }
    let body = json!({"data": {"teamCreate": {"success": true, "team": {"id": "t", "name": "n"}}}});
    assert!(parse_response::<CreateTeam>(body.to_string().as_bytes()).is_err());
}

/// Accept one request, reply (or close without replying), and report
/// whether a second connection arrived afterwards.
fn serve_once(reply: Option<&'static str>) -> (String, thread::JoinHandle<bool>) {
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
        if let Some(body) = reply {
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(head.as_bytes()).unwrap();
            stream.write_all(body.as_bytes()).unwrap();
        }
        drop(stream);
        thread::sleep(Duration::from_millis(100));
        listener.set_nonblocking(true).unwrap();
        listener.accept().is_ok()
    });
    (endpoint, server)
}

fn transport(endpoint: &str) -> GraphQlTransport {
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
async fn post_write_failures_warn_once_and_reported_failures_do_not() {
    for (reply, uncertain) in [
        (None, true),
        (
            Some(r#"{"data":{"teamCreate":{"success":true,"team":{"id":"t"}}}}"#),
            true,
        ),
        (
            Some(r#"{"data":{"teamCreate":{"success":false,"team":null}}}"#),
            false,
        ),
        (
            Some(r#"{"errors":[{"message":"Key already in use"}]}"#),
            false,
        ),
    ] {
        let (endpoint, server) = serve_once(reply);
        let error = team_create::submit(
            &transport(&endpoint),
            &options(Some("Ops"), None, None, false),
        )
        .await
        .unwrap_err();
        assert_eq!(
            error.message().ends_with("; team may already exist"),
            uncertain,
            "{reply:?}: {}",
            error.message()
        );
        assert!(!server.join().unwrap(), "create must not retry");
    }
}
