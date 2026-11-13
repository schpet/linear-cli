use std::path::PathBuf;

use super::no_keyring_hint;
use crate::config::OptionSource;

const INSTALL: &str = "Install it.";

#[test]
fn without_a_configured_key_the_hint_offers_plaintext_or_the_environment() {
    assert_eq!(
        no_keyring_hint(Some(INSTALL), None),
        "Install it.\n  Or pass --plaintext to store the key in the credentials file, \
         or set LINEAR_API_KEY instead of logging in."
    );
    assert_eq!(
        no_keyring_hint(None, None),
        "Pass --plaintext to store the key in the credentials file, \
         or set LINEAR_API_KEY instead of logging in."
    );
}

#[test]
fn a_configured_key_is_named_and_never_suggested_again() {
    let path = PathBuf::from("/work/file");
    for (source, origin) in [
        (OptionSource::Env, "LINEAR_API_KEY is set"),
        (
            OptionSource::ProjectEnv { path: path.clone() },
            "LINEAR_API_KEY is set in /work/file",
        ),
        (
            OptionSource::ProjectConfig { path: path.clone() },
            "An API key is set in /work/file",
        ),
        (
            OptionSource::GlobalConfig { path: path.clone() },
            "An API key is set in /work/file",
        ),
    ] {
        let in_use =
            format!("{origin}, so other commands already use it and there is no need to log in.");
        assert_eq!(
            no_keyring_hint(Some(INSTALL), Some(&source)),
            format!("{in_use}\n  To store a key anyway, pass --plaintext. Install it.")
        );
        assert_eq!(
            no_keyring_hint(None, Some(&source)),
            format!("{in_use}\n  To store a key anyway, pass --plaintext.")
        );
    }
}
