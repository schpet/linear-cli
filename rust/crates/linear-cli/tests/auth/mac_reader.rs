//! Runs only private fake executables, never /usr/bin/security.
use super::Sandbox;
use linear_cli::auth::keyring::{ProcessKeyringReader, ProcessLookupFailure, ReaderFlavor};
use std::{collections::BTreeMap, ffi::OsString, fs, time::Duration};

fn reader(sandbox: &Sandbox) -> ProcessKeyringReader {
    ProcessKeyringReader::with_test_environment(
        ReaderFlavor::MacSecurity,
        sandbox.executable.clone().into_os_string(),
        BTreeMap::from([
            (
                OsString::from("PATH"),
                sandbox.root.clone().into_os_string(),
            ),
            (
                OsString::from("TRACE"),
                sandbox.root.join("trace").into_os_string(),
            ),
        ]),
        Duration::from_secs(2),
    )
}

#[test]
fn mac_flavor_has_exact_literal_argv_null_stdin_and_bom_js_trim() {
    let sandbox = Sandbox::new(
        "printf '%s\\n' \"$@\" > \"$TRACE\"; if IFS= read -r line; then exit 8; fi; printf '\\357\\273\\277 \\tdummy_mac\\r\\n'",
    );
    let key = reader(&sandbox)
        .lookup_detailed("dummy space")
        .unwrap()
        .unwrap();
    assert_eq!(key.expose(), "dummy_mac");
    assert_eq!(
        fs::read_to_string(sandbox.root.join("trace")).unwrap(),
        "find-generic-password\n-a\ndummy space\n-s\nlinear-cli\n-w\n"
    );
}

#[test]
fn mac_miss44_ignores_output_but_other_exit_and_invalid_utf8_stay_typed() {
    let absent = Sandbox::new("printf '\\377' >&2; printf ignored; exit 44");
    assert!(reader(&absent).lookup_detailed("dummy").unwrap().is_none());
    let wrong_exit = Sandbox::new("exit 1");
    assert_eq!(
        reader(&wrong_exit).lookup_detailed("dummy").unwrap_err(),
        ProcessLookupFailure::ExitFailure
    );
    let empty = Sandbox::new("printf '\\357\\273\\277 \\t\\r\\n'");
    assert!(reader(&empty).lookup_detailed("dummy").unwrap().is_none());
    let invalid = Sandbox::new("printf '\\377'");
    assert_eq!(
        reader(&invalid).lookup_detailed("dummy").unwrap_err(),
        ProcessLookupFailure::InvalidUtf8
    );
    let noisy = Sandbox::new("printf dummy_error_bytes >&2; exit 5");
    let error = reader(&noisy).lookup_detailed("dummy").unwrap_err();
    assert_eq!(error, ProcessLookupFailure::ExitFailure);
    assert!(!format!("{error:?}").contains("dummy_error_bytes"));
}
