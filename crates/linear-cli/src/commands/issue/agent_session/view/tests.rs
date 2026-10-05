use chrono::{TimeZone, Utc};
use serde_json::{Value, json};

use super::{ensure_supported, markdown};
use crate::graphql::envelope::parse_response;
use crate::graphql::operations::agent_session::{AgentActivityContent, GetAgentSessionDetails};

const SESSION: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/agent_session.json"
));

fn session(wire: &Value) -> GetAgentSessionDetails {
    parse_response(wire.to_string().as_bytes()).expect("session fixture")
}

fn wire() -> Value {
    serde_json::from_str(SESSION).expect("session fixture JSON")
}

#[test]
fn markdown_lists_every_activity_kind_in_order() {
    let now = Utc.with_ymd_and_hms(2026, 9, 30, 12, 0, 0).unwrap();
    let text = markdown(&session(&wire()).agent_session, now, &Utc).expect("markdown");
    let mut previous = 0;
    for kind in [
        "thought",
        "action",
        "response",
        "prompt",
        "error",
        "elicitation",
    ] {
        let position = text.find(&format!("- **{kind}**")).expect(kind);
        assert!(position > previous, "{kind} out of order");
        previous = position;
    }
    assert!(text.contains("- **action** (2020-01-02) - run: "));
    assert!(text.contains("- **thought** (2020-01-02) - line one line two"));
    assert!(!text.contains("JSON only"), "action results are JSON-only");
    assert!(text.contains("**Dismissed by:** Dismiss User"));
    assert!(text.contains("**Creator:** Creator"));
}

#[test]
fn markdown_omits_empty_details() {
    let now = Utc.with_ymd_and_hms(2026, 9, 30, 12, 0, 0).unwrap();
    let mut wire = wire();
    let details = &mut wire["data"]["agentSession"];
    details["type"] = Value::Null;
    for field in ["startedAt", "endedAt", "dismissedAt"] {
        details[field] = Value::Null;
    }
    for field in ["summary", "externalLink"] {
        details[field] = json!("");
    }
    for index in [0, 2, 3, 4, 5] {
        details["activities"]["nodes"][index]["content"]["body"] = json!("");
    }
    details["activities"]["nodes"][1]["content"]["action"] = json!("");
    let text = markdown(&session(&wire).agent_session, now, &Utc).expect("markdown");
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
fn an_unknown_activity_kind_is_refused() {
    let mut value = session(&wire()).agent_session;
    value.activities.nodes[5].content =
        AgentActivityContent::Unsupported("FutureContent".to_owned());
    let error = ensure_supported(&value).expect_err("unknown activity");
    assert!(error.to_string().contains("FutureContent"), "{error}");
    assert!(markdown(&value, Utc::now(), &Utc).is_err());
}
