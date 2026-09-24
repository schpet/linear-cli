use linear_cli::graphql::operations::agent_session::{
    ActionResultData, AgentActivityType, AgentSessionStatus,
};
use linear_cli::graphql::scalars::{DateTime, Duration, Json, JsonObject, TimelessDate, Uuid};
use serde_json::{Value, from_str, from_value, json, to_value};

#[test]
fn json_scalar_is_a_string_and_rejects_other_shapes() {
    let parsed: Json = from_str(r#""{\"a\":1}""#).expect("string accepted");
    assert_eq!(parsed, Json(r#"{"a":1}"#.to_owned()));
    assert_eq!(to_value(&parsed).expect("value"), Value::from(r#"{"a":1}"#));
    assert!(from_value::<Json>(json!(42)).is_err());
    assert!(from_value::<Json>(json!({"a": 1})).is_err());
    assert!(from_value::<Json>(Value::Null).is_err());
}

#[test]
fn json_object_scalar_is_an_object_and_rejects_other_shapes() {
    let parsed: JsonObject =
        from_value(json!({"type": "doc", "content": [1, "x", null]})).expect("object accepted");
    assert_eq!(parsed.0.get("type"), Some(&Value::from("doc")));
    assert_eq!(
        to_value(&parsed).expect("value"),
        json!({"type": "doc", "content": [1, "x", null]})
    );
    assert!(from_value::<JsonObject>(json!("{\"type\":\"doc\"}")).is_err());
    assert!(from_value::<JsonObject>(json!([1, 2])).is_err());
    assert!(from_value::<JsonObject>(json!(1)).is_err());
}

#[test]
fn embedded_result_data_fixture_parses_through_the_schema_checked_fragment() {
    let parsed: ActionResultData =
        from_value(json!({"resultData": {"type": "doc", "content": []}})).expect("fragment");
    let data = parsed.result_data.as_ref().expect("present");
    assert_eq!(data.0.get("type"), Some(&Value::from("doc")));
    assert_eq!(
        serde_json::to_string(&parsed).expect("string"),
        r#"{"resultData":{"type":"doc","content":[]}}"#
    );
    let absent: ActionResultData = from_value(json!({"resultData": null})).expect("null");
    assert!(absent.result_data.is_none());
    assert!(from_value::<ActionResultData>(json!({"resultData": "{}"})).is_err());
}

#[test]
fn string_scalars_preserve_wire_text_and_reject_numbers() {
    let date: DateTime = from_value(json!("2026-09-23T10:00:00.000Z")).expect("string");
    assert_eq!(date.0, "2026-09-23T10:00:00.000Z");
    assert!(from_value::<DateTime>(json!(1_700_000_000)).is_err());
    assert!(from_value::<DateTime>(Value::Null).is_err());
    let day: TimelessDate = from_value(json!("2026")).expect("shortcut kept verbatim");
    assert_eq!(day.0, "2026");
    assert!(from_value::<TimelessDate>(json!(true)).is_err());
    let duration: Duration = from_value(json!("P2W1D")).expect("duration");
    assert_eq!(to_value(&duration).expect("value"), Value::from("P2W1D"));
    assert!(from_value::<Duration>(json!(86_400_000)).is_err());
    let uuid: Uuid = from_value(json!("9f1c2c8e-1d2b-4a3c-8e5f-0a1b2c3d4e5f")).expect("uuid");
    assert_eq!(uuid.0.len(), 36);
    assert!(from_value::<Uuid>(json!(["not", "a", "uuid"])).is_err());
}

#[test]
fn enums_use_exact_schema_spellings_and_reject_unknown_values() {
    let parsed: AgentActivityType = from_value(json!("thought")).expect("lowercase");
    assert_eq!(parsed, AgentActivityType::Thought);
    assert_eq!(to_value(parsed).expect("value"), Value::from("thought"));
    let action: AgentActivityType = from_value(json!("action")).expect("lowercase");
    assert_eq!(action, AgentActivityType::Action);
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
