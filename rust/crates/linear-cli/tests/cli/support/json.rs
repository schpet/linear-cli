//! Shape-tolerant access to `--json` output.
//!
//! Tests pin entity content (ids, identifiers, titles, states, ...), not the list wrappers around
//! it. Lists may be printed as a bare array, as a `{nodes, pageInfo}` connection, or under a single
//! parent key; these helpers accept all of them so a wrapper change touches only this file.
use serde_json::{Map, Value};

/// The entities in a list output or connection.
#[track_caller]
pub fn nodes(value: &Value) -> Vec<Value> {
    match value {
        Value::Array(items) => items.clone(),
        Value::Object(object) => match connection_nodes(object) {
            Some(items) => items.clone(),
            None if object.len() == 1 => nodes(object.values().next().expect("one entry")),
            None => panic!("no entity list in {value:#}"),
        },
        _ => panic!("no entity list in {value:#}"),
    }
}

/// `value` with every connection object (`{nodes, pageInfo?, totalCount?}`) collapsed to its
/// node array, so two outputs compare equal when they carry the same entities.
fn normalized(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(normalized).collect()),
        Value::Object(object) => match connection_nodes(object) {
            Some(items) => Value::Array(items.iter().map(normalized).collect()),
            None => Value::Object(
                object
                    .iter()
                    .map(|(key, value)| (key.clone(), normalized(value)))
                    .collect(),
            ),
        },
        scalar => scalar.clone(),
    }
}

/// Assert `actual` carries the same content as `expected`, ignoring list wrapper shapes.
#[track_caller]
pub fn assert_json(actual: &Value, expected: &Value) {
    assert_eq!(normalized(actual), normalized(expected));
}

fn connection_nodes(object: &Map<String, Value>) -> Option<&Vec<Value>> {
    let items = object.get("nodes")?.as_array()?;
    object
        .keys()
        .all(|key| matches!(key.as_str(), "nodes" | "pageInfo" | "totalCount"))
        .then_some(items)
}
