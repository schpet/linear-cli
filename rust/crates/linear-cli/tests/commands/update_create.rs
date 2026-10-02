//! Public boundaries with actual captured wire requests and strict post-send outcomes.
use linear_cli::{
    commands::{
        initiative_view::Reference,
        text_input,
        update_create::{self as command, Fields, Health, Mode},
    },
    platform::prompt::{PromptOutcome, PromptSession},
};
use serde_json::{Value, json};
use std::{
    io::{self, Read},
    time::{SystemTime, UNIX_EPOCH},
};
fn payload(mode: Mode, success: Value, health: Value) -> String {
    let parent = match mode {
        Mode::Project => "project",
        Mode::Initiative => "initiative",
    };
    let root = match mode {
        Mode::Project => "projectUpdateCreate",
        Mode::Initiative => "initiativeUpdateCreate",
    };
    let field = match mode {
        Mode::Project => "projectUpdate",
        Mode::Initiative => "initiativeUpdate",
    };
    json!({"data":{root:{"success":success,field:{"id":"update-1","body":"server body","health":health,"url":"https://example.test/update","createdAt":"2026-09-30T00:00:00Z",parent:{"name":"Server parent","slugId":"server-slug"}}}}}).to_string()
}
#[test]
fn attended_policy_and_health_are_exact_without_broad_stdout_gate() {
    for mode in [Mode::Project, Mode::Initiative] {
        assert!(command::Health::parse(Some("OnTrack"), mode).is_err());
        assert_eq!(
            command::Health::parse(Some("onTrack"), mode).unwrap(),
            Some(Health::OnTrack)
        );
        assert!(command::Health::parse(Some(""), mode).unwrap().is_none());
    }
    assert!(command::attended(false, true, true, None, None, None).unwrap());
    assert!(command::attended(false, true, true, Some(""), Some(""), Some("")).unwrap());
    assert!(!command::attended(false, false, true, None, None, None).unwrap());
    assert!(!command::attended(false, true, false, None, None, None).unwrap());
    assert!(!command::attended(false, true, true, None, None, Some("atRisk")).unwrap());
    for (stdin, stdout) in [(false, false), (true, false), (false, true)] {
        assert!(command::attended(true, stdin, stdout, Some("body"), None, None).is_err());
    }
}
#[test]
fn typed_input_omits_project_empty_but_retains_initiative_empty_and_raw_text() {
    let project = command::project_request(
        "parent",
        Fields {
            body: Some("".to_owned()),
            health: Some(Health::OffTrack),
        },
    );
    assert_eq!(
        serde_json::to_value(project.variables).unwrap(),
        json!({"input":{"projectId":"parent","health":"offTrack"}})
    );
    let initiative = command::initiative_request(
        "parent",
        Fields {
            body: Some("".to_owned()),
            health: None,
        },
    );
    assert_eq!(
        serde_json::to_value(initiative.variables).unwrap(),
        json!({"input":{"initiativeId":"parent","body":""}})
    );
    let project = command::project_request(
        "parent",
        Fields {
            body: Some("  raw\r\n界 😀  ".to_owned()),
            health: None,
        },
    );
    assert_eq!(
        serde_json::to_value(project.variables).unwrap(),
        json!({"input":{"projectId":"parent","body":"  raw\r\n界 😀  "}})
    );
}
struct BrokenRead;
impl Read for BrokenRead {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("source optional read failure"))
    }
}
#[test]
fn stdin_content_is_read_whole_without_splitting() {
    assert_eq!(
        text_input::read_stdin(&b"\xef\xbb\xbfHello, world\n  indented\n"[..]).unwrap(),
        Some("Hello, world\n  indented".to_owned())
    );
    assert_eq!(text_input::read_stdin(&b" \n"[..]).unwrap(), None);
    assert!(text_input::read_stdin(&b"A\xff"[..]).is_err());
    assert!(text_input::read_stdin(BrokenRead).is_err());
}
#[test]
fn file_policy_keeps_raw_body_and_stage_specific_missing_errors() {
    let root = std::env::temp_dir().join(format!(
        "update-create-public-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let body = root.join("body");
    std::fs::write(&body, b"\xef\xbb\xbfraw\r\n").unwrap();
    for mode in [Mode::Project, Mode::Initiative] {
        assert_eq!(
            command::file(body.to_str().unwrap(), mode, false).unwrap(),
            "raw\r\n"
        );
        let missing = root.join("missing");
        assert_eq!(
            command::file(missing.to_str().unwrap(), mode, false)
                .unwrap_err()
                .message(),
            format!("File not found: {}", missing.display())
        );
        assert!(
            command::file(root.to_str().unwrap(), mode, false)
                .unwrap_err()
                .message()
                .starts_with("Failed to read body file: ")
        );
    }
    let missing = root.join("missing");
    assert!(
        command::file(missing.to_str().unwrap(), Mode::Initiative, true)
            .unwrap_err()
            .message()
            .starts_with("Failed to read file: ")
    );
    assert_eq!(
        command::file(missing.to_str().unwrap(), Mode::Project, true)
            .unwrap_err()
            .message(),
        format!("File not found: {}", missing.display())
    );
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn prompt_health_defaults_and_all_explicit_choices_map_to_typed_inputs() {
    for mode in [Mode::Project, Mode::Initiative] {
        let mut session = PromptSession::script(&b"\n"[..], Vec::new());
        assert_eq!(
            command::prompt_health(&mut session, mode).unwrap(),
            PromptOutcome::Submitted(None)
        );
        let out = String::from_utf8(session.into_output().unwrap()).unwrap();
        assert!(out.contains(match mode {
            Mode::Project => "No change",
            Mode::Initiative => "Skip (no change)",
        }));
        for (token, expected) in [
            ("onTrack", Health::OnTrack),
            ("atRisk", Health::AtRisk),
            ("offTrack", Health::OffTrack),
        ] {
            let input = format!("{token}\n");
            let mut session = PromptSession::script(input.as_bytes(), Vec::new());
            assert_eq!(
                command::prompt_health(&mut session, mode).unwrap(),
                PromptOutcome::Submitted(Some(expected))
            );
        }
    }
}
#[tokio::test]
async fn full_typed_mutations_print_server_metadata_and_unknown_returned_health() {
    for mode in [Mode::Project, Mode::Initiative] {
        let (transport, worker) = super::project_write_server::serve(vec![payload(
            mode,
            json!(true),
            json!("futureHealth"),
        )]);
        let out = command::create(
            &transport,
            "parent",
            Fields {
                body: Some("requested".to_owned()),
                health: Some(Health::OnTrack),
            },
            mode,
        )
        .await
        .unwrap();
        assert_eq!(out,b"Created status update for: Server parent\nHealth: futureHealth\nhttps://example.test/update\n");
        let requests = worker.join().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0]["variables"]["input"][match mode {
                Mode::Project => "projectId",
                Mode::Initiative => "initiativeId",
            }],
            "parent"
        );
        assert_eq!(requests[0]["variables"]["input"]["body"], "requested");
        assert_eq!(requests[0]["variables"]["input"]["health"], "onTrack");
        let (transport, worker) =
            super::project_write_server::serve(vec![payload(mode, json!(false), json!("onTrack"))]);
        assert_eq!(
            command::create(&transport, "parent", Fields::default(), mode)
                .await
                .unwrap_err()
                .message(),
            mode.context()
        );
        assert_eq!(worker.join().unwrap().len(), 1);
    }
}
#[tokio::test]
async fn decode_first_keeps_false_null_precedence_and_only_boolean_true_confirms_effect() {
    for mode in [Mode::Project, Mode::Initiative] {
        let root = match mode {
            Mode::Project => "projectUpdateCreate",
            Mode::Initiative => "initiativeUpdateCreate",
        };
        let field = match mode {
            Mode::Project => "projectUpdate",
            Mode::Initiative => "initiativeUpdate",
        };
        for success in [json!(false), json!(true), json!("true"), Value::Null] {
            let body = json!({"data":{root:{"success":success.clone(),field:null}}}).to_string();
            let (transport, worker) = super::project_write_server::serve(vec![body]);
            let error = command::create(&transport, "parent", Fields::default(), mode)
                .await
                .unwrap_err();
            assert!(
                error
                    .message()
                    .starts_with("UPDATE-CREATE-UNEXPECTED-SHAPE:")
            );
            assert!(error.message().contains(if success == json!(true) {
                "creation confirmed by success:true"
            } else {
                "creation outcome unknown"
            }));
            assert_eq!(worker.join().unwrap().len(), 1);
        }
    }
}
#[tokio::test]
async fn initiative_specific_slug_name_first_match_and_display_fallback_are_ordered() {
    let (transport,worker)=super::project_write_server::serve(vec![json!({"errors":[{"message":"slug unavailable"}]}).to_string(),json!({"data":{"initiatives":{"nodes":[{"id":"first","name":"First"},{"id":"second","name":"Second"}]}}}).to_string(),json!({"errors":[{"message":"display unavailable"}]}).to_string()]);
    let id = command::initiative_id(
        &transport,
        &Reference::NameOrSlug("Exact".to_owned()),
        "Exact",
    )
    .await
    .unwrap();
    assert_eq!(id, "first");
    assert_eq!(
        command::initiative_name(&transport, &id, "Exact").await,
        "Exact"
    );
    let requests = worker.join().unwrap();
    assert_eq!(requests.len(), 3);
    assert!(
        requests[0]["query"]
            .as_str()
            .unwrap()
            .starts_with("query GetInitiativeBySlugForStatusUpdate")
    );
    assert!(
        requests[1]["query"]
            .as_str()
            .unwrap()
            .starts_with("query GetInitiativeByNameForStatusUpdate")
    );
    assert!(
        requests[2]["query"]
            .as_str()
            .unwrap()
            .starts_with("query GetInitiativeNameForStatusUpdate")
    );
    let (transport, worker) = super::project_write_server::serve(vec![
        json!({"data":{"initiatives":{"nodes":[{"id":null,"slugId":"x"}]}}}).to_string(),
    ]);
    let error = command::initiative_id(
        &transport,
        &Reference::NameOrSlug("Exact".to_owned()),
        "Exact",
    )
    .await
    .unwrap_err();
    assert!(
        error
            .message()
            .starts_with("UPDATE-CREATE-UNEXPECTED-SHAPE:")
    );
    assert_eq!(worker.join().unwrap().len(), 1);
}

#[tokio::test]
async fn url_lookup_failure_has_only_outer_mode_context() {
    let (transport, worker) = super::project_write_server::serve(vec![
        json!({"errors":[{"message":"URL lookup failed"}]}).to_string(),
    ]);
    let error = command::initiative_id(
        &transport,
        &Reference::UrlSlug("known-slug".to_owned()),
        "https://linear.app/acme/initiative/known-slug",
    )
    .await
    .unwrap_err()
    .context(Mode::Initiative.context());
    assert_eq!(
        error.to_string(),
        "Failed to create initiative status update: URL lookup failed"
    );
    let requests = worker.join().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["operationName"], "ResolveInitiativeBySlug");
    assert_eq!(
        requests[0]["variables"],
        json!({"slugId":"known-slug", "includeArchived":false})
    );
}
