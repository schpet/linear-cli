//! `--json` output.
//!
//! Every command prints the same shapes: a list command prints a JSON array
//! of its entities, a view command the entity object, and a command that
//! changes something the entity it created or changed. Connections nested in
//! an entity are arrays of their nodes. Nothing carries pagination fields:
//! `--limit` decides how much is fetched.
use serde::Serialize;

/// `value` as pretty-printed JSON with a trailing newline.
pub fn render(value: &impl Serialize) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(value).expect("output types always serialize");
    bytes.push(b'\n');
    bytes
}

/// A string quoted and escaped as JSON, for display within text.
pub(super) fn quoted(text: &str) -> String {
    serde_json::to_string(text).expect("strings always serialize")
}
