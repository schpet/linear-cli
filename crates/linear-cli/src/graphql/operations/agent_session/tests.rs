use serde_json::{Value, from_value, json, to_string, to_value};

use cynic::QueryBuilder;

use super::{
    AgentActivityContent, AgentActivityType, AgentSessionStatus, AgentSessionType,
    GetAgentSessionDetails, GetAgentSessionDetailsVariables,
};
use crate::graphql::envelope::parse_response;

fn content(value: Value) -> AgentActivityContent {
    from_value(value).expect("union member parses")
}

#[test]
fn every_union_member_deserializes_to_its_variant_and_serializes_without_typename() {
    let cases: [(Value, &str); 6] = [
        (
            json!({"__typename": "AgentActivityThoughtContent", "type": "thought", "body": "hmm"}),
            r#"{"type":"thought","body":"hmm"}"#,
        ),
        (
            json!({"__typename": "AgentActivityActionContent", "type": "action", "action": "read", "parameter": "src/a.ts", "result": null}),
            r#"{"type":"action","action":"read","parameter":"src/a.ts","result":null}"#,
        ),
        (
            json!({"__typename": "AgentActivityResponseContent", "type": "response", "body": "done"}),
            r#"{"type":"response","body":"done"}"#,
        ),
        (
            json!({"__typename": "AgentActivityPromptContent", "type": "prompt", "body": "which?"}),
            r#"{"type":"prompt","body":"which?"}"#,
        ),
        (
            json!({"__typename": "AgentActivityErrorContent", "type": "error", "body": "boom"}),
            r#"{"type":"error","body":"boom"}"#,
        ),
        (
            json!({"__typename": "AgentActivityElicitationContent", "type": "elicitation", "body": "need input"}),
            r#"{"type":"elicitation","body":"need input"}"#,
        ),
    ];
    for (input, expected) in cases {
        let parsed = content(input.clone());
        parsed.ensure_supported().expect("supported");
        assert_eq!(to_string(&parsed).expect("serialize"), expected, "{input}");
    }
    let thought = content(
        json!({"__typename": "AgentActivityThoughtContent", "type": "thought", "body": "x"}),
    );
    match &thought {
        AgentActivityContent::AgentActivityThoughtContent(inner) => {
            assert_eq!(inner.activity_type, AgentActivityType::Thought);
            assert_eq!(inner.body, "x");
        }
        other => panic!("expected thought, got {other:?}"),
    }
    let action = content(
        json!({"__typename": "AgentActivityActionContent", "type": "action", "action": "a", "parameter": "p", "result": "r"}),
    );
    match &action {
        AgentActivityContent::AgentActivityActionContent(inner) => {
            assert_eq!(inner.result.as_deref(), Some("r"));
        }
        other => panic!("expected action, got {other:?}"),
    }
}

#[test]
fn unknown_typename_lands_in_the_fallback_and_is_rejected_at_the_boundary() {
    let parsed =
        content(json!({"__typename": "AgentActivityFutureContent", "type": "future", "body": "?"}));
    assert_eq!(
        parsed,
        AgentActivityContent::Unsupported("AgentActivityFutureContent".to_owned())
    );
    let error = parsed.ensure_supported().expect_err("unsupported");
    assert_eq!(error.typename, "AgentActivityFutureContent");
    assert_eq!(
        error.to_string(),
        "unsupported agent activity content type: AgentActivityFutureContent"
    );
    let serialize_error = to_value(&parsed).expect_err("fallback must not render");
    assert!(
        serialize_error
            .to_string()
            .contains("unsupported AgentActivityContent type: AgentActivityFutureContent")
    );
}

#[test]
fn member_with_wrong_field_shape_is_rejected() {
    let error = from_value::<AgentActivityContent>(
        json!({"__typename": "AgentActivityThoughtContent", "type": "thought", "body": 5}),
    )
    .expect_err("body must be a string");
    assert!(error.to_string().contains("string"), "{error}");
    assert!(
        from_value::<AgentActivityContent>(json!({"type": "thought", "body": "no typename"}))
            .is_err()
    );
}

const SESSION_BODY: &str = r#"{"data":{"agentSession":{"id":"s1","status":"awaitingInput","type":"commentThread","createdAt":"2026-09-01T00:00:00.000Z","updatedAt":"2026-09-02T00:00:00.000Z","startedAt":null,"endedAt":null,"dismissedAt":null,"summary":null,"externalLink":"https://example.invalid/s1","creator":{"name":"Ada"},"appUser":{"name":"Bot"},"dismissedBy":null,"issue":{"identifier":"ENG-7","title":"Fix login","url":"https://linear.app/x/issue/ENG-7"},"activities":{"nodes":[{"id":"a1","createdAt":"2026-09-01T00:00:01.000Z","content":{"__typename":"AgentActivityThoughtContent","type":"thought","body":"thinking"}},{"id":"a2","createdAt":"2026-09-01T00:00:02.000Z","content":{"__typename":"AgentActivityActionContent","type":"action","action":"grep","parameter":"foo","result":null}}],"pageInfo":{"hasNextPage":false,"endCursor":null}}}}}"#;

#[test]
fn full_session_document_parses_and_renders_in_document_order() {
    let data: GetAgentSessionDetails = parse_response(SESSION_BODY.as_bytes()).expect("session");
    let session = &data.agent_session;
    assert_eq!(session.status, AgentSessionStatus::AwaitingInput);
    assert_eq!(session.session_type, Some(AgentSessionType::CommentThread));
    assert_eq!(session.activities.nodes.len(), 2);
    for activity in &session.activities.nodes {
        activity.content.ensure_supported().expect("supported");
    }
    let rendered = serde_json::to_string_pretty(session).expect("pretty");
    assert_eq!(
        rendered,
        r#"{
  "id": "s1",
  "status": "awaitingInput",
  "type": "commentThread",
  "createdAt": "2026-09-01T00:00:00.000Z",
  "updatedAt": "2026-09-02T00:00:00.000Z",
  "startedAt": null,
  "endedAt": null,
  "dismissedAt": null,
  "summary": null,
  "externalLink": "https://example.invalid/s1",
  "creator": {
    "name": "Ada"
  },
  "appUser": {
    "name": "Bot"
  },
  "dismissedBy": null,
  "issue": {
    "identifier": "ENG-7",
    "title": "Fix login",
    "url": "https://linear.app/x/issue/ENG-7"
  },
  "activities": [
    {
      "id": "a1",
      "createdAt": "2026-09-01T00:00:01.000Z",
      "content": {
        "type": "thought",
        "body": "thinking"
      }
    },
    {
      "id": "a2",
      "createdAt": "2026-09-01T00:00:02.000Z",
      "content": {
        "type": "action",
        "action": "grep",
        "parameter": "foo",
        "result": null
      }
    }
  ]
}"#
    );
}

#[test]
fn enums_use_exact_schema_spellings_and_reject_unknown_values() {
    let parsed: AgentActivityType = from_value(json!("thought")).expect("lowercase");
    assert_eq!(parsed, AgentActivityType::Thought);
    assert_eq!(to_value(parsed).expect("value"), Value::from("thought"));
    let error = from_value::<AgentActivityType>(json!("THOUGHT")).expect_err("wrong case");
    assert!(error.to_string().contains("unknown variant"), "{error}");
    assert!(from_value::<AgentActivityType>(json!("Thought")).is_err());
    assert!(from_value::<AgentActivityType>(json!(0)).is_err());

    let status: AgentSessionStatus = from_value(json!("awaitingInput")).expect("camelCase");
    assert_eq!(status, AgentSessionStatus::AwaitingInput);
    assert_eq!(
        to_value(status).expect("value"),
        Value::from("awaitingInput")
    );
    assert!(from_value::<AgentSessionStatus>(json!("AWAITING_INPUT")).is_err());
    assert!(from_value::<AgentSessionStatus>(json!("awaiting_input")).is_err());
}

#[test]
fn session_details_page_through_activities() {
    let operation = GetAgentSessionDetails::build(GetAgentSessionDetailsVariables {
        id: "s1".to_owned(),
        first: 100,
        after: Some("cursor".to_owned()),
    });
    assert_eq!(
        to_value(&operation.variables).expect("variables"),
        json!({"id": "s1", "first": 100, "after": "cursor"})
    );
    assert!(
        operation
            .query
            .contains("activities(first: $first, after: $after)"),
        "{}",
        operation.query
    );
}
