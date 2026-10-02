use linear_cli::auth::{
    CredentialFormat, CredentialFormatErrorKind, CredentialInvariantError, CredentialManifest,
    CredentialWarning, LookupFailureCategory, LookupReply, LookupResult, hydrate,
    parse_credentials,
};
use linear_cli::config::{ConfigSecret, RawConfigFile, parse_config_tier};
use std::path::PathBuf;

fn parse_manifest(
    text: &str,
) -> Result<CredentialManifest, linear_cli::auth::CredentialFormatError> {
    let tier = parse_config_tier(RawConfigFile {
        path: PathBuf::from("/fake/credentials.toml"),
        bytes: text.as_bytes().to_vec(),
    })
    .expect("valid TOML fixture");
    parse_credentials(tier)
}

#[test]
fn inline_file_order_and_metadata_dedup() {
    let inline =
        parse_manifest("\"10\"='k10'\n\"2\"='k2'\n\"01\"='k01'\n\"-1\"='km'\na='ka'\ndefault='a'")
            .expect("inline");
    assert_eq!(inline.format(), CredentialFormat::Inline);
    assert_eq!(inline.workspaces(), ["10", "2", "01", "-1", "a"]);
    assert!(inline.lookup_requests().is_empty());
    let store = hydrate(inline, vec![]).expect("inline needs no replies");
    assert_eq!(store.default(), Some("a"));
    assert_eq!(store.key("2").expect("key").expose(), "k2");
    let meta = parse_manifest("workspaces=['a','a','b']\ndefault='b'").expect("metadata");
    assert_eq!(meta.format(), CredentialFormat::Metadata);
    assert_eq!(meta.workspaces(), ["a", "b"]);
    assert_eq!(meta.lookup_requests(), ["a", "b"]);
}

#[test]
fn shape_errors_have_fixed_priority_and_no_secret_text() {
    for text in [
        "a='lin_api_fake'\nworkspaces=['a']",
        "workspaces=['a']\na='lin_api_fake'",
    ] {
        let err = parse_manifest(text).expect_err("mixed");
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
            parse_manifest(text).expect_err("wrong type").kind,
            CredentialFormatErrorKind::WrongType
        );
    }
    assert_eq!(
        parse_manifest("\"\"='key'").expect_err("empty inline").kind,
        CredentialFormatErrorKind::EmptyWorkspace
    );
    assert_eq!(
        parse_manifest("workspaces=['']")
            .expect_err("empty metadata")
            .kind,
        CredentialFormatErrorKind::EmptyWorkspace
    );
    let many_with_empty = format!(
        "workspaces=['',{}]",
        (0..256)
            .map(|i| format!("'w{i}'"))
            .collect::<Vec<_>>()
            .join(",")
    );
    assert_eq!(
        parse_manifest(&many_with_empty)
            .expect_err("empty beats cap")
            .kind,
        CredentialFormatErrorKind::EmptyWorkspace
    );
    let many = (0..257)
        .map(|i| format!("'w{i}'"))
        .collect::<Vec<_>>()
        .join(",");
    assert_eq!(
        parse_manifest(&format!("workspaces=[{many}]"))
            .expect_err("cap")
            .kind,
        CredentialFormatErrorKind::TooManyWorkspaces
    );
    let max = (0..256)
        .map(|i| format!("'w{i}'"))
        .collect::<Vec<_>>()
        .join(",");
    assert_eq!(
        parse_manifest(&format!("workspaces=[{max}]"))
            .expect("at cap")
            .lookup_requests()
            .len(),
        256
    );
}

#[test]
fn default_warning_precedes_lookup_warnings_and_hydration_checks_reply_table() {
    let manifest = parse_manifest("default='missing'\nworkspaces=['a','b','c']").expect("metadata");
    assert_eq!(manifest.default(), None);
    assert_eq!(
        manifest.warnings(),
        &[CredentialWarning::InvalidDefault {
            workspace: "missing".to_owned()
        }]
    );
    let store = hydrate(
        manifest,
        vec![
            LookupReply {
                workspace: "c".to_owned(),
                result: LookupResult::Failed(LookupFailureCategory::Unavailable),
            },
            LookupReply {
                workspace: "a".to_owned(),
                result: LookupResult::Hit(ConfigSecret::new("".to_owned())),
            },
            LookupReply {
                workspace: "b".to_owned(),
                result: LookupResult::Miss,
            },
        ],
    )
    .expect("all replies");
    assert_eq!(store.key("a").expect("empty cached hit").expose(), "");
    assert_eq!(
        store.warnings(),
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
    let manifest = parse_manifest("workspaces=['a','b']").expect("metadata");
    assert_eq!(
        hydrate(
            manifest,
            vec![LookupReply {
                workspace: "a".to_owned(),
                result: LookupResult::Miss
            }]
        )
        .expect_err("missing reply"),
        CredentialInvariantError::MissingReply {
            workspace: "b".to_owned()
        }
    );
    let manifest = parse_manifest("workspaces=['a']").expect("metadata");
    assert_eq!(
        hydrate(
            manifest,
            vec![
                LookupReply {
                    workspace: "a".to_owned(),
                    result: LookupResult::Miss
                },
                LookupReply {
                    workspace: "a".to_owned(),
                    result: LookupResult::Miss
                }
            ]
        )
        .expect_err("duplicate reply"),
        CredentialInvariantError::DuplicateReply {
            workspace: "a".to_owned()
        }
    );
    let manifest = parse_manifest("workspaces=['a']").expect("metadata");
    assert_eq!(
        hydrate(
            manifest,
            vec![LookupReply {
                workspace: "x".to_owned(),
                result: LookupResult::Miss
            }]
        )
        .expect_err("extra reply"),
        CredentialInvariantError::ExtraReply {
            workspace: "x".to_owned()
        }
    );
}

#[test]
fn defaults_and_inline_reply_rules() {
    let inline = parse_manifest("a='lin_api_fake'\ndefault='missing'").expect("inline");
    assert_eq!(inline.default(), Some("missing"));
    assert!(inline.warnings().is_empty());
    assert_eq!(
        hydrate(
            inline,
            vec![LookupReply {
                workspace: "a".to_owned(),
                result: LookupResult::Miss
            }]
        )
        .expect_err("inline reply"),
        CredentialInvariantError::ExtraReply {
            workspace: "a".to_owned()
        }
    );
    let only_default = parse_manifest("default='missing'").expect("default only");
    assert_eq!(only_default.workspaces().len(), 0);
    assert_eq!(
        only_default.warnings(),
        &[CredentialWarning::InvalidDefault {
            workspace: "missing".to_owned()
        }]
    );
    let empty_default = parse_manifest("default=''\nworkspaces=['a']").expect("empty default");
    assert_eq!(empty_default.default(), None);
    assert_eq!(
        empty_default.warnings(),
        &[CredentialWarning::InvalidDefault {
            workspace: String::new()
        }]
    );
}

#[test]
fn every_secret_bearing_debug_path_is_redacted() {
    let marker = "lin_api_fake_unique_marker";
    let inline = parse_manifest(&format!("a='{marker}'")).expect("inline");
    assert!(!format!("{inline:?}").contains(marker));
    let store = hydrate(inline, vec![]).expect("store");
    assert!(!format!("{store:?}").contains(marker));
    let reply = LookupReply {
        workspace: "a".to_owned(),
        result: LookupResult::Hit(ConfigSecret::new(marker.to_owned())),
    };
    assert!(!format!("{reply:?}").contains(marker));
}
