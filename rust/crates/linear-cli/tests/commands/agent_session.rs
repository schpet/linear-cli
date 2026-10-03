use chrono::{TimeZone, Utc};
use linear_cli::cli::issue::{IssueAgentSessionCommand, IssueCommand};
use linear_cli::cli::{AgentSessionStatus, RootCommand};
use linear_cli::commands::issue::agent_session;
use linear_cli::graphql::envelope::parse_response;
use linear_cli::graphql::operations::agent_session::{
    AgentActivityContent, GetAgentSessionDetails, GetIssueAgentSessions,
};
use serde_json::{Value, json};

fn session_wire() -> Value {
    serde_json::from_str(include_str!("fixtures/agent-session.json")).unwrap()
}
fn comments_wire() -> Value {
    serde_json::from_str(include_str!("fixtures/agent-session-comments.json")).unwrap()
}
fn session() -> GetAgentSessionDetails {
    parse_response(include_bytes!("fixtures/agent-session.json")).unwrap()
}
fn comments() -> GetIssueAgentSessions {
    parse_response(include_bytes!("fixtures/agent-session-comments.json")).unwrap()
}

#[test]
fn public_cli_preserves_alias_json_and_six_native_status_spellings() {
    for (word, expected) in [
        ("pending", AgentSessionStatus::Pending),
        ("active", AgentSessionStatus::Active),
        ("complete", AgentSessionStatus::Complete),
        ("awaitingInput", AgentSessionStatus::AwaitingInput),
        ("error", AgentSessionStatus::Error),
        ("stale", AgentSessionStatus::Stale),
    ] {
        let words = [
            "issue",
            "agent-session",
            "list",
            "ENG-1",
            "-j",
            "--status",
            word,
        ]
        .map(std::ffi::OsString::from);
        let parsed = crate::parse(&words).unwrap();
        let RootCommand::Issue(issue) = parsed.command else {
            panic!("issue")
        };
        let IssueCommand::AgentSession(group) = issue.command else {
            panic!("session")
        };
        let IssueAgentSessionCommand::List(list) = group.command else {
            panic!("list")
        };
        assert_eq!(list.status, Some(expected));
        assert!(list.json);
        assert_eq!(list.issue_id.as_deref(), Some("ENG-1"));
    }
    for invalid in ["ACTIVE", "awaiting_input", "awaitinginput", "unknown", ""] {
        assert!(
            crate::parse(
                &["issue", "agent-session", "list", "--status", invalid]
                    .map(std::ffi::OsString::from)
            )
            .is_err()
        );
    }
    let parsed = crate::parse(
        &["issue", "agent-session", "v", "session-id", "-j"].map(std::ffi::OsString::from),
    )
    .unwrap();
    let RootCommand::Issue(issue) = parsed.command else {
        panic!("issue")
    };
    let IssueCommand::AgentSession(group) = issue.command else {
        panic!("session")
    };
    let IssueAgentSessionCommand::View(view) = group.command else {
        panic!("view")
    };
    assert_eq!(view.session_id, "session-id");
    assert!(view.json);
}

#[test]
fn view_json_keeps_nulls_fragment_fields_and_hides_typename() {
    let mut value = session().agent_session;
    value.session_type = None;
    value.summary = None;
    let output = serde_json::to_value(&value).unwrap();
    assert_eq!(output["type"], Value::Null);
    assert_eq!(output["summary"], Value::Null);
    assert!(!output.to_string().contains("__typename"));
    let nodes = output["activities"].as_array().unwrap();
    assert_eq!(nodes.len(), 6);
    for (i, kind) in [
        "thought",
        "action",
        "response",
        "prompt",
        "error",
        "elicitation",
    ]
    .iter()
    .enumerate()
    {
        assert_eq!(nodes[i]["content"]["type"], *kind);
    }
    assert_eq!(nodes[1]["content"]["result"], "JSON only");
    assert_eq!(nodes[1]["content"]["parameter"], "");
}

#[test]
fn markdown_renders_all_variants_order_and_source_empty_detail_rules() {
    let now = Utc.with_ymd_and_hms(2026, 9, 30, 12, 0, 0).unwrap();
    let value = session().agent_session;
    let text = agent_session::markdown(&value, now, &Utc).unwrap();
    let mut previous = 0;
    for kind in [
        "thought",
        "action",
        "response",
        "prompt",
        "error",
        "elicitation",
    ] {
        let position = text.find(&format!("- **{kind}**")).unwrap();
        assert!(position > previous);
        previous = position;
    }
    assert!(text.contains("- **action** (2020-01-02) - run: "));
    assert!(text.contains("- **thought** (2020-01-02) - line one line two"));
    assert!(!text.contains("JSON only"));
    assert!(text.contains("**Dismissed by:** Dismiss User"));
    assert!(text.contains("**Creator:** Creator"));
    let mut wire = session_wire();
    wire["data"]["agentSession"]["type"] = Value::Null;
    for field in [
        "startedAt",
        "endedAt",
        "dismissedAt",
        "summary",
        "externalLink",
    ] {
        wire["data"]["agentSession"][field] = json!("");
    }
    for index in [0, 2, 3, 4, 5] {
        wire["data"]["agentSession"]["activities"]["nodes"][index]["content"]["body"] = json!("");
    }
    wire["data"]["agentSession"]["activities"]["nodes"][1]["content"]["action"] = json!("");
    let value: GetAgentSessionDetails =
        parse_response(&serde_json::to_vec(&wire).unwrap()).unwrap();
    let text = agent_session::markdown(&value.agent_session, now, &Utc).unwrap();
    assert!(text.contains("**Type:** null"));
    for word in [
        "**Started:**",
        "**Ended:**",
        "**Dismissed:",
        "**Dismissed by:**",
        "## Summary",
        "**External Link:**",
        " - run:",
        " - line one",
    ] {
        assert!(!text.contains(word), "{word}");
    }
}

#[test]
fn unknown_union_never_produces_json_or_partial_markdown() {
    let mut value = session().agent_session;
    value.activities.nodes[5].content =
        AgentActivityContent::Unsupported("FutureContent".to_owned());
    assert!(
        agent_session::ensure_supported(&value)
            .unwrap_err()
            .to_string()
            .contains("FutureContent")
    );
    assert!(agent_session::markdown(&value, Utc::now(), &Utc).is_err());
}

#[test]
fn sessions_keep_comment_order_and_duplicates_and_skip_comments_without_one() {
    let original = comments().issue.comments.nodes;
    assert_eq!(original.len(), 8);
    let all = agent_session::sessions(original.clone(), None);
    assert_eq!(
        all.len(),
        original
            .iter()
            .filter(|comment| comment.agent_session.is_some())
            .count()
    );
    for (status, word) in [
        (AgentSessionStatus::Pending, "pending"),
        (AgentSessionStatus::Active, "active"),
        (AgentSessionStatus::Complete, "complete"),
        (AgentSessionStatus::AwaitingInput, "awaitingInput"),
        (AgentSessionStatus::Error, "error"),
        (AgentSessionStatus::Stale, "stale"),
    ] {
        let mut duplicated = original.clone();
        duplicated.push(original[1].clone());
        let filtered = agent_session::sessions(duplicated, Some(status));
        assert_eq!(filtered.len(), if word == "pending" { 2 } else { 1 });
        assert_eq!(agent_session::status_name(filtered[0].status), word);
    }
}

#[test]
fn required_shapes_status_types_and_actions_decode_strictly() {
    let original = session_wire();
    for field in [
        "id",
        "status",
        "createdAt",
        "updatedAt",
        "appUser",
        "activities",
    ] {
        let mut wire = original.clone();
        wire["data"]["agentSession"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(
            parse_response::<GetAgentSessionDetails>(&serde_json::to_vec(&wire).unwrap()).is_err(),
            "{field}"
        );
    }
    for value in [json!(null), json!("unknown"), json!(3)] {
        let mut wire = original.clone();
        wire["data"]["agentSession"]["status"] = value;
        assert!(
            parse_response::<GetAgentSessionDetails>(&serde_json::to_vec(&wire).unwrap()).is_err()
        );
    }
    for field in ["action", "parameter"] {
        let mut wire = original.clone();
        wire["data"]["agentSession"]["activities"]["nodes"][1]["content"][field] = Value::Null;
        assert!(
            parse_response::<GetAgentSessionDetails>(&serde_json::to_vec(&wire).unwrap()).is_err()
        );
    }
    for data in [
        json!({"agentSession":null}),
        json!({}),
        json!({"agentSession":{"id":"a"}}),
    ] {
        assert!(
            parse_response::<GetAgentSessionDetails>(
                &serde_json::to_vec(&json!({"data":data})).unwrap()
            )
            .is_err()
        );
    }
    let original = comments_wire();
    for data in [
        json!({"issue":null}),
        json!({}),
        json!({"issue":{"comments":null}}),
    ] {
        assert!(
            parse_response::<GetIssueAgentSessions>(
                &serde_json::to_vec(&json!({"data":data})).unwrap()
            )
            .is_err()
        );
    }
    let mut wire = original;
    wire["data"]["issue"]["comments"]["nodes"][0] = Value::Null;
    assert!(parse_response::<GetIssueAgentSessions>(&serde_json::to_vec(&wire).unwrap()).is_err());
}
