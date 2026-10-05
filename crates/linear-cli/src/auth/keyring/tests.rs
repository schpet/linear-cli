//! A round trip through the real system keyring. Ignored by default because
//! it needs an unlocked keyring; CI runs it with `--ignored`.

use super::{LookupResult, native};
use crate::config::{ChildEnvOverlay, ConfigSecret};

#[test]
#[ignore = "uses the system keyring"]
fn system_keyring_round_trip() {
    let keyring = native(&ChildEnvOverlay::empty());
    let workspace = format!("linear-cli-integration-test-{}", std::process::id());
    assert!(keyring.available(), "no system keyring is available");
    assert!(matches!(keyring.get(&workspace), LookupResult::Miss));

    keyring
        .set(
            &workspace,
            &ConfigSecret::new("lin_api_test_secret".to_owned()),
        )
        .expect("store the key");
    let stored = keyring.get(&workspace);
    keyring.delete(&workspace).expect("delete the key");

    match stored {
        LookupResult::Hit(secret) => assert_eq!(secret.expose(), "lin_api_test_secret"),
        other => panic!("expected the stored key, got {other:?}"),
    }
    assert!(matches!(keyring.get(&workspace), LookupResult::Miss));
}
