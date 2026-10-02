use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

use super::startup::BinarySandbox;

const CASE_IDS: &[&str] = &[
    "c009-absent",
    "c009-absent-no-color-empty",
    "c009-absent-no-color-unset",
    "c009-alias",
    "c009-closed-stdout",
    "c009-debug-absent",
    "c009-dotenv",
    "c009-dotenv-disabled",
    "c009-dotenv-over-project",
    "c009-dotproject",
    "c009-empty-env-shadows-project",
    "c009-env-empty",
    "c009-env-mixed",
    "c009-env-over-dotenv",
    "c009-env-unicode-ligature",
    "c009-env-unicode-sharp",
    "c009-extra-poisoned-config",
    "c009-extra-positional",
    "c009-first-candidate",
    "c009-global",
    "c009-help",
    "c009-help-poisoned-config",
    "c009-inline-credential",
    "c009-invalid-project-shadows-global",
    "c009-invalid-unrelated-option",
    "c009-json-rejected",
    "c009-malformed-credential",
    "c009-malformed-first-fallback",
    "c009-metadata-missing",
    "c009-metadata-no-color-unset",
    "c009-project",
    "c009-project-over-global",
    "c009-unknown-option",
    "c009-valid-env-over-invalid-project",
    "c009-workspace-after",
    "c009-workspace-before",
];

fn cases_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../parity/runner/cases")
}

fn load_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).expect("read frozen case or golden"))
        .expect("valid frozen case or golden JSON")
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create private fixture directory");
    for entry in fs::read_dir(source).expect("read frozen fixture directory") {
        let entry = entry.expect("fixture directory entry");
        let source = entry.path();
        let destination = destination.join(entry.file_name());
        let kind = entry.file_type().expect("fixture entry type");
        if kind.is_dir() {
            copy_tree(&source, &destination);
        } else if kind.is_file() {
            fs::copy(source, destination).expect("copy frozen fixture file");
        } else {
            panic!("unexpected C009 fixture entry");
        }
    }
}

fn case_expected(id: &str, case: &Value) -> Value {
    if case["deviation"].is_null() {
        case["expected"].clone()
    } else {
        let golden = load_json(
            &cases_root()
                .join("rust-goldens/rust-3.0.0-alpha.1")
                .join(format!("{id}.json")),
        );
        assert_eq!(golden["caseId"], id);
        assert_eq!(golden["deviationId"], case["deviation"]["id"]);
        assert_eq!(golden["contract"], "rust-3.0.0-alpha.1");
        golden["candidate"]["expected"].clone()
    }
}

fn substituted(value: &str, sandbox: &BinarySandbox) -> String {
    value
        .replace("{{home}}", &sandbox.root().join("home").to_string_lossy())
        .replace(
            "{{configHome}}",
            &sandbox.root().join("config").to_string_lossy(),
        )
        .replace("{{cwd}}", &sandbox.root().join("cwd").to_string_lossy())
        .replace("{{bin}}", &sandbox.root().join("bin").to_string_lossy())
        .replace(
            "{{denoDir}}",
            &sandbox.root().join("deno").to_string_lossy(),
        )
}

fn prepared_command(case: &Value, sandbox: &BinarySandbox) -> Command {
    let root = sandbox.root();
    for directory in ["config", "deno"] {
        fs::create_dir_all(root.join(directory)).expect("create private directory");
    }
    let fixture = case["cwdFixture"].as_str().expect("cwd fixture name");
    if fixture != "empty" {
        copy_tree(
            &cases_root().join("fixtures").join(fixture),
            &root.join("cwd"),
        );
    }
    if let Some(fixture) = case["configFixture"].as_str() {
        copy_tree(
            &cases_root().join("fixtures").join(fixture),
            &root.join("config"),
        );
    }
    let mut command = Command::new(env!("CARGO_BIN_EXE_linear"));
    command.env_clear().current_dir(root.join("cwd"));
    for (name, value) in case["env"].as_object().expect("frozen environment") {
        command.env(
            name,
            substituted(value.as_str().expect("environment value"), sandbox),
        );
    }
    for argument in case["argv"].as_array().expect("frozen argv") {
        command.arg(argument.as_str().expect("argv item"));
    }
    command
}

/// Drops the parser position and message that follow "invalid TOML".
fn without_toml_position(text: &str) -> String {
    match text.split_once(": invalid TOML at line ") {
        Some((before, after)) => {
            let rest = after.split_once('\n').map_or("", |(_, rest)| rest);
            format!("{before}: invalid TOML\n{rest}")
        }
        None => text.to_owned(),
    }
}

#[test]
fn public_binary_matches_frozen_team_id_cases() {
    for id in CASE_IDS {
        if *id == "c009-closed-stdout" {
            continue;
        }
        let case = load_json(&cases_root().join(format!("{id}.json")));
        if !cfg!(target_os = "linux") && case["configFixture"] == "c009-metadata" {
            continue;
        }
        let sandbox = BinarySandbox::new();
        let output = prepared_command(&case, &sandbox)
            .output()
            .expect("team id public binary runs");
        let expected = case_expected(id, &case);
        assert_eq!(
            output.status.code(),
            expected["exit"]["code"]
                .as_i64()
                .and_then(|code| i32::try_from(code).ok()),
            "{id} exit"
        );
        assert_eq!(
            output.stdout,
            substituted(
                expected["stdout"]["utf8"].as_str().expect("stdout"),
                &sandbox
            )
            .as_bytes(),
            "{id} stdout"
        );
        // Config errors now describe the offending value or TOML position.
        let expected_stderr = substituted(
            expected["stderr"]["utf8"].as_str().expect("stderr"),
            &sandbox,
        )
        .replace(
            ": expected a string\n",
            if *id == "c009-invalid-unrelated-option" {
                ": invalid type: integer `123`, expected manual or priority\n"
            } else {
                ": invalid type: integer `123`, expected a string\n"
            },
        );
        assert_eq!(
            without_toml_position(&String::from_utf8_lossy(&output.stderr)),
            expected_stderr,
            "{id} stderr"
        );
    }
}

#[cfg(unix)]
#[test]
fn team_id_closing_stdout_at_start_is_quiet() {
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;
    use std::process::Stdio;

    let case = load_json(&cases_root().join("c009-closed-stdout.json"));
    let sandbox = BinarySandbox::new();
    let (reader, writer) = UnixStream::pair().expect("create closed output pipe");
    drop(reader);
    let output = prepared_command(&case, &sandbox)
        .stdout(Stdio::from(OwnedFd::from(writer)))
        .output()
        .expect("team id public binary runs");
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
}
