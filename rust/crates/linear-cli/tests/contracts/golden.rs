use std::path::PathBuf;

use serde_json::Value;

pub fn expected_for_case(id: &str, case: &Value) -> Value {
    let frozen = &case["expected"];
    let binding = &case["deviation"];
    if binding.is_null() {
        assert!(
            !frozen["stdout"]["utf8"]
                .as_str()
                .expect("case stdout text")
                .contains("2.6.0"),
            "{id} has an unbound Deno version"
        );
        return frozen.clone();
    }

    let deviation_id = binding["id"].as_str().expect("deviation ID");
    assert!(
        matches!(
            deviation_id,
            "CLAP-NATIVE-PARSER"
                | "R01V-CLI-VERSION"
                | "R01C2-LABEL-LIST-HELP"
                | "R01C2-WORKSPACE-HELP-VALUE"
                | "R01C2-BULK-HELP-PRECEDENCE"
                | "R01C2-ENUM-HELP-VALUE"
                | "R01C2-WORKSPACE-DELIMITER-VALUE"
                | "R01C2-BULK-EMPTY-TAIL"
                | "R01C2-BULK-UNKNOWN-OPTION"
                | "R01C2-EMPTY-HELP-SUFFIX"
                | "R01C2-EMPTY-SWITCH-SUFFIX"
                | "R02B3-STARTUP-VALIDATION"
                | "R02B3-WARNING-VERSION"
        ),
        "{id} has an unreviewed deviation ID: {deviation_id}"
    );
    let contract = binding["contract"].as_str().expect("candidate contract");
    assert_eq!(contract, "rust-3.0.0-alpha.1", "{id} contract");
    assert!(binding["sha256"].as_str().is_some(), "{id} SHA pin");
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../parity/runner/cases/rust-goldens")
        .join(contract)
        .join(format!("{id}.json"));
    let source = std::fs::read_to_string(path).expect("reviewed Rust golden exists");
    let golden: Value = serde_json::from_str(&source).expect("valid reviewed Rust golden");
    assert_eq!(golden["formatVersion"], 1, "{id} golden format");
    assert_eq!(golden["caseId"], id, "{id} case ID");
    assert_eq!(golden["deviationId"], deviation_id, "{id} deviation ID");
    assert_eq!(golden["contract"], contract, "{id} golden contract");
    let candidate = &golden["candidate"]["expected"];
    assert!(candidate.is_object(), "{id} candidate expected output");
    let surfaces = golden["approvedSurfaces"]
        .as_array()
        .expect("approved surfaces");
    assert!(!surfaces.is_empty(), "{id} has no approved surfaces");
    for (name, field) in [
        ("exit", "exit"),
        ("stdout", "stdout"),
        ("stderr", "stderr"),
        ("files", "fileEffects"),
    ] {
        let approved = surfaces.iter().any(|surface| surface == name);
        if approved {
            assert_ne!(candidate[field], frozen[field], "{id} {name} is unchanged");
        } else {
            assert_eq!(
                candidate[field], frozen[field],
                "{id} {name} was not approved"
            );
        }
    }
    for surface in surfaces {
        let name = surface.as_str().expect("surface name");
        assert!(
            ["exit", "stdout", "stderr", "files"].contains(&name),
            "{id} unsupported local golden surface: {name}"
        );
    }
    candidate.clone()
}
