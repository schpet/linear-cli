//! A template's inner `templateData` as the JavaScript value `JSON.parse`
//! builds, for `template view`'s text output only; `--json` never parses it.
//!
//! Numbers are IEEE doubles, as in JavaScript: integer literals are rounded
//! through `f64` and printed with the ECMAScript number spelling. Object keys
//! follow `Object.entries` order: canonical array-index keys ascending, then
//! the other keys in insertion order. A duplicate key keeps its first position
//! and its last value. Unlike `JSON.parse`, an out-of-range number such as
//! `1e400` or an escaped lone surrogate is rejected as invalid JSON.

use std::fmt;

use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

use crate::error::{AppError, AppErrorKind};
use crate::graphql::operations::templates::Template;

/// The largest array index: `2^32 - 2`.
const MAX_ARRAY_INDEX: u64 = 4_294_967_294;

#[derive(Clone, Debug, PartialEq)]
pub enum JsValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JsValue>),
    Object(JsObject),
}

/// Own enumerable properties in `Object.entries` order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct JsObject {
    entries: Vec<(String, JsValue)>,
}

impl JsObject {
    pub fn entries(&self) -> &[(String, JsValue)] {
        &self.entries
    }

    pub fn get(&self, key: &str) -> Option<&JsValue> {
        self.entries
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Build from properties in creation order, as `JSON.parse` defines them.
    pub fn from_created(properties: impl IntoIterator<Item = (String, JsValue)>) -> Self {
        let mut entries: Vec<(String, JsValue)> = Vec::new();
        for (key, value) in properties {
            if let Some((_, existing)) = entries.iter_mut().find(|(name, _)| *name == key) {
                *existing = value;
            } else {
                entries.push((key, value));
            }
        }
        let (mut indexed, named): (Vec<_>, Vec<_>) = entries
            .into_iter()
            .map(|(key, value)| (array_index(&key), key, value))
            .partition(|(index, _, _)| index.is_some());
        indexed.sort_by_key(|(index, _, _)| *index);
        Self {
            entries: indexed
                .into_iter()
                .chain(named)
                .map(|(_, key, value)| (key, value))
                .collect(),
        }
    }
}

/// A canonical numeric string no greater than `2^32 - 2`: `"0"` or digits
/// without a leading zero. `"01"`, `"-1"` and `"4294967295"` are not indices.
fn array_index(key: &str) -> Option<u64> {
    let bytes = key.as_bytes();
    let canonical = match bytes {
        [b'0'] => true,
        [b'1'..=b'9', rest @ ..] => rest.len() < 10 && rest.iter().all(u8::is_ascii_digit),
        _ => false,
    };
    if !canonical {
        return None;
    }
    key.parse::<u64>()
        .ok()
        .filter(|index| *index <= MAX_ARRAY_INDEX)
}

impl<'de> Deserialize<'de> for JsValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(JsValueVisitor)
    }
}

struct JsValueVisitor;

impl<'de> Visitor<'de> for JsValueVisitor {
    type Value = JsValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_unit<E>(self) -> Result<JsValue, E> {
        Ok(JsValue::Null)
    }

    fn visit_bool<E>(self, value: bool) -> Result<JsValue, E> {
        Ok(JsValue::Bool(value))
    }

    // `JSON.parse` rounds every numeric literal to the nearest double; the
    // integer-to-float conversions round the same way.
    fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<JsValue, E> {
        value
            .to_string()
            .parse::<f64>()
            .map(JsValue::Number)
            .map_err(E::custom)
    }

    fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<JsValue, E> {
        value
            .to_string()
            .parse::<f64>()
            .map(JsValue::Number)
            .map_err(E::custom)
    }

    fn visit_f64<E>(self, value: f64) -> Result<JsValue, E> {
        Ok(JsValue::Number(value))
    }

    fn visit_str<E>(self, value: &str) -> Result<JsValue, E> {
        Ok(JsValue::String(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<JsValue, E> {
        Ok(JsValue::String(value))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<JsValue, A::Error> {
        let mut items: Vec<JsValue> = Vec::new();
        while let Some(item) = seq.next_element::<JsValue>()? {
            items.push(item);
        }
        Ok(JsValue::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<JsValue, A::Error> {
        let mut properties = Vec::new();
        while let Some(property) = map.next_entry::<String, JsValue>()? {
            properties.push(property);
        }
        Ok(JsValue::Object(JsObject::from_created(properties)))
    }
}

/// `String(number)`: the ECMAScript spelling, so `-0` is `0` and `1e21` is
/// `1e+21`. Parsed values are always finite.
pub fn js_number(value: f64) -> String {
    let mut buffer = ryu_js::Buffer::new();
    buffer.format(value).to_owned()
}

/// `JSON.stringify(value)` without indentation.
pub fn js_stringify(value: &JsValue) -> String {
    let mut output = String::new();
    write_stringified(value, &mut output);
    output
}

fn write_stringified(value: &JsValue, output: &mut String) {
    match value {
        JsValue::Null => output.push_str("null"),
        JsValue::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        JsValue::Number(value) => output.push_str(&js_number(*value)),
        JsValue::String(value) => write_quoted(value, output),
        JsValue::Array(items) => {
            output.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                write_stringified(item, output);
            }
            output.push(']');
        }
        JsValue::Object(object) => {
            output.push('{');
            for (index, (key, item)) in object.entries().iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                write_quoted(key, output);
                output.push(':');
                write_stringified(item, output);
            }
            output.push('}');
        }
    }
}

/// `JSON.stringify`'s string quoting: the two-character escapes, lowercase
/// `\u00xx` for other controls, everything else (including U+2028) verbatim.
fn write_quoted(text: &str, output: &mut String) {
    output.push('"');
    for ch in text.chars() {
        match ch {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\u{8}' => output.push_str("\\b"),
            '\u{c}' => output.push_str("\\f"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            '\u{0}'..='\u{1f}' => output.push_str(&format!("\\u{:04x}", u32::from(ch))),
            _ => output.push(ch),
        }
    }
    output.push('"');
}

/// Decode a template's stringified `templateData` into its top-level object.
pub fn parse_template_data(template: &Template) -> Result<JsObject, AppError> {
    let subject = format!(
        "Template data for \"{}\" ({})",
        template.name,
        template.id.inner()
    );
    let decoded: JsValue = serde_json::from_str(&template.template_data.0).map_err(|error| {
        AppError::new(
            AppErrorKind::Validation,
            format!("{subject} is not valid JSON"),
        )
        .with_source(error)
    })?;
    match decoded {
        JsValue::Object(object) => Ok(object),
        _ => Err(AppError::new(
            AppErrorKind::Validation,
            format!("{subject} is not a JSON object"),
        )),
    }
}
