use linear_cli::{
    commands::initiative::update::{self as command, Fields},
    graphql::operations::initiative_update::CurrentInitiative,
    platform::{
        prompt::{PlainOption, PlainSelect, PromptOutcome, PromptSession},
        prompt_text::TextOptions,
    },
};
use serde_json::json;
use std::io::Cursor;
fn current() -> CurrentInitiative {
    serde_json::from_value(json!({"id":"not-the-mutation-id","slugId":"INI-1","name":" raw 界 ","description":"line1\r\nline2\t","status":"paused","targetDate":"nonsense","color":"#raw","icon":null,"owner":null})).unwrap()
}
#[test]
fn any_status_and_raw_fields_are_update_specific_not_create_validation() {
    for status in ["PlAnNeD", "ACTIVE", "Completed", "PAUSED", "Future 界", "İ"] {
        let fields = Fields {
            status: Some(status.to_owned()),
            name: Some("".to_owned()),
            description: Some(" raw\r\n界 ".to_owned()),
            target_date: Some("not a date".to_owned()),
            ..Fields::default()
        };
        let actual = serde_json::to_value(fields.input(None)).unwrap();
        assert_eq!(
            actual,
            json!({"status":status.to_lowercase(),"name":"","description":" raw\r\n界 ","targetDate":"not a date"})
        );
    }
    for (interactive, stdout, expected) in [
        (true, true, true),
        (false, true, false),
        (true, false, false),
    ] {
        assert_eq!(
            Fields::default().should_prompt(interactive, stdout),
            expected
        );
    }
    for field in 0..7 {
        let mut fields = Fields::default();
        let values = [
            &mut fields.name,
            &mut fields.description,
            &mut fields.status,
            &mut fields.owner,
            &mut fields.target_date,
            &mut fields.color,
            &mut fields.icon,
        ];
        *values.into_iter().nth(field).unwrap() = Some(" ".to_owned());
        assert!(!fields.should_prompt(true, true));
    }
}
#[test]
fn escaped_default_is_display_only_and_changed_blank_nonname_fields_are_omitted() {
    let current = current();
    let mut output = vec![];
    let mut session = PromptSession::script_cr_or_lf(Cursor::new(b"\r\r\r\r\r"), &mut output);
    let fields = match command::prompt(&mut session, &current).unwrap() {
        PromptOutcome::Submitted(fields) => fields,
        _ => panic!("submitted expected"),
    };
    assert!(fields.empty());
    drop(session);
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("line1\\r\\nline2\\t"));
    assert!(!text.contains("line1\r\n"));
    let mut session = PromptSession::script_cr_or_lf(Cursor::new(b" \r \rplanned\r \r \r"), vec![]);
    let fields = match command::prompt(&mut session, &current).unwrap() {
        PromptOutcome::Submitted(fields) => fields,
        _ => panic!("submitted expected"),
    };
    assert_eq!(
        serde_json::to_value(fields.input(None)).unwrap(),
        json!({"name":"","status":"planned"})
    );
    let mut strict = PromptSession::script(Cursor::new(b"\n"), vec![]);
    assert!(
        strict
            .text_with_options(
                "Description:",
                TextOptions {
                    required: false,
                    default: current.description.as_deref()
                }
            )
            .is_err()
    );
}
#[test]
fn cr_lf_native_protocol_retains_text_select_answers_and_eof() {
    let options = vec![PlainOption {
        label: "Paused".to_owned(),
        value: "paused".to_owned(),
        script_token: "paused".to_owned(),
    }];
    for raw in [b"\r\n\r\nnext\r\n".as_slice(), b"\n\nnext\n", b"\r\rnext\r"] {
        let mut session = PromptSession::script_cr_or_lf(Cursor::new(raw), vec![]);
        assert_eq!(
            session.text("Name:", 0, |_| Ok(())).unwrap(),
            PromptOutcome::Submitted("".to_owned())
        );
        assert_eq!(
            session
                .select(&PlainSelect {
                    message: "Status:",
                    options: &options,
                    default_index: 0,
                    default_hint: None
                })
                .unwrap(),
            PromptOutcome::Submitted("paused".to_owned())
        );
        assert_eq!(
            session.text("Next:", 0, |_| Ok(())).unwrap(),
            PromptOutcome::Submitted("next".to_owned())
        );
        assert_eq!(
            session.text("EOF:", 0, |_| Ok(())).unwrap(),
            PromptOutcome::EndOfInput
        );
    }
    let mut session = PromptSession::script_cr_or_lf(Cursor::new(b"partial"), vec![]);
    assert!(session.text("Name:", 0, |_| Ok(())).is_err());
    let mut session = PromptSession::script(Cursor::new(b"\r"), vec![]);
    assert!(session.text("Name:", 0, |_| Ok(())).is_err());
}
#[test]
fn cr_submission_never_peeks_before_printing_next_text_or_select() {
    use std::{
        cell::RefCell,
        io::{self, Read, Write},
        rc::Rc,
    };
    struct Reader {
        bytes: Cursor<Vec<u8>>,
        output: Rc<RefCell<Vec<u8>>>,
        reads: usize,
    }
    impl Read for Reader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.reads == 1 {
                assert!(
                    String::from_utf8_lossy(&self.output.borrow()).contains("? Status:"),
                    "CR must return and print next question before another read"
                );
            }
            if self.reads == 2 {
                assert!(
                    String::from_utf8_lossy(&self.output.borrow()).contains("? Next:"),
                    "Select CR must return before another read"
                );
            }
            self.reads += 1;
            self.bytes.read(&mut buf[..1])
        }
    }
    struct Writer(Rc<RefCell<Vec<u8>>>);
    impl Write for Writer {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let output = Rc::new(RefCell::new(vec![]));
    let reader = Reader {
        bytes: Cursor::new(b"\r\r\r".to_vec()),
        output: output.clone(),
        reads: 0,
    };
    let mut session = PromptSession::script_cr_or_lf(reader, Writer(output));
    session.text("Name:", 0, |_| Ok(())).unwrap();
    let options = [PlainOption {
        label: "Paused".to_owned(),
        value: "paused".to_owned(),
        script_token: "paused".to_owned(),
    }];
    session
        .select(&PlainSelect {
            message: "Status:",
            options: &options,
            default_index: 0,
            default_hint: None,
        })
        .unwrap();
    session.text("Next:", 0, |_| Ok(())).unwrap();
}

#[tokio::test]
async fn ordered_lookups_details_and_owner_selection_preserve_requests() {
    use linear_cli::commands::initiative::view::Reference;
    let (transport,worker)=super::project_write_server::serve(vec![json!({"errors":[{"message":"slug unavailable"}]}).to_string(),json!({"data":{"initiatives":{"nodes":[{"id":"first","name":"First"},{"id":"second","name":"Second"}]}}}).to_string(),json!({"data":{"initiative":json!({"id":"other","slugId":"slug","name":" raw 界 ","description":null,"status":"planned","targetDate":null,"color":null,"icon":null,"owner":null})}}).to_string(),json!({"data":{"users":{"nodes":[{"id":"display","email":"other","displayName":"Owner","name":"First"},{"id":"email","email":"OWNER","displayName":"Different","name":"Second"}]}}}).to_string()]);
    let id = command::resolve(
        &transport,
        &Reference::NameOrSlug("Exact".to_owned()),
        "Exact",
    )
    .await
    .unwrap();
    assert_eq!(id, "first");
    let detail = command::details(&transport, &id, "Exact").await.unwrap();
    assert_eq!(detail.name, " raw 界 ");
    assert_eq!(
        command::owner(&transport, Some("Owner")).await.unwrap(),
        Some("email".to_owned())
    );
    let requests = worker.join().unwrap();
    assert_eq!(requests.len(), 4);
    assert_eq!(
        requests
            .iter()
            .map(|r| r["operationName"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec![
            "GetInitiativeBySlug",
            "GetInitiativeByName",
            "GetInitiativeForUpdate",
            "LookupUser"
        ]
    );
    assert_eq!(requests[2]["variables"], json!({"id":"first"}));
    let (transport, worker) = super::project_write_server::serve(vec![
        json!({"data":{"initiatives":{"nodes":[{"id":"not-uuid"}]}}}).to_string(),
    ]);
    let error = command::resolve(&transport, &Reference::UrlSlug("slug".to_owned()), "URL")
        .await
        .unwrap_err();
    assert!(error.message().contains("non-UUID"));
    assert_eq!(worker.join().unwrap().len(), 1);
}
#[tokio::test]
async fn owner_raw_sdk_error_details_handled_error_and_null_precedence() {
    let body = json!({"errors":[{"message":"owner failed"}]}).to_string();
    let (transport, worker) = super::project_write_server::serve(vec![body.clone()]);
    let error = command::owner(&transport, Some("@me")).await.unwrap_err();
    assert!(error.message().starts_with("owner failed:"));
    assert!(error.message().contains("GetViewerId"));
    assert!(!error.message().contains("Failed to fetch"));
    assert_eq!(worker.join().unwrap().len(), 1);
    let (transport, worker) = super::project_write_server::serve(vec![body]);
    let error = command::details(&transport, "id", "original")
        .await
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "Failed to fetch initiative details: owner failed"
    );
    assert_eq!(worker.join().unwrap().len(), 1);
    let (transport, worker) =
        super::project_write_server::serve(vec![json!({"data":{"initiative":null}}).to_string()]);
    assert_eq!(
        command::details(&transport, "id", "original")
            .await
            .unwrap_err()
            .message(),
        "Initiative not found: original"
    );
    assert_eq!(worker.join().unwrap().len(), 1);
}
#[tokio::test]
async fn full_decode_before_false_and_only_boolean_true_confirms_mutation() {
    for success in [
        json!(true),
        json!(false),
        json!("true"),
        serde_json::Value::Null,
    ] {
        let (transport, worker) = super::project_write_server::serve(vec![
            json!({"data":{"initiativeUpdate":{"success":success,"initiative":null}}}).to_string(),
        ]);
        let error = command::submit(
            &transport,
            "resolved",
            Fields {
                status: Some("PAUSED".to_owned()),
                ..Fields::default()
            }
            .input(None),
        )
        .await
        .unwrap_err();
        assert!(
            error
                .message()
                .contains("Linear returned an unexpected response")
        );
        assert!(error.message().contains(if success == json!(true) {
            "update confirmed"
        } else {
            "update outcome unknown"
        }));
        let requests = worker.join().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0]["variables"],
            json!({"id":"resolved","input":{"status":"paused"}})
        );
    }
    for success in [true, false] {
        let (transport,worker)=super::project_write_server::serve(vec![json!({"data":{"initiativeUpdate":{"success":success,"initiative":{"id":"other","slugId":"slug","name":"Returned 界","url":""}}}}).to_string()]);
        let result = command::submit(&transport, "resolved", Fields::default().input(None)).await;
        if success {
            assert_eq!(
                result.unwrap(),
                "✓ Updated initiative: Returned 界\n".as_bytes()
            );
        } else {
            assert_eq!(
                result.unwrap_err().to_string(),
                "Failed to update initiative: Failed to update initiative"
            );
        }
        assert_eq!(worker.join().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn shared_owner_list_and_create_keep_selection_and_friendly_error_policy() {
    use linear_cli::commands::{
        initiative::create as initiative_create, initiative::list as initiative_list,
    };
    let reply = json!({"data":{"users":{"nodes":[
        {"id":"first","email":"other","displayName":"Owner","name":"First"},
        {"id":"winner","email":"OWNER","displayName":"Different","name":"Later"}
    ]}}})
    .to_string();
    let (transport, worker) = super::project_write_server::serve(vec![reply.clone(), reply]);
    assert_eq!(
        initiative_list::resolve_owner(&transport, "Owner")
            .await
            .unwrap()
            .inner(),
        "winner"
    );
    assert_eq!(
        initiative_create::resolve_owner(&transport, Some("Owner"))
            .await
            .unwrap(),
        Some("winner".to_owned())
    );
    let requests = worker.join().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(
        requests
            .iter()
            .all(|r| r["operationName"] == "LookupUser"
                && r["variables"] == json!({"input":"Owner"}))
    );
    let body=json!({"errors":[{"message":"raw owner","extensions":{"userPresentableMessage":"Friendly owner"}}]}).to_string();
    let (transport, worker) = super::project_write_server::serve(vec![body.clone(), body]);
    assert_eq!(
        initiative_list::resolve_owner(&transport, "owner")
            .await
            .unwrap_err()
            .message(),
        "Friendly owner"
    );
    assert!(
        command::owner(&transport, Some("owner"))
            .await
            .unwrap_err()
            .message()
            .starts_with("raw owner:")
    );
    assert_eq!(worker.join().unwrap().len(), 2);
    let (transport,worker)=super::project_write_server::serve(vec![json!({"data":{"users":{"nodes":[{"id":"first","email":null,"displayName":"Owner","name":"First"}]}}}).to_string()]);
    let error = command::owner(&transport, Some("Owner")).await.unwrap_err();
    assert!(
        error
            .message()
            .contains("Linear returned an unexpected response")
    );
    assert_eq!(worker.join().unwrap().len(), 1);
}
