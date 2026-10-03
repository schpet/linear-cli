//! Custom scalar newtypes for the eight Linear scalars.
//!
//! Every newtype preserves the original wire form. Date, duration and UUID
//! scalars stay strings: no parsing or normalization happens here; values are
//! passed through and formatted only for display.
//! `JSON` is *stringified* JSON (a JSON string on the wire) while `JSONObject`
//! is *embedded* JSON (an object on the wire); the two are deliberately distinct
//! types so one cannot be used where the schema expects the other.

use serde_json::{Map, Value};

use super::schema;

/// ISO 8601 date-time, kept as the exact wire string.
#[derive(cynic::Scalar, Clone, Debug, PartialEq, Eq)]
#[cynic(graphql_type = "DateTime")]
pub struct DateTime(pub String);

/// ISO 8601 date-time or duration, kept as the exact wire string.
#[derive(cynic::Scalar, Clone, Debug, PartialEq, Eq)]
#[cynic(graphql_type = "DateTimeOrDuration")]
pub struct DateTimeOrDuration(pub String);

/// ISO 8601 duration, kept as the exact wire string.
#[derive(cynic::Scalar, Clone, Debug, PartialEq, Eq)]
#[cynic(graphql_type = "Duration")]
pub struct Duration(pub String);

/// ISO 8601 date without time, kept as the exact wire string.
#[derive(cynic::Scalar, Clone, Debug, PartialEq, Eq)]
#[cynic(graphql_type = "TimelessDate")]
pub struct TimelessDate(pub String);

impl From<chrono::NaiveDate> for TimelessDate {
    fn from(date: chrono::NaiveDate) -> Self {
        Self(date.format("%Y-%m-%d").to_string())
    }
}

/// ISO 8601 date or duration, kept as the exact wire string.
#[derive(cynic::Scalar, Clone, Debug, PartialEq, Eq)]
#[cynic(graphql_type = "TimelessDateOrDuration")]
pub struct TimelessDateOrDuration(pub String);

/// RFC 4122 UUID, kept as the exact wire string.
#[derive(cynic::Scalar, Clone, Debug, PartialEq, Eq)]
#[cynic(graphql_type = "UUID")]
pub struct Uuid(pub String);

/// The `JSON` scalar: arbitrary values as *stringified* JSON.
///
/// On the wire this is a JSON string whose content is itself JSON text. The
/// text is retained verbatim; it is not parsed here.
#[derive(cynic::Scalar, Clone, Debug, PartialEq, Eq)]
#[cynic(graphql_type = "JSON")]
pub struct Json(pub String);

/// The `JSONObject` scalar: arbitrary values as *embedded* JSON.
///
/// On the wire this is a JSON object. Only objects are accepted; strings,
/// arrays and other shapes are rejected at deserialization time.
#[derive(cynic::Scalar, Clone, Debug, PartialEq, Eq)]
#[cynic(graphql_type = "JSONObject")]
pub struct JsonObject(pub Map<String, Value>);

#[cfg(test)]
mod tests {
    use super::{DateTime, Duration, Json, JsonObject, TimelessDate, Uuid};
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
    fn string_scalars_preserve_wire_text_and_reject_other_types() {
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
}
