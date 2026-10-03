//! Team reference cases from `tests/fixtures/team_references.json`: the
//! `team members` argument, workspace inputs, the expected error and the
//! scripted GraphQL exchanges.
use serde_json::Value;

use crate::auth::ApiKeyInput;
use crate::refs::WorkspaceScope;

const TEAM_REFERENCES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/team_references.json"
));

pub(crate) fn case(name: &str) -> Value {
    let all: Value = serde_json::from_str(TEAM_REFERENCES).expect("team reference fixture");
    let mut spec = all[name].clone();
    assert!(spec.is_object(), "missing team reference case {name}");
    spec["id"] = Value::from(name);
    spec
}

pub(crate) fn argument(spec: &Value) -> &str {
    spec["reference"]
        .as_str()
        .unwrap_or_else(|| panic!("{} reference", spec["id"]))
}

pub(crate) fn expected_error(spec: &Value) -> (&str, Option<&str>) {
    let message = spec["error"]["message"]
        .as_str()
        .unwrap_or_else(|| panic!("{} error message", spec["id"]));
    (message, spec["error"]["suggestion"].as_str())
}

pub(crate) fn first_reference(spec: &Value) -> Option<&str> {
    spec["steps"][0]["variables"]["reference"].as_str()
}

/// A scope with no workspace configured anywhere.
pub(crate) fn absent_scope<'a>(key: &'a ApiKeyInput<'a>) -> WorkspaceScope<'a> {
    WorkspaceScope {
        cli_workspace: None,
        sourced_workspace: None,
        default_workspace: None,
        api_key: key.clone(),
    }
}
