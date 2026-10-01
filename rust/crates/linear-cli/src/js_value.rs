//! Ordered JSON values and ECMAScript binary64 serialization shared by explicit callers.
use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use std::{collections::HashMap, fmt};

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
#[derive(Clone, Default, PartialEq)]
pub struct JsObject {
    entries: Vec<(String, JsValue)>,
    positions: HashMap<String, usize>,
}
impl fmt::Debug for JsObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JsObject")
            .field("entries", &self.entries)
            .finish()
    }
}

impl JsObject {
    pub fn entries(&self) -> &[(String, JsValue)] {
        &self.entries
    }

    pub fn get(&self, key: &str) -> Option<&JsValue> {
        self.positions.get(key).map(|position| {
            &self
                .entries
                .get(*position)
                .unwrap_or_else(|| unreachable!("property position is in bounds"))
                .1
        })
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Build from properties in creation order, as `JSON.parse` defines them.
    pub fn from_created(properties: impl IntoIterator<Item = (String, JsValue)>) -> Self {
        let mut entries: Vec<(String, JsValue)> = Vec::new();
        let mut positions = HashMap::<String, usize>::new();
        for (key, value) in properties {
            if let Some(position) = positions.get(&key) {
                entries
                    .get_mut(*position)
                    .unwrap_or_else(|| unreachable!("created property position is in bounds"))
                    .1 = value;
            } else {
                positions.insert(key.clone(), entries.len());
                entries.push((key, value));
            }
        }
        let (mut indexed, named): (Vec<_>, Vec<_>) = entries
            .into_iter()
            .map(|(key, value)| (array_index(&key), key, value))
            .partition(|(index, _, _)| index.is_some());
        indexed.sort_by_key(|(index, _, _)| *index);
        let entries = indexed
            .into_iter()
            .chain(named)
            .map(|(_, key, value)| (key, value))
            .collect::<Vec<_>>();
        let positions = entries
            .iter()
            .enumerate()
            .map(|(position, (key, _))| (key.clone(), position))
            .collect();
        Self { entries, positions }
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

impl JsObject {
    /// Ordinary JavaScript object assignment invokes the inherited __proto__ setter.
    pub fn assign(&mut self, key: String, value: JsValue) {
        if key == "__proto__" {
            return;
        }
        if let Some(position) = self.positions.get(&key) {
            self.entries
                .get_mut(*position)
                .unwrap_or_else(|| unreachable!("assigned property position is in bounds"))
                .1 = value;
        } else if let Some(index) = array_index(&key) {
            let count = self
                .entries
                .partition_point(|(key, _)| array_index(key).is_some());
            let position = match self
                .entries
                .get(..count)
                .unwrap_or_else(|| unreachable!("numeric prefix length is in bounds"))
                .binary_search_by_key(&index, |(key, _)| {
                    array_index(key)
                        .unwrap_or_else(|| unreachable!("numeric prefix contains indices"))
                }) {
                Err(position) => position,
                Ok(_) => unreachable!("new numeric key is absent from the position index"),
            };
            self.entries.insert(position, (key, value));
            for (position, (key, _)) in self.entries.iter().enumerate().skip(position) {
                self.positions.insert(key.clone(), position);
            }
        } else {
            self.positions.insert(key.clone(), self.entries.len());
            self.entries.push((key, value));
        }
    }
}
impl JsValue {
    pub fn get(&self, key: &str) -> Option<&Self> {
        match self {
            Self::Object(value) => value.get(key),
            _ => None,
        }
    }
    pub fn truthy(&self) -> bool {
        match self {
            Self::Null => false,
            Self::Bool(value) => *value,
            Self::Number(value) => *value != 0.0 && !value.is_nan(),
            Self::String(value) => !value.is_empty(),
            Self::Array(_) | Self::Object(_) => true,
        }
    }
}
/// JSON.stringify with the source's two-space indentation and JS key/number order.
pub fn js_pretty(value: &JsValue) -> String {
    fn pretty(value: &JsValue, depth: usize) -> String {
        let (open, close, items): (char, char, Vec<String>) = match value {
            JsValue::Array(values) => (
                '[',
                ']',
                values.iter().map(|v| pretty(v, depth + 1)).collect(),
            ),
            JsValue::Object(object) => (
                '{',
                '}',
                object
                    .entries()
                    .iter()
                    .map(|(k, v)| {
                        format!(
                            "{}: {}",
                            js_stringify(&JsValue::String(k.clone())),
                            pretty(v, depth + 1)
                        )
                    })
                    .collect(),
            ),
            _ => return js_stringify(value),
        };
        if items.is_empty() {
            return format!("{open}{close}");
        }
        let indent = "  ".repeat(depth + 1);
        format!(
            "{open}\n{indent}{}\n{}{close}",
            items.join(&format!(",\n{indent}")),
            "  ".repeat(depth)
        )
    }
    pretty(value, 0)
}
