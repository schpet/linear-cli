use chrono::{TimeZone, Utc};
use linear_cli::cli::issue::{IssueAgentSessionCommand, IssueCommand};
use linear_cli::cli::{self, AgentSessionStatus, RootCommand};
use linear_cli::commands::agent_session;
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
        let parsed = cli::parse(&words).unwrap();
        let Some(RootCommand::Issue(issue)) = parsed.command else {
            panic!("issue")
        };
        let Some(IssueCommand::AgentSession(group)) = issue.command else {
            panic!("session")
        };
        let Some(IssueAgentSessionCommand::List(list)) = group.command else {
            panic!("list")
        };
        assert_eq!(list.status, Some(expected));
        assert!(list.json);
        assert_eq!(list.issue_id.as_deref(), Some("ENG-1"));
    }
    for invalid in ["ACTIVE", "awaiting_input", "awaitinginput", "unknown", ""] {
        assert!(
            cli::parse(
                &["issue", "agent-session", "list", "--status", invalid]
                    .map(std::ffi::OsString::from)
            )
            .is_err()
        );
    }
    let parsed = cli::parse(
        &["issue", "agent-session", "v", "session-id", "-j"].map(std::ffi::OsString::from),
    )
    .unwrap();
    let Some(RootCommand::Issue(issue)) = parsed.command else {
        panic!("issue")
    };
    let Some(IssueCommand::AgentSession(group)) = issue.command else {
        panic!("session")
    };
    let Some(IssueAgentSessionCommand::View(view)) = group.command else {
        panic!("view")
    };
    assert_eq!(view.session_id, "session-id");
    assert!(view.json);
}

#[test]
fn public_requests_are_one_direct_read_with_original_limits_and_union_order() {
    let view = serde_json::to_value(agent_session::view_request("opaque-id")).unwrap();
    assert_eq!(view["variables"], json!({"id":"opaque-id"}));
    assert_eq!(view["operationName"], "GetAgentSessionDetails");
    let query = view["query"].as_str().unwrap();
    assert!(query.contains("activities(first: 20)"));
    let mut previous = 0;
    for word in [
        "__typename",
        "AgentActivityThoughtContent",
        "AgentActivityActionContent",
        "AgentActivityResponseContent",
        "AgentActivityPromptContent",
        "AgentActivityErrorContent",
        "AgentActivityElicitationContent",
    ] {
        let position = query.find(word).unwrap();
        assert!(position > previous);
        previous = position;
    }
    let list = serde_json::to_value(agent_session::list_request("ENG-7")).unwrap();
    assert_eq!(list["variables"], json!({"issueId":"ENG-7"}));
    assert_eq!(list["operationName"], "GetIssueAgentSessions");
    assert_eq!(
        list["query"],
        "query GetIssueAgentSessions($issueId: String!) {\n  issue(id: $issueId) {\n    comments(first: 100) {\n      nodes {\n        agentSession {\n          id\n          status\n          type\n          createdAt\n          startedAt\n          endedAt\n          summary\n          creator {\n            name\n          }\n          appUser {\n            name\n          }\n        }\n      }\n      pageInfo {\n        hasNextPage\n        endCursor\n      }\n    }\n  }\n}\n"
    );
}

#[test]
fn view_json_keeps_nulls_fragment_fields_and_hides_typename() {
    let mut value = session().agent_session;
    value.session_type = None;
    value.summary = None;
    let bytes = agent_session::json(&value).unwrap();
    assert_eq!(bytes.last(), Some(&b'\n'));
    let output: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(output["type"], Value::Null);
    assert_eq!(output["summary"], Value::Null);
    assert!(!String::from_utf8(bytes).unwrap().contains("__typename"));
    let nodes = output["activities"]["nodes"].as_array().unwrap();
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
    assert!(text.contains("- **action** (1/2/2020) - run: "));
    assert!(text.contains("- **thought** (1/2/2020) - line one line two"));
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
            .display_message()
            .contains("FutureContent")
    );
    assert!(agent_session::json(&value).is_err());
    assert!(agent_session::markdown(&value, Utc::now(), &Utc).is_err());
}

#[test]
fn list_filter_keeps_connection_page_info_order_duplicates_and_unfiltered_nulls() {
    let original = comments().issue.comments;
    assert_eq!(original.nodes.len(), 8);
    let plain = agent_session::filter(original.clone(), None);
    assert_eq!(plain, original);
    for (status, word) in [
        (AgentSessionStatus::Pending, "pending"),
        (AgentSessionStatus::Active, "active"),
        (AgentSessionStatus::Complete, "complete"),
        (AgentSessionStatus::AwaitingInput, "awaitingInput"),
        (AgentSessionStatus::Error, "error"),
        (AgentSessionStatus::Stale, "stale"),
    ] {
        let mut duplicated = original.clone();
        duplicated.nodes.push(original.nodes[1].clone());
        let filtered = agent_session::filter(duplicated, Some(status));
        assert_eq!(filtered.page_info, original.page_info);
        assert_eq!(filtered.nodes.len(), if word == "pending" { 2 } else { 1 });
        assert_eq!(
            agent_session::status_name(filtered.nodes[0].agent_session.as_ref().unwrap().status),
            word
        );
        let out: Value = serde_json::from_slice(&agent_session::json(&filtered).unwrap()).unwrap();
        assert!(out["pageInfo"]["hasNextPage"].as_bool().unwrap());
        assert_eq!(out["pageInfo"]["endCursor"], "keep-original");
    }
    let output: Value = serde_json::from_slice(&agent_session::json(&plain).unwrap()).unwrap();
    assert_eq!(output["nodes"][0]["agentSession"], Value::Null);
}

#[test]
fn list_text_width_colors_dates_and_empty_contract() {
    let original = comments().issue.comments;
    let text = String::from_utf8(agent_session::text(&original, 40, false)).unwrap();
    assert!(text.starts_with("STATUS        AGENT   CREATED    SUMMARY\n"));
    assert!(text.contains("代理 🤖"));
    assert!(text.contains("2020-01-02"));
    assert!(text.contains("Long su..."));
    assert!(!text.contains('\x1b'));
    assert!(!text.contains("line two"));
    let color = String::from_utf8(agent_session::text(&original, 120, true)).unwrap();
    for escape in [
        "\x1b[1m\x1b[4m",
        "\x1b[32mactive",
        "\x1b[33mpending",
        "\x1b[90mcomplete",
        "\x1b[90m--",
    ] {
        assert!(color.contains(escape), "{escape}");
    }
    let mut empty = original;
    empty
        .nodes
        .retain(|comment| comment.agent_session.is_none());
    assert_eq!(
        agent_session::text(&empty, 120, true),
        b"No agent sessions found for this issue.\n"
    );
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

#[test]
fn list_date_slice_preserves_javascript_utf16_and_invalid_scalar_text() {
    for (input, expected) in [
        ("bad-date", "bad-date"),
        ("123456789🤖extra", "123456789�"),
        ("12345678🤖extra", "12345678🤖"),
        ("", ""),
    ] {
        let mut value = comments().issue.comments;
        value.nodes.truncate(2);
        let session = value.nodes[1].agent_session.as_mut().unwrap();
        session.created_at.0 = input.to_owned();
        let output = String::from_utf8(agent_session::text(&value, 120, false)).unwrap();
        assert!(
            output.contains(&format!(
                " {} --",
                linear_cli::commands::display::pad(expected, 10)
            )),
            "{output}"
        );
    }
}
