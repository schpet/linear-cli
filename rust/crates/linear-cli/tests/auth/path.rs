use linear_cli::auth::credentials_path;
use linear_cli::config::OsFamily;
use std::path::PathBuf;

#[test]
fn credential_path_obeys_os_bases_and_normalizes_join() {
    assert_eq!(
        credentials_path(OsFamily::Unix, Some("/tmp/a/../b/"), Some("/home/x"), None),
        Some(PathBuf::from("/tmp/b/linear/credentials.toml"))
    );
    assert_eq!(
        credentials_path(OsFamily::Unix, Some(""), Some("/home/x"), None),
        Some(PathBuf::from("/home/x/.config/linear/credentials.toml"))
    );
    assert_eq!(
        credentials_path(
            OsFamily::Windows,
            Some("/unused"),
            None,
            Some("C:/Users/x/AppData/Roaming")
        ),
        Some(PathBuf::from(
            "C:/Users/x/AppData/Roaming/linear/credentials.toml"
        ))
    );
    assert_eq!(
        credentials_path(
            OsFamily::Windows,
            Some("/unused"),
            Some("/unused"),
            Some("")
        ),
        None
    );
    assert_eq!(credentials_path(OsFamily::Unix, None, None, None), None);
}

#[test]
fn credential_path_clamps_root_and_keeps_unresolved_relative_parents() {
    for (base, expected) in [
        ("/..", "/linear/credentials.toml"),
        ("../../", "../../linear/credentials.toml"),
        ("a/../../b", "../b/linear/credentials.toml"),
    ] {
        assert_eq!(
            credentials_path(OsFamily::Unix, Some(base), None, None),
            Some(PathBuf::from(expected)),
            "base {base}"
        );
    }
}
