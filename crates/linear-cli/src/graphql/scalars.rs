//! Newtypes for the Linear custom scalars the CLI sends or reads.
//!
//! `DateTime` and `TimelessDate` are parsed into chrono values when a response
//! is decoded, so a malformed timestamp is a decode error rather than odd
//! output later. `DateTimeOrDuration` stays a string because it can hold
//! either form.
//! `JSON` is *stringified* JSON (a JSON string on the wire) while `JSONObject`
//! is *embedded* JSON (an object on the wire); the two are deliberately distinct
//! types so one cannot be used where the schema expects the other.

use std::fmt;

use chrono::{NaiveDate, SecondsFormat, Utc};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use super::schema;

/// An RFC 3339 instant, held in UTC.
///
/// It serializes the way Linear sends it: UTC, millisecond precision, `Z`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DateTime(pub chrono::DateTime<Utc>);

impl<'de> Deserialize<'de> for DateTime {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        chrono::DateTime::parse_from_rfc3339(&text)
            .map(|instant| Self(instant.to_utc()))
            .map_err(|error| D::Error::custom(format!("invalid DateTime {text:?}: {error}")))
    }
}

impl Serialize for DateTime {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_rfc3339_opts(SecondsFormat::Millis, true))
    }
}

cynic::impl_scalar!(DateTime, schema::DateTime);

/// ISO 8601 date-time or duration, kept as the exact wire string.
#[derive(cynic::Scalar, Clone, Debug, PartialEq, Eq)]
#[cynic(graphql_type = "DateTimeOrDuration")]
pub struct DateTimeOrDuration(pub String);

/// A calendar date without time or zone, `YYYY-MM-DD` on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimelessDate(pub NaiveDate);

const TIMELESS_DATE_FORMAT: &str = "%Y-%m-%d";

impl<'de> Deserialize<'de> for TimelessDate {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        // The round trip rejects unpadded forms such as `2026-9-5`, which the
        // parser alone accepts.
        NaiveDate::parse_from_str(&text, TIMELESS_DATE_FORMAT)
            .ok()
            .filter(|date| date.format(TIMELESS_DATE_FORMAT).to_string() == text)
            .map(Self)
            .ok_or_else(|| D::Error::custom(format!("invalid TimelessDate {text:?}")))
    }
}

impl Serialize for TimelessDate {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl fmt::Display for TimelessDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.format(TIMELESS_DATE_FORMAT).fmt(f)
    }
}

impl From<NaiveDate> for TimelessDate {
    fn from(date: NaiveDate) -> Self {
        Self(date)
    }
}

cynic::impl_scalar!(TimelessDate, schema::TimelessDate);

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

/// A `Float` field the schema uses for a whole quantity: cycle and issue
/// numbers, priorities, counts. Decoding rejects anything that is not a
/// whole number in `u32` range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct WholeNumber(pub u32);

impl<'de> Deserialize<'de> for WholeNumber {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let number = serde_json::Number::deserialize(deserializer)?;
        let whole = match number.as_u64() {
            Some(value) => u32::try_from(value).ok(),
            None => number
                .as_f64()
                .filter(|value| value.fract() == 0.0)
                .and_then(|value| value.to_string().parse::<u32>().ok()),
        };
        whole
            .map(Self)
            .ok_or_else(|| D::Error::custom(format!("expected a whole number, found {number}")))
    }
}

impl fmt::Display for WholeNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

cynic::impl_scalar!(WholeNumber, schema::Float);

/// A fractional `Float` field (sort orders, positions, progress). It keeps the
/// number as received, except that a whole value is held as an integer so it
/// is written back without a fractional part.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Float(pub serde_json::Number);

impl<'de> Deserialize<'de> for Float {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let number = serde_json::Number::deserialize(deserializer)?;
        // `f64` displays whole values without an exponent or fraction, so
        // parsing that text is an exact, range-checked conversion.
        let whole = number
            .as_f64()
            .filter(|value| number.is_f64() && value.fract() == 0.0)
            .and_then(|value| value.to_string().parse::<i64>().ok());
        Ok(Self(whole.map_or(number, serde_json::Number::from)))
    }
}

impl Float {
    pub fn get(&self) -> f64 {
        self.0
            .as_f64()
            .unwrap_or_else(|| unreachable!("JSON numbers are representable as f64"))
    }
}

impl fmt::Display for Float {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

cynic::impl_scalar!(Float, schema::Float);

#[cfg(test)]
mod tests {
    use super::{DateTime, Json, JsonObject, TimelessDate};
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
    fn date_time_parses_rfc3339_and_writes_utc_millis() {
        let date: DateTime = from_value(json!("2026-09-23T10:00:00Z")).expect("rfc3339");
        assert_eq!(
            to_value(date).expect("value"),
            json!("2026-09-23T10:00:00.000Z")
        );
        let offset: DateTime = from_value(json!("2026-09-23T15:30:00.5+05:30")).expect("offset");
        assert_eq!(
            to_value(offset).expect("value"),
            json!("2026-09-23T10:00:00.500Z")
        );
        for bad in [
            json!("2026-09-23"),
            json!("2026-09-23 10:00:00"),
            json!("not a date"),
            json!(1_700_000_000),
            Value::Null,
        ] {
            assert!(from_value::<DateTime>(bad.clone()).is_err(), "{bad}");
        }
    }

    #[test]
    fn timeless_date_round_trips_and_rejects_other_forms() {
        let day: TimelessDate = from_value(json!("2026-09-05")).expect("date");
        assert_eq!(day.to_string(), "2026-09-05");
        assert_eq!(to_value(day).expect("value"), json!("2026-09-05"));
        for bad in [
            json!("2026"),
            json!("2026-9-5"),
            json!("2026-02-30"),
            json!("2026-09-05T00:00:00Z"),
            json!(true),
        ] {
            assert!(from_value::<TimelessDate>(bad.clone()).is_err(), "{bad}");
        }
    }
}
