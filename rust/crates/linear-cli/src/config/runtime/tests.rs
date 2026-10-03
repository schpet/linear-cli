use std::ffi::OsString;

use super::*;

#[test]
fn process_snapshot_filters_and_preserves_empty_values() {
    let cwd = PathBuf::from("/tmp/project");
    let snapshot = ProcessEnvSnapshot::from_vars_os(
        cwd.clone(),
        OsFamily::Unix,
        [
            (OsString::from("LINEAR_API_KEY"), OsString::from("")),
            (
                OsString::from("XDG_CONFIG_HOME"),
                OsString::from("/tmp/xdg"),
            ),
            (OsString::from("SSL_CERT_FILE"), OsString::from("/ca.pem")),
            (
                OsString::from("https_proxy"),
                OsString::from("http://proxy"),
            ),
            (OsString::from("UNRELATED"), OsString::from("ignored")),
            (OsString::from("NO_COLOR"), OsString::from("")),
        ],
    )
    .unwrap();
    assert_eq!(snapshot.inputs.cwd, cwd);
    assert_eq!(snapshot.inputs.env("LINEAR_API_KEY"), Some(""));
    assert_eq!(snapshot.inputs.env("SSL_CERT_FILE"), Some("/ca.pem"));
    assert_eq!(snapshot.inputs.env("NO_COLOR"), Some(""));
    assert!(!snapshot.inputs.process_env.contains_key("https_proxy"));
    assert!(!snapshot.inputs.process_env.contains_key("UNRELATED"));
}

#[test]
fn windows_names_are_case_insensitive_and_duplicates_fail() {
    let windows = ProcessEnvSnapshot::from_vars_os(
        PathBuf::from("C:\\tmp"),
        OsFamily::Windows,
        [
            (OsString::from("linear_api_key"), OsString::from("fake")),
            (OsString::from("no_color"), OsString::from("false")),
        ],
    )
    .unwrap();
    assert_eq!(windows.inputs.env("LINEAR_API_KEY"), Some("fake"));
    assert_eq!(windows.inputs.env("NO_COLOR"), Some("false"));
    let duplicate = ProcessEnvSnapshot::from_vars_os(
        PathBuf::from("C:\\tmp"),
        OsFamily::Windows,
        [
            (OsString::from("LINEAR_API_KEY"), OsString::from("one")),
            (OsString::from("linear_api_key"), OsString::from("two")),
        ],
    );
    assert!(
        matches!(duplicate, Err(ProcessEnvError::DuplicateName { name }) if name == "linear_api_key")
    );
}

#[cfg(unix)]
#[test]
fn relevant_non_utf8_env_value_fails_and_unrelated_one_is_ignored() {
    use std::os::unix::ffi::OsStringExt;
    let bad = OsString::from_vec(vec![0xff]);
    let cwd = PathBuf::from("/tmp/project");
    let result = ProcessEnvSnapshot::from_vars_os(
        cwd.clone(),
        OsFamily::Unix,
        [(OsString::from("LINEAR_API_KEY"), bad.clone())],
    );
    assert!(
        matches!(result, Err(ProcessEnvError::InvalidValue { name }) if name == "LINEAR_API_KEY")
    );
    for name in ["UNRELATED", "HTTPS_PROXY"] {
        let result = ProcessEnvSnapshot::from_vars_os(
            cwd.clone(),
            OsFamily::Unix,
            [(OsString::from(name), bad.clone())],
        );
        assert!(result.is_ok(), "{name}");
    }
    let result = ProcessEnvSnapshot::from_vars_os(
        cwd,
        OsFamily::Unix,
        [(
            OsString::from_vec(b"LINEAR_\xff".to_vec()),
            OsString::from("value"),
        )],
    );
    assert!(matches!(result, Err(ProcessEnvError::InvalidName)));
}
