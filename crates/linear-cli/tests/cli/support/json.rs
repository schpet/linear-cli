//! Access to `--json` output, which follows one rule: a list command prints a
//! JSON array of entities, a view or mutation prints the entity object, and
//! connections nested in an entity are arrays of their nodes.
use serde_json::{Map, Value};

/// The entities a list command printed.
#[track_caller]
pub fn nodes(value: &Value) -> Vec<Value> {
    match value {
        Value::Array(items) => items.clone(),
        _ => panic!("a list prints a JSON array, got {value:#}"),
    }
}

/// `value` with every connection object (`{nodes, pageInfo?}`) in a fixture
/// collapsed to its node array, as the CLI prints it.
fn flattened(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(flattened).collect()),
        Value::Object(object) => match connection_nodes(object) {
            Some(items) => Value::Array(items.iter().map(flattened).collect()),
            None => Value::Object(
                object
                    .iter()
                    .map(|(key, value)| (key.clone(), flattened(value)))
                    .collect(),
            ),
        },
        scalar => scalar.clone(),
    }
}

/// Assert the output equals a GraphQL fixture, with the fixture's connections
/// flattened. The output itself must not contain connection objects.
#[track_caller]
pub fn assert_json(actual: &Value, expected: &Value) {
    assert_eq!(
        actual,
        &flattened(actual),
        "output contains a connection object"
    );
    assert_eq!(actual, &flattened(expected));
}

fn connection_nodes(object: &Map<String, Value>) -> Option<&Vec<Value>> {
    let items = object.get("nodes")?.as_array()?;
    object
        .keys()
        .all(|key| matches!(key.as_str(), "nodes" | "pageInfo" | "totalCount"))
        .then_some(items)
}
