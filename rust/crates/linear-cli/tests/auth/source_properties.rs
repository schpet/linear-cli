use linear_cli::auth::{
    CredentialFormatErrorKind, LookupReply, LookupResult, hydrate, parse_credentials,
};
use linear_cli::config::{RawConfigFile, parse_config_tier};
use std::path::PathBuf;

fn tier(text: &str) -> linear_cli::config::ConfigTier {
    parse_config_tier(RawConfigFile {
        path: PathBuf::from("/fake/credentials.toml"),
        bytes: text.as_bytes().to_vec(),
    })
    .unwrap()
}
#[test]
fn shared_parse_omits_proto_property_before_format_and_counts_but_keeps_constructor() {
    let manifest=parse_credentials(tier("default='constructor'\n__proto__='lin_api_fake_proto'\nconstructor='lin_api_fake_constructor'\nalpha='lin_api_fake_alpha'\n")).unwrap();
    assert_eq!(manifest.workspaces(), ["constructor", "alpha"]);
    let store = hydrate(manifest, vec![]).unwrap();
    assert!(store.key("__proto__").is_none());
    assert_eq!(
        store.key("constructor").unwrap().expose(),
        "lin_api_fake_constructor"
    );
    let empty = parse_credentials(tier("__proto__='lin_api_fake_proto'\n")).unwrap();
    assert!(empty.workspaces().is_empty());
    assert!(empty.lookup_requests().is_empty());
}
#[test]
fn metadata_proto_is_a_legal_value_and_ordinary_wrong_type_rejection_is_unchanged() {
    let manifest = parse_credentials(tier(
        "default='__proto__'\nworkspaces=['__proto__','constructor']\n",
    ))
    .unwrap();
    assert_eq!(manifest.lookup_requests(), ["__proto__", "constructor"]);
    let store = hydrate(
        manifest,
        vec![
            LookupReply {
                workspace: "__proto__".to_owned(),
                result: LookupResult::Miss,
            },
            LookupReply {
                workspace: "constructor".to_owned(),
                result: LookupResult::Miss,
            },
        ],
    )
    .unwrap();
    assert_eq!(store.default(), Some("__proto__"));
    assert_eq!(
        parse_credentials(tier("constructor='lin_api_fake_constructor'\nother=7\n"))
            .unwrap_err()
            .kind,
        CredentialFormatErrorKind::WrongType
    );
    assert_eq!(
        parse_credentials(tier(
            "workspaces=['alpha']\nconstructor='lin_api_fake_constructor'\n"
        ))
        .unwrap_err()
        .kind,
        CredentialFormatErrorKind::MixedFormat
    );
}
