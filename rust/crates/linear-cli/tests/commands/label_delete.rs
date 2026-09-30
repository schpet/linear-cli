use std::io::Cursor;

use linear_cli::commands::label_delete::{self, Lookup};
use linear_cli::error::AppErrorKind;
use linear_cli::graphql::envelope::{ResponseError, parse_response};
use linear_cli::graphql::operations::label_delete::{
    DeleteIssueLabel, GetLabelById, GetLabelByName, Label, LabelTeam,
};
use linear_cli::platform::prompt::{PromptKey, PromptOutcome, PromptSession};
use serde_json::{Value, json};

fn label(id: &str, team: Option<&str>) -> Label {
    Label {
        id: cynic::Id::new(id),
        name: "Bug".into(),
        color: "#123456".into(),
        team: team.map(|key| LabelTeam {
            key: key.into(),
            name: "Team".into(),
        }),
    }
}

#[test]
fn requests_have_exact_source_selections_and_only_used_variables() {
    let id = serde_json::to_value(label_delete::id_request("original-id")).unwrap();
    let name = serde_json::to_value(label_delete::name_request(" Bug ")).unwrap();
    let delete = serde_json::to_value(label_delete::delete_request("chosen-id")).unwrap();
    let compact = |s: &str| {
        s.chars()
            .filter(|ch| !ch.is_whitespace() && *ch != ',')
            .collect::<String>()
    };
    for (wire, expected, variables) in [
        (
            id,
            "query GetLabelById($id: String!) { issueLabel(id: $id) { id name color team { key name } } }",
            json!({"id":"original-id"}),
        ),
        (
            name,
            "query GetLabelByName($name: String!) { issueLabels(filter: { name: { eqIgnoreCase: $name } }) { nodes { id name color team { key name } } } }",
            json!({"name":" Bug "}),
        ),
        (
            delete,
            "mutation DeleteIssueLabel($id: String!) { issueLabelDelete(id: $id) { success } }",
            json!({"id":"chosen-id"}),
        ),
    ] {
        assert_eq!(compact(wire["query"].as_str().unwrap()), compact(expected));
        assert_eq!(wire["variables"], variables);
        assert_eq!(wire.as_object().unwrap().len(), 3);
    }
}

#[test]
fn scope_uses_first_team_match_then_workspace_but_direct_id_bypasses_it() {
    let labels = || {
        vec![
            label("workspace", None),
            label("first", Some("eNg")),
            label("second", Some("ENG")),
            label("other", Some("OPS")),
        ]
    };
    assert_eq!(
        label_delete::scoped(Lookup::Named(labels()), Some("ENG")),
        vec![label("first", Some("eNg"))]
    );
    assert_eq!(
        label_delete::scoped(Lookup::Named(labels()), Some("missing")),
        vec![label("workspace", None)]
    );
    assert_eq!(
        label_delete::scoped(
            Lookup::Named(vec![label("other", Some("OPS"))]),
            Some("ENG")
        ),
        vec![]
    );
    assert_eq!(
        label_delete::scoped(Lookup::Named(labels()), None),
        labels()
    );
    assert_eq!(
        label_delete::scoped(Lookup::Direct(label("direct", Some("OPS"))), Some("ENG")),
        vec![label("direct", Some("OPS"))]
    );
    assert_eq!(
        label_delete::missing("x", Some("ENG"))
            .suggestion
            .as_deref(),
        Some("Searched in team ENG and workspace.")
    );
}

#[test]
fn typed_boundaries_preserve_empty_strings_and_nullable_team_but_reject_corruption() {
    let valid =
        br##"{"data":{"issueLabels":{"nodes":[{"id":"","name":"","color":"","team":null}]}}}"##;
    let result: GetLabelByName = parse_response(valid).unwrap();
    assert_eq!(result.issue_labels.nodes[0].id.inner(), "");
    assert_eq!(
        label_delete::display(&result.issue_labels.nodes[0]),
        " (Workspace)"
    );
    assert_eq!(
        label_delete::display(&label("id", Some(""))),
        "Bug (Workspace)"
    );
    for value in [
        json!({"issueLabel": null}),
        json!({"issueLabel": {"id":"x","name":"x","color":12,"team":null}}),
    ] {
        let result =
            parse_response::<GetLabelById>(&serde_json::to_vec(&json!({"data":value})).unwrap());
        assert!(matches!(result, Err(ResponseError::UnexpectedShape(_))));
    }
    for value in [
        json!({"issueLabels":null}),
        json!({"issueLabels":{"nodes":[null]}}),
    ] {
        assert!(matches!(
            parse_response::<GetLabelByName>(&serde_json::to_vec(&json!({"data":value})).unwrap()),
            Err(ResponseError::UnexpectedShape(_))
        ));
    }
    assert!(matches!(
        parse_response::<DeleteIssueLabel>(br#"{"data":{"issueLabelDelete":{"success":null}}}"#),
        Err(ResponseError::UnexpectedShape(_))
    ));
}

#[test]
fn duplicate_picker_uses_source_order_options_and_first_id_find() {
    let labels = vec![label("one", None), label("two", Some("ENG"))];
    let mut keys = vec![PromptKey::Down, PromptKey::Enter].into_iter();
    let mut output = Vec::new();
    let mut session = PromptSession::<Cursor<Vec<u8>>, _>::keys(&mut output, 80, 24, move || {
        Ok(keys.next().unwrap_or(PromptKey::EndOfInput))
    })
    .unwrap();
    assert_eq!(
        label_delete::choose(&mut session, "Bug", &labels).unwrap(),
        PromptOutcome::Submitted(label("two", Some("ENG")))
    );
    drop(session);
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("Bug (Workspace) - #123456"));
    assert!(output.contains("Bug (ENG) - #123456"));
    let mut script = PromptSession::script(Cursor::new(b"two\n"), Vec::new());
    assert_eq!(
        label_delete::choose(&mut script, "Bug", &labels).unwrap(),
        PromptOutcome::Submitted(label("two", Some("ENG")))
    );
    for (key, expected) in [
        (PromptKey::Interrupt, PromptOutcome::Interrupted),
        (PromptKey::EndOfInput, PromptOutcome::EndOfInput),
    ] {
        let mut session =
            PromptSession::<Cursor<Vec<u8>>, _>::keys(Vec::new(), 80, 24, move || Ok(key)).unwrap();
        assert_eq!(
            label_delete::choose(&mut session, "Bug", &labels).unwrap(),
            expected
        );
    }
}

#[tokio::test]
async fn delete_sends_selected_id_once_and_has_exact_success_or_rejection() {
    for (body, succeeds) in [
        (r#"{"data":{"issueLabelDelete":{"success":true}}}"#, true),
        (r#"{"data":{"issueLabelDelete":{"success":false}}}"#, false),
        (r#"{"errors":[{"message":"delete denied"}]}"#, false),
    ] {
        let (transport, server) = super::delete_server::serve(body);
        let result = label_delete::submit(&transport, &label("chosen-id", None)).await;
        let wire: Value = server.join().unwrap();
        assert_eq!(wire["variables"], json!({"id":"chosen-id"}));
        if succeeds {
            assert_eq!(
                result.unwrap(),
                "✓ Deleted label: Bug (Workspace)\n".as_bytes()
            );
        } else {
            assert_eq!(result.unwrap_err().kind, AppErrorKind::GraphQl);
        }
    }
}

#[tokio::test]
async fn ordinary_name_exchange_failure_is_not_found_but_malformed_shape_stops() {
    for (body, malformed) in [
        (r#"{"errors":[{"message":"unauthorized"}]}"#, false),
        (r#"{"data":{"issueLabels":{"nodes":[null]}}}"#, true),
    ] {
        let (transport, server) = super::delete_server::serve(body);
        let result = label_delete::lookup(&transport, "Bug").await;
        server.join().unwrap();
        if malformed {
            assert_eq!(result.err().unwrap().kind, AppErrorKind::Invariant);
        } else {
            assert!(matches!(result, Ok(Lookup::Named(labels)) if labels.is_empty()));
        }
    }
}
