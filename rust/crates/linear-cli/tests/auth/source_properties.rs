use crate::{LookupReply, hydrate};
use linear_cli::auth::{CredentialFormatErrorKind, LookupResult, parse_credentials};
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
fn unusual_workspace_names_are_ordinary_values() {
    let manifest = parse_credentials(tier(
        "default='__proto__'\nworkspaces=['__proto__','constructor']\n",
    ))
    .unwrap();
    assert_eq!(manifest.workspaces(), ["__proto__", "constructor"]);
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
