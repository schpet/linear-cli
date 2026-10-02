use std::io::Cursor;

use linear_cli::commands::label::create::{self as label_create, Options};
use linear_cli::graphql::envelope::parse_response;
use linear_cli::graphql::operations::label_create::CreateIssueLabel;
use linear_cli::graphql::operations::team_resolver::GetAllTeams;
use linear_cli::platform::prompt::{PromptOutcome, PromptSession};
use linear_cli::refs::{ResolvedTeam, fetch_all_teams};
use serde_json::{Value, json};

fn options(name: Option<&str>, color: Option<&str>) -> Options {
    Options {
        name: name.map(str::to_owned),
        color: color.map(str::to_owned),
        ..Options::default()
    }
}

#[test]
fn typed_request_matches_frozen_source_document_and_variables() {
    for (id, supplied, team) in [
        (
            "c017-minimal-workspace",
            options(Some("Label 1701"), None),
            None,
        ),
        (
            "c017-team-description",
            Options {
                name: Some("Label 1702".to_owned()),
                color: Some("#ABCdef".to_owned()),
                description: Some("**Client description**".to_owned()),
                team: Some("eng".to_owned()),
                interactive: false,
            },
            Some("00000000-0000-4000-9000-000000001701".to_owned()),
        ),
    ] {
        let path = format!(
            "{}/../../parity/runner/c017-frozen-cases/{id}.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let case: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let expected = case["graphql"]["groups"][0]["steps"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["operation"]
            .clone();
        let wire = serde_json::to_value(label_create::request(&supplied, team).unwrap()).unwrap();
        let compact = |s: &str| {
            s.chars()
                .filter(|ch| !ch.is_whitespace() && *ch != ',')
                .collect::<String>()
        };
        assert_eq!(
            compact(wire["query"].as_str().unwrap()),
            compact(expected["document"].as_str().unwrap())
        );
        assert_eq!(wire["variables"], expected["variables"]);
        assert_eq!(wire["operationName"], "CreateIssueLabel");
    }
}

#[test]
fn validation_defaults_omissions_and_explicit_colors() {
    let mut supplied = options(Some("A"), None);
    supplied.description = Some(String::new());
    let wire = serde_json::to_value(label_create::request(&supplied, None).unwrap()).unwrap();
    assert_eq!(
        wire["variables"]["input"],
        json!({"name":"A","color":"#5E6AD2"})
    );
    for color in ["red", " #abcdef", "#abcde", "#abcdefg", "#12345é"] {
        assert_eq!(
            label_create::validate(&options(Some("A"), Some(color)))
                .unwrap_err()
                .message(),
            "Color must be a valid hex code (e.g., #EB5757)"
        );
    }
    for color in ["#aBcDeF", "#000000", "#FFFFFF"] {
        assert!(label_create::validate(&options(Some("A"), Some(color))).is_ok());
    }
    let error = label_create::validate(&options(None, Some("red"))).unwrap_err();
    assert_eq!(error.message(), "Label name is required");
    assert!(error.hint().unwrap().contains("--name"));
}

#[test]
fn prompt_mode_uses_name_presence_or_interactive_and_stdout() {
    for name in [None, Some("A")] {
        for interactive in [false, true] {
            for tty in [false, true] {
                let mut supplied = options(name, None);
                supplied.interactive = interactive;
                assert_eq!(
                    label_create::should_prompt(&supplied, tty),
                    tty && (name.is_none() || interactive)
                );
            }
        }
    }
}

fn team(key: &str, name: &str) -> ResolvedTeam {
    ResolvedTeam {
        id: key.to_owned(),
        key: key.to_owned(),
        name: name.to_owned(),
    }
}

#[test]
fn buffered_script_survives_suspend_and_uses_configured_team_default() {
    let mut supplied = Options::default();
    let mut session = PromptSession::script(Cursor::new(b"Label\n\n  \n\n"), Vec::new());
    assert_eq!(
        label_create::prompt_fields(&mut supplied, &mut session).unwrap(),
        PromptOutcome::Submitted(())
    );
    assert_eq!(supplied.name.as_deref(), Some("Label"));
    assert_eq!(supplied.color.as_deref(), Some("#5E6AD2"));
    assert_eq!(supplied.description, None);
    session.suspend().unwrap();
    assert!(session.text("Forbidden", 0, |_| Ok(())).is_err());
    session.resume().unwrap();
    assert_eq!(
        label_create::prompt_team(
            &mut supplied,
            &mut session,
            &[team("ENG", "Équipe")],
            Some("ENG")
        )
        .unwrap(),
        PromptOutcome::Submitted(())
    );
    assert_eq!(supplied.team.as_deref(), Some("ENG"));
    let output = String::from_utf8(session.into_output().unwrap()).unwrap();
    assert!(output.contains("? Color: (Indigo (#5E6AD2))"));
    assert!(output.contains("? Team: (Équipe (ENG))\n? Team: › Équipe (ENG)"));
}

#[test]
fn skip_provided_fields_custom_palette_and_workspace_fallback() {
    let mut supplied = options(Some("Already"), None);
    supplied.description = Some("Given".to_owned());
    let mut session =
        PromptSession::script(Cursor::new(b"custom\n#aBc123\nworkspace\n"), Vec::new());
    label_create::prompt_fields(&mut supplied, &mut session).unwrap();
    label_create::prompt_team(
        &mut supplied,
        &mut session,
        &[team("ENG", "Engineering")],
        Some("UNKNOWN"),
    )
    .unwrap();
    assert_eq!(supplied.color.as_deref(), Some("#aBc123"));
    assert_eq!(supplied.team, None);
    let output = String::from_utf8(session.into_output().unwrap()).unwrap();
    assert!(!output.contains("Label name:"));
    assert!(!output.contains("Description (optional):"));
    assert!(output.contains("? Team: (Workspace (shared by all teams))"));
    // The ten palette colors have no skip choice; menu 10 is Gray, 11 custom.
    for (token, expected) in [("1", "#EB5757"), ("10", "#6B6F76")] {
        let mut supplied = options(Some("A"), None);
        supplied.description = Some("d".to_owned());
        let mut session = PromptSession::script(Cursor::new(format!("{token}\n")), Vec::new());
        label_create::prompt_fields(&mut supplied, &mut session).unwrap();
        assert_eq!(supplied.color.as_deref(), Some(expected));
    }
}

#[test]
fn prompt_eof_and_bad_custom_color_stop_before_team() {
    for raw in ["", "A\n", "A\n\n"] {
        let mut supplied = Options::default();
        let mut session = PromptSession::script(Cursor::new(raw), Vec::new());
        assert_eq!(
            label_create::prompt_fields(&mut supplied, &mut session).unwrap(),
            PromptOutcome::EndOfInput
        );
    }
    let mut supplied = options(Some("A"), None);
    let mut session = PromptSession::script(Cursor::new("custom\nred\n"), Vec::new());
    assert_eq!(
        label_create::prompt_fields(&mut supplied, &mut session)
            .unwrap_err()
            .message(),
        "Please enter a valid hex color (e.g., #FF5733)"
    );
}

#[tokio::test]
async fn all_teams_pages_are_minimal_typed_sorted_and_stable() {
    let mut count = 0;
    let teams = fetch_all_teams(|request| {
        let wire = serde_json::to_value(request).unwrap();
        let (nodes, next, cursor) = match count {
            0 => { assert_eq!(wire["variables"], json!({"first":100}));
                (json!([{"id":"z","key":"Z","name":"Zulu"},{"id":"e","key":"E","name":"Équipe"}]),true,Some("next")) },
            1 => { assert_eq!(wire["variables"], json!({"first":100,"after":"next"}));
                (json!([{"id":"a","key":"A","name":"apple"},{"id":"a2","key":"B","name":"APPLE"}]),false,None) },
            _ => panic!("unexpected page"),
        }; count += 1;
        async move { Ok(parse_response::<GetAllTeams>(json!({"data":{"teams":{"nodes":nodes,"pageInfo":{"hasNextPage":next,"endCursor":cursor}}}}).to_string().as_bytes()).unwrap()) }
    }).await.unwrap();
    assert_eq!(count, 2);
    assert_eq!(
        teams.iter().map(|t| t.key.as_str()).collect::<Vec<_>>(),
        vec!["A", "B", "E", "Z"]
    );
}

#[test]
fn render_preserves_scope_description_and_rejects_malformed_payloads() {
    let label = json!({"id":"l","name":"Server","color":"#ABCdef","description":"desc","team":{"key":"ENG","name":"Équipe"}});
    let decode = |success, label| {
        parse_response::<CreateIssueLabel>(
            json!({"data":{"issueLabelCreate":{"success":success,"issueLabel":label}}})
                .to_string()
                .as_bytes(),
        )
        .unwrap()
    };
    assert_eq!(
        label_create::render(&decode(true, label.clone()).issue_label_create).unwrap(),
        "✓ Created label: Server\n  Color: #ABCdef\n  Description: desc\n  Scope: Équipe (ENG)\n"
            .as_bytes()
    );
    assert_eq!(
        label_create::render(&decode(false, label.clone()).issue_label_create)
            .unwrap_err()
            .message(),
        "Failed to create label"
    );
    let mut workspace = label;
    workspace["team"] = Value::Null;
    workspace["description"] = json!("");
    assert_eq!(
        label_create::render(&decode(true, workspace).issue_label_create).unwrap(),
        "✓ Created label: Server\n  Color: #ABCdef\n  Scope: Workspace\n".as_bytes()
    );
    for label in [Value::Null, json!({"id":"l","name":"Server","team":null})] {
        assert!(
            parse_response::<CreateIssueLabel>(
                json!({"data":{"issueLabelCreate":{"success":true,"issueLabel":label}}})
                    .to_string()
                    .as_bytes()
            )
            .is_err()
        );
    }
}
