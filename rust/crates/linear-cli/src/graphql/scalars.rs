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
