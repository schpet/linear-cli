//! public update/file/collection/write-barrier vectors.
use linear_cli::{
    commands::{
        project::collections::{
            self as project_collections, FailedWrite, InitiativeChange, InitiativeLink, ResolvedRef,
        },
        project::update,
        project::write as shared,
    },
    graphql::{edit::Edit, operations::project_write::ProjectUpdateInput},
};
use serde_json::json;
fn reference(id: &str, label: &str) -> ResolvedRef {
    ResolvedRef {
        id: id.to_owned(),
        label: label.to_owned(),
    }
}
#[test]
fn local_inputs_preserve_clear_empty_and_validation_phase_order() {
    let input = update::local(&update::Options {
        description: Some("".to_owned()),
        content: Some("".to_owned()),
        clear_lead: true,
        clear_start_date: true,
        clear_target_date: true,
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        serde_json::to_value(input).unwrap(),
        json!({"description":"","content":"","startDate":null,"targetDate":null})
    );
    let error = update::local(&update::Options::default()).unwrap_err();
    assert_eq!(
        error.message(),
        "At least one update option must be provided"
    );
    let error = update::local(&update::Options {
        lead: Some("x".to_owned()),
        clear_lead: true,
        description_file: Some("missing".to_owned()),
        ..Default::default()
    })
    .unwrap_err();
    assert_eq!(
        error.message(),
        "Cannot specify both --lead and --clear-lead"
    );
    let error = update::local(&update::Options {
        labels: Some(vec![" \t".to_owned()]),
        content_file: Some("missing".to_owned()),
        ..Default::default()
    })
    .unwrap_err();
    assert_eq!(error.message(), "Project label cannot be empty");
    assert!(update::replace_conflict("team", true, true, false).is_err());
    assert!(update::replace_conflict("label", true, false, true).is_err());
    assert!(
        update::overlap(
            "initiative",
            &[reference("same", "a")],
            &[reference("same", "b")]
        )
        .is_err()
    );
    assert!(!update::has_fields(&ProjectUpdateInput::default()));
    assert!(update::has_fields(&ProjectUpdateInput {
        label_ids: Some(vec![]),
        ..Default::default()
    }));
    assert!(update::has_fields(&ProjectUpdateInput {
        lead_id: Edit::Clear,
        ..Default::default()
    }));
}
#[test]
fn decoded_files_preserve_source_success_and_distinct_error_shapes() {
    static SERIAL: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "project-file-test-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).unwrap();
    let file = root.join("body");
    std::fs::write(&file, b"\xef\xbb\xbf#raw\r\n").unwrap();
    let path = file.to_str().unwrap();
    let expected = Some("#raw\r\n".to_owned());
    assert_eq!(shared::content(None, Some(path)).unwrap(), expected);
    assert_eq!(shared::description(None, Some(path)).unwrap(), expected);
    std::fs::write(&file, []).unwrap();
    assert_eq!(
        shared::content(None, Some(path)).unwrap(),
        Some(String::new())
    );
    let missing = root.join("missing");
    let missing = missing.to_str().unwrap();
    let content = shared::content(None, Some(missing)).unwrap_err();
    assert_eq!(
        content.message(),
        format!("Failed to read content file: {missing}")
    );
    assert!(content.hint().unwrap().starts_with("Error: "));
    assert_eq!(
        shared::description(None, Some(missing))
            .unwrap_err()
            .message(),
        format!("File not found: {missing}")
    );
    let directory = root.to_str().unwrap();
    let content = shared::content(None, Some(directory)).unwrap_err();
    assert_eq!(
        content.message(),
        format!("Failed to read content file: {directory}")
    );
    assert!(content.hint().unwrap().starts_with("Error: "));
    let description = shared::description(None, Some(directory)).unwrap_err();
    assert!(
        description
            .message()
            .starts_with("Failed to read description file: ")
    );
    assert!(description.hint().is_none());
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn collection_and_join_plan_preserve_order_duplicate_rows_and_uuid_recovery() {
    let current = vec!["old".to_owned(), "keep".to_owned(), "keep".to_owned()];
    let edited = project_collections::apply_collection_edit(
        &current,
        &[reference("new", "New"), reference("keep", "Alias")],
        &[reference("old", "Old")],
    )
    .unwrap();
    assert_eq!(edited, ["keep", "keep", "new"]);
    assert!(
        project_collections::apply_collection_edit(&current, &[], &[reference("missing", "M")])
            .is_err()
    );
    let links = vec![
        InitiativeLink {
            id: "join-a".to_owned(),
            initiative_id: "old-id".to_owned(),
            initiative_name: "Old A".to_owned(),
        },
        InitiativeLink {
            id: "join-b".to_owned(),
            initiative_id: "old-id".to_owned(),
            initiative_name: "Old B".to_owned(),
        },
    ];
    let desired = vec!["new-id".to_owned()];
    let changes = project_collections::plan_initiative_changes(
        &links,
        &desired,
        &[
            reference("new-id", "Replacement raw"),
            reference("new-id", "Added raw"),
        ],
    );
    assert_eq!(changes.len(), 3);
    assert_eq!(changes[0].description(), "removed \"Old A\"");
    assert_eq!(changes[1].description(), "removed \"Old B\"");
    assert_eq!(changes[2].description(), "added \"Replacement raw\"");
    assert_eq!(changes[0].recovery_flag(), "--remove-initiative old-id");
    let rejected =
        project_collections::partial_diagnostic(&changes, 1, FailedWrite::Rejected, true).unwrap();
    assert!(
        rejected
            .message
            .contains("Applied: updated the project's other fields, removed \"Old A\".")
    );
    assert!(
        rejected
            .message
            .contains("Not applied: removed \"Old B\", added \"Replacement raw\".")
    );
    assert!(!rejected.message.contains("Unknown"));
    let unknown =
        project_collections::partial_diagnostic(&changes, 1, FailedWrite::Unknown, false).unwrap();
    assert!(
        unknown
            .message
            .contains("Unknown (the request failed before Linear answered): removed \"Old B\".")
    );
    assert!(
        unknown
            .message
            .contains("Not applied: added \"Replacement raw\".")
    );
    assert!(
        unknown
            .suggestion
            .starts_with("Check the project's initiatives")
    );
    assert!(!unknown.suggestion.contains("join-b"));
}
#[tokio::test]
async fn null_field_payload_still_applies_joins_and_never_uses_link_snapshot() {
    let (transport, server) = super::project_write_server::serve(vec![
        r#"{"data":{"projectUpdate":{"success":true,"project":null}}}"#.to_owned(),
        r#"{"data":{"initiativeToProjectDelete":{"success":true}}}"#.to_owned(),
        r#"{"data":{"initiativeToProjectCreate":{"success":true}}}"#.to_owned(),
    ]);
    let plan = update::Plan {
        project_id: "project".to_owned(),
        input: ProjectUpdateInput {
            name: Edit::Set("new".to_owned()),
            ..Default::default()
        },
        changes: vec![
            InitiativeChange::Remove {
                link_id: "join-old".to_owned(),
                initiative_id: "old".to_owned(),
                label: "Old".to_owned(),
            },
            InitiativeChange::Add {
                initiative_id: "new".to_owned(),
                label: "New".to_owned(),
            },
        ],
        initiative_only_display: Some(update::DisplayProject {
            name: "Last link snapshot".to_owned(),
            url: "url".to_owned(),
        }),
    };
    let result = update::submit(&transport, plan).await.unwrap();
    assert!(result.is_none());
    assert_eq!(update::output(result.as_ref()), b"");
    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[0]["operationName"], "UpdateProject");
    assert_eq!(
        requests[1]["operationName"],
        "RemoveProjectFromInitiativeForUpdate"
    );
    assert_eq!(requests[1]["variables"]["id"], "join-old");
    assert_eq!(
        requests[2]["operationName"],
        "AddProjectToInitiativeForUpdate"
    );
}
#[tokio::test]
async fn initiative_only_noop_has_no_empty_project_update_and_last_page_output() {
    let (transport, _server) = super::project_write_server::serve(vec![]);
    let plan = update::Plan {
        project_id: "p".to_owned(),
        input: ProjectUpdateInput::default(),
        changes: vec![],
        initiative_only_display: Some(update::DisplayProject {
            name: "Last".to_owned(),
            url: "url".to_owned(),
        }),
    };
    let result = update::submit(&transport, plan).await.unwrap();
    assert_eq!(
        update::output(result.as_ref()),
        "✓ Updated project: Last\nurl\n".as_bytes()
    );
}
#[tokio::test]
async fn field_rejection_aborts_all_join_writes() {
    let (transport, server) = super::project_write_server::serve(vec![
        r#"{"data":{"projectUpdate":{"success":false,"project":null}}}"#.to_owned(),
    ]);
    let plan = update::Plan {
        project_id: "p".to_owned(),
        input: ProjectUpdateInput {
            label_ids: Some(vec![]),
            ..Default::default()
        },
        changes: vec![InitiativeChange::Add {
            initiative_id: "i".to_owned(),
            label: "I".to_owned(),
        }],
        initiative_only_display: None,
    };
    assert_eq!(
        update::submit(&transport, plan)
            .await
            .unwrap_err()
            .message(),
        "Failed to update project"
    );
    assert_eq!(server.join().unwrap().len(), 1);
}
