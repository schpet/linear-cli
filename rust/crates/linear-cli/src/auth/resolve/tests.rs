use std::collections::BTreeMap;
use std::path::PathBuf;

use super::*;
use crate::auth::CredentialStore;
use crate::auth::keyring::LookupResult;
use crate::auth::test_support::{hit, manifest, store as canned_store};
use crate::config::{
    ConfigInputs, OptionInputs, OsFamily, RawConfigFile, SelectedEnv, parse_config_tier,
};

/// Workspaces `a` (key `ka`, the default), `b` (no keyring entry) and `c`
/// (an empty keyring entry).
fn store() -> CredentialStore {
    canned_store(
        manifest("workspaces=['a','b','c']\ndefault='a'").expect("manifest"),
        &[("a", hit("ka")), ("b", LookupResult::Miss), ("c", hit(""))],
    )
}

fn inputs<'a>(
    api_key: ApiKeyInput<'a>,
    cli_workspace: Option<&'a str>,
    sourced_workspace: Option<&'a str>,
) -> CredentialSelectionInputs<'a> {
    CredentialSelectionInputs {
        api_key,
        cli_workspace,
        sourced_workspace: sourced_workspace.map(|workspace| (workspace, OptionSource::Env)),
    }
}

/// The selection, with the selected key spelled out.
fn outcome(inputs: &CredentialSelectionInputs<'_>, store: &CredentialStore) -> String {
    match resolve(inputs, store) {
        CredentialSelection::Selected { secret, workspace } => {
            format!("{} from {workspace:?}", secret.expose())
        }
        CredentialSelection::NoKey => "no key".to_owned(),
        CredentialSelection::EnvWorkspaceConflict { source } => format!("conflict ({source:?})"),
        CredentialSelection::Unavailable {
            workspace,
            choice,
            stored,
        } => format!("{workspace} unavailable ({choice:?}, stored: {stored})"),
    }
}

#[test]
fn configured_api_keys_come_first() {
    let store = store();
    let raw = ConfigSecret::new("raw".to_owned());
    let raw = || ApiKeyInput::Raw {
        value: &raw,
        source: OptionSource::Env,
    };
    let sourced = ConfigSecret::new("sourced".to_owned());
    let sourced = || ApiKeyInput::Sourced { value: &sourced };
    assert_eq!(
        outcome(&inputs(raw(), None, Some("b")), &store),
        "raw from None"
    );
    assert_eq!(
        outcome(&inputs(raw(), Some("a"), None), &store),
        "conflict (Env)"
    );
    // A key from a config file outranks --workspace.
    assert_eq!(
        outcome(&inputs(sourced(), Some("b"), None), &store),
        "sourced from None"
    );
}

#[test]
fn empty_api_keys_fall_through_to_stored_credentials() {
    let store = store();
    let empty = ConfigSecret::new(String::new());
    let raw = ApiKeyInput::Raw {
        value: &empty,
        source: OptionSource::Env,
    };
    assert_eq!(
        outcome(&inputs(raw, None, None), &store),
        "ka from Some(\"a\")"
    );
    let sourced = ApiKeyInput::Sourced { value: &empty };
    assert_eq!(
        outcome(&inputs(sourced, Some("a"), None), &store),
        "ka from Some(\"a\")"
    );
}

#[test]
fn the_flag_then_the_configured_then_the_default_workspace_is_used() {
    let store = store();
    let absent = || ApiKeyInput::Absent;
    assert_eq!(
        outcome(&inputs(absent(), Some("a"), Some("b")), &store),
        "ka from Some(\"a\")"
    );
    assert_eq!(
        outcome(&inputs(absent(), None, Some("a")), &store),
        "ka from Some(\"a\")"
    );
    assert_eq!(
        outcome(&inputs(absent(), None, None), &store),
        "ka from Some(\"a\")"
    );
    // Empty names count as unset.
    assert_eq!(
        outcome(&inputs(absent(), Some(""), Some("")), &store),
        "ka from Some(\"a\")"
    );
}

#[test]
fn a_chosen_workspace_without_a_usable_key_never_falls_back() {
    let store = store();
    let absent = || ApiKeyInput::Absent;
    assert_eq!(
        outcome(&inputs(absent(), Some("b"), None), &store),
        "b unavailable (Flag, stored: true)"
    );
    assert_eq!(
        outcome(&inputs(absent(), Some("A"), None), &store),
        "A unavailable (Flag, stored: false)"
    );
    assert_eq!(
        outcome(&inputs(absent(), None, Some("b")), &store),
        "b unavailable (Configured(Env), stored: true)"
    );
    assert_eq!(
        outcome(&inputs(absent(), None, Some("zzz")), &store),
        "zzz unavailable (Configured(Env), stored: false)"
    );
    assert_eq!(
        outcome(&inputs(absent(), Some("c"), None), &store),
        "c unavailable (Flag, stored: true)"
    );
    let default_c = canned_store(
        manifest("workspaces=['c']").expect("manifest"),
        &[("c", hit(""))],
    );
    assert_eq!(
        outcome(&inputs(absent(), None, None), &default_c),
        "c unavailable (Default, stored: true)"
    );
}

#[test]
fn no_stored_workspace_is_no_key() {
    let empty = canned_store(manifest("").expect("manifest"), &[]);
    assert_eq!(
        outcome(&inputs(ApiKeyInput::Absent, None, None), &empty),
        "no key"
    );
    let no_default = canned_store(
        manifest("workspaces=['a','b']").expect("manifest"),
        &[("a", hit("ka"))],
    );
    assert_eq!(
        outcome(&inputs(ApiKeyInput::Absent, None, None), &no_default),
        "no key"
    );
}

#[test]
fn debug_output_redacts_the_key() {
    let store = store();
    let raw = ConfigSecret::new("lin_api_fake_unique_marker".to_owned());
    let raw = ApiKeyInput::Raw {
        value: &raw,
        source: OptionSource::Env,
    };
    let selection = resolve(&inputs(raw, None, None), &store);
    assert!(!format!("{selection:?}").contains("lin_api_fake_unique_marker"));
}

#[test]
fn configured_secret_maps_to_raw_or_sourced_without_copying() {
    let make_env = |pairs: &[(&str, &str)]| ConfigInputs {
        cwd: PathBuf::from("/repo"),
        os: OsFamily::Unix,
        process_env: pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect(),
    };
    let none = SelectedEnv {
        applied: BTreeMap::new(),
        source_path: None,
        diagnostics: vec![],
    };
    let process = make_env(&[("LINEAR_API_KEY", "process")]);
    let options = ConfigOptions::from_inputs(OptionInputs {
        env: &process,
        dotenv: &none,
        project: None,
        global: None,
    })
    .expect("process options");
    assert!(matches!(
        ApiKeyInput::from_options(&options),
        ApiKeyInput::Raw {
            source: OptionSource::Env,
            ..
        }
    ));
    let applied = SelectedEnv {
        applied: BTreeMap::from([("LINEAR_API_KEY".to_owned(), "dotenv".to_owned())]),
        source_path: Some(PathBuf::from("/repo/.env")),
        diagnostics: vec![],
    };
    let options = ConfigOptions::from_inputs(OptionInputs {
        env: &make_env(&[]),
        dotenv: &applied,
        project: None,
        global: None,
    })
    .expect("dotenv options");
    assert!(matches!(
        ApiKeyInput::from_options(&options),
        ApiKeyInput::Raw {
            source: OptionSource::ProjectEnv { .. },
            ..
        }
    ));
    let project = parse_config_tier(RawConfigFile {
        path: PathBuf::from("/repo/linear.toml"),
        bytes: b"api_key='project'".to_vec(),
    })
    .expect("TOML");
    let options = ConfigOptions::from_inputs(OptionInputs {
        env: &make_env(&[]),
        dotenv: &none,
        project: Some(&project),
        global: None,
    })
    .expect("project options");
    assert!(matches!(
        ApiKeyInput::from_options(&options),
        ApiKeyInput::Sourced { .. }
    ));
    let options = ConfigOptions::from_inputs(OptionInputs {
        env: &make_env(&[]),
        dotenv: &none,
        project: None,
        global: Some(&project),
    })
    .expect("global options");
    assert!(matches!(
        ApiKeyInput::from_options(&options),
        ApiKeyInput::Sourced { .. }
    ));
}
