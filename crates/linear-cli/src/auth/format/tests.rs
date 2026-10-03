use super::*;
use crate::auth::test_support::{manifest, store};

#[test]
fn inline_file_order_and_metadata_dedup() {
    let inline =
        manifest("\"10\"='k10'\n\"2\"='k2'\n\"01\"='k01'\n\"-1\"='km'\na='ka'\ndefault='a'")
            .expect("inline");
    assert_eq!(inline.format(), CredentialFormat::Inline);
    assert_eq!(inline.workspaces(), ["10", "2", "01", "-1", "a"]);

    let store = store(inline, &[]);
    assert_eq!(store.default(), Some("a"));
    assert_eq!(store.key("2").expect("key").expose(), "k2");
    let meta = manifest("workspaces=['a','a','b']\ndefault='b'").expect("metadata");
    assert_eq!(meta.format(), CredentialFormat::Metadata);
    assert_eq!(meta.workspaces(), ["a", "b"]);
}

#[test]
fn shape_errors_have_fixed_priority_and_no_secret_text() {
    for text in [
        "a='lin_api_fake'\nworkspaces=['a']",
        "workspaces=['a']\na='lin_api_fake'",
    ] {
        let err = manifest(text).expect_err("mixed");
        assert_eq!(err.kind, CredentialFormatErrorKind::MixedFormat);
        assert!(!format!("{err:?} {err}").contains("lin_api_fake"));
    }
    for text in [
        "workspaces='x'",
        "workspaces=[1]",
        "default=1",
        "a=1\nb='key'",
        "a='key'\ndefault=1",
        "workspaces=['a']\n[t]\nx=1",
        "workspaces=['',1]",
        "a='k'\nworkspaces=1\nx=1",
    ] {
        assert_eq!(
            manifest(text).expect_err("wrong type").kind,
            CredentialFormatErrorKind::WrongType
        );
    }
    assert_eq!(
        manifest("\"\"='key'").expect_err("empty inline").kind,
        CredentialFormatErrorKind::EmptyWorkspace
    );
    assert_eq!(
        manifest("workspaces=['']")
            .expect_err("empty metadata")
            .kind,
        CredentialFormatErrorKind::EmptyWorkspace
    );
    let names = |count: usize| {
        (0..count)
            .map(|i| format!("'w{i}'"))
            .collect::<Vec<_>>()
            .join(",")
    };
    assert_eq!(
        manifest(&format!("workspaces=['',{}]", names(256)))
            .expect_err("empty beats cap")
            .kind,
        CredentialFormatErrorKind::EmptyWorkspace
    );
    assert_eq!(
        manifest(&format!("workspaces=[{}]", names(257)))
            .expect_err("cap")
            .kind,
        CredentialFormatErrorKind::TooManyWorkspaces
    );
    assert_eq!(
        manifest(&format!("workspaces=[{}]", names(256)))
            .expect("at cap")
            .workspaces()
            .len(),
        256
    );
}

#[test]
fn default_warning_precedes_lookup_warnings() {
    let manifest = manifest("default='missing'\nworkspaces=['a','b','c']").expect("metadata");
    assert_eq!(manifest.default(), None);
    assert_eq!(
        manifest.warnings(),
        &[CredentialWarning::InvalidDefault {
            workspace: "missing".to_owned()
        }]
    );
    let store = store(
        manifest,
        &[
            (
                "c",
                LookupResult::Failed(LookupFailureCategory::Unavailable),
            ),
            ("a", LookupResult::Hit(ConfigSecret::new(String::new()))),
        ],
    );
    assert_eq!(store.key("a").expect("empty cached hit").expose(), "");
    assert!(store.key("b").is_none());
    assert!(store.key("c").is_none());
    assert_eq!(
        store.take_warnings(),
        &[
            CredentialWarning::InvalidDefault {
                workspace: "missing".to_owned()
            },
            CredentialWarning::LookupMiss {
                workspace: "b".to_owned()
            },
            CredentialWarning::LookupFailed {
                workspace: "c".to_owned(),
                category: LookupFailureCategory::Unavailable
            },
        ]
    );
}

#[test]
fn inline_defaults_are_not_checked_but_metadata_defaults_are() {
    let inline = manifest("a='lin_api_fake'\ndefault='missing'").expect("inline");
    assert_eq!(inline.default(), Some("missing"));
    assert!(inline.warnings().is_empty());
    let only_default = manifest("default='missing'").expect("default only");
    assert_eq!(only_default.workspaces().len(), 0);
    assert_eq!(
        only_default.warnings(),
        &[CredentialWarning::InvalidDefault {
            workspace: "missing".to_owned()
        }]
    );
    let empty_default = manifest("default=''\nworkspaces=['a']").expect("empty default");
    assert_eq!(empty_default.default(), None);
    assert_eq!(
        empty_default.warnings(),
        &[CredentialWarning::InvalidDefault {
            workspace: String::new()
        }]
    );
}

#[test]
fn debug_output_redacts_every_secret() {
    let marker = "lin_api_fake_unique_marker";
    let inline = manifest(&format!("a='{marker}'")).expect("inline");
    assert!(!format!("{inline:?}").contains(marker));
    let store = store(inline, &[]);
    assert!(!format!("{store:?}").contains(marker));
    let reply = LookupResult::Hit(ConfigSecret::new(marker.to_owned()));
    assert!(!format!("{reply:?}").contains(marker));
}
