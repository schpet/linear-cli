use std::path::PathBuf;

use serde_json::Value;

pub fn expected_for_case(id: &str, case: &Value) -> Value {
    let frozen = &case["expected"];
    let stdout = frozen["stdout"]["utf8"]
        .as_str()
        .expect("help/parser case stdout text");
    if !stdout.contains("2.6.0") {
        return frozen.clone();
    }
    let binding = &case["deviation"];
    assert_eq!(binding["id"], "R01V-CLI-VERSION", "{id} deviation ID");
    assert_eq!(binding["contract"], "rust-3.0.0-alpha.1", "{id} contract");
    assert!(binding["sha256"].as_str().is_some(), "{id} SHA pin");
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../parity/runner/cases/rust-goldens/rust-3.0.0-alpha.1")
        .join(format!("{id}.json"));
    let source = std::fs::read_to_string(path).expect("reviewed Rust golden exists");
    let golden: Value = serde_json::from_str(&source).expect("valid reviewed Rust golden");
    assert_eq!(golden["caseId"], id);
    assert_eq!(golden["deviationId"], binding["id"]);
    assert_eq!(golden["contract"], binding["contract"]);
    assert_eq!(golden["approvedSurfaces"], serde_json::json!(["stdout"]));
    assert_eq!(golden["candidate"]["expected"]["exit"], frozen["exit"]);
    assert_eq!(golden["candidate"]["expected"]["stderr"], frozen["stderr"]);
    assert_eq!(
        golden["candidate"]["expected"]["fileEffects"],
        frozen["fileEffects"]
    );
    golden["candidate"]["expected"].clone()
}
