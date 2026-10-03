use std::collections::BTreeMap;
use std::path::PathBuf;

use super::*;
use crate::auth::CredentialStore;
use crate::auth::keyring::LookupResult;
use crate::auth::test_support::{hit, manifest, store as canned_store};
use crate::config::{
    ConfigInputs, OptionInputs, OsFamily, RawConfigFile, SelectedEnv, parse_config_tier,
};

/// Workspaces `a` (key `ka`, the default) and `b` (no keyring entry).
fn store() -> CredentialStore {
    canned_store(
        manifest("workspaces=['a','b']\ndefault='a'").expect("manifest"),
        &[("a", hit("ka")), ("b", LookupResult::Miss)],
    )
}
#[test]
fn raw_config_cli_workspace_and_default_precedence() {
    let store = store();
    let raw = ConfigSecret::new("raw".to_owned());
    let sourced = ConfigSecret::new("sourced".to_owned());
    let inputs = CredentialSelectionInputs {
        api_key: ApiKeyInput::Raw {
            value: &raw,
            source: OptionSource::Env,
        },
        cli_workspace: Some("b"),
        sourced_workspace: None,
    };
    assert!(matches!(
        resolve(&inputs, &store),
        CredentialSelection::EnvWorkspaceConflict
    ));
    let inputs = CredentialSelectionInputs {
        api_key: ApiKeyInput::Raw {
            value: &raw,
            source: OptionSource::Env,
        },
        cli_workspace: None,
        sourced_workspace: None,
    };
    assert!(
        matches!(resolve(&inputs, &store), CredentialSelection::Selected { secret, source: CredentialSource::Raw(OptionSource::Env) } if secret.expose() == "raw")
    );
    let inputs = CredentialSelectionInputs {
        api_key: ApiKeyInput::Sourced {
            value: &sourced,
            source: OptionSource::ProjectConfig {
                path: PathBuf::from("/repo/linear.toml"),
            },
        },
        cli_workspace: Some("b"),
        sourced_workspace: None,
    };
    assert!(
        matches!(resolve(&inputs, &store), CredentialSelection::Selected { secret, source: CredentialSource::Sourced(_) } if secret.expose() == "sourced")
    );
    let inputs = CredentialSelectionInputs {
        api_key: ApiKeyInput::Absent,
        cli_workspace: None,
        sourced_workspace: None,
    };
    assert!(
        matches!(resolve(&inputs, &store), CredentialSelection::Selected { secret, source: CredentialSource::DefaultWorkspace { .. } } if secret.expose() == "ka")
    );
}

#[test]
fn empty_raw_shadows_config_and_workspace_fallbacks_are_exact() {
    let store = store();
    let empty = ConfigSecret::new(String::new());
    let inputs = CredentialSelectionInputs {
        api_key: ApiKeyInput::Raw {
            value: &empty,
            source: OptionSource::ProjectEnv {
                path: PathBuf::from("/repo/.env"),
            },
        },
        cli_workspace: Some("b"),
        sourced_workspace: None,
    };
    assert!(matches!(
        resolve(&inputs, &store),
        CredentialSelection::MissingExplicitWorkspace { workspace: "b" }
    ));
    let inputs = CredentialSelectionInputs {
        api_key: ApiKeyInput::Absent,
        cli_workspace: Some(""),
        sourced_workspace: Some(("b", OptionSource::Env)),
    };
    assert!(matches!(
        resolve(&inputs, &store),
        CredentialSelection::Selected {
            source: CredentialSource::DefaultWorkspace { .. },
            ..
        }
    ));
    let inputs = CredentialSelectionInputs {
        api_key: ApiKeyInput::Absent,
        cli_workspace: Some("A"),
        sourced_workspace: None,
    };
    assert!(matches!(
        resolve(&inputs, &store),
        CredentialSelection::MissingExplicitWorkspace { workspace: "A" }
    ));
    let inputs = CredentialSelectionInputs {
        api_key: ApiKeyInput::Absent,
        cli_workspace: None,
        sourced_workspace: Some(("", OptionSource::Env)),
    };
    assert!(matches!(
        resolve(&inputs, &store),
        CredentialSelection::Selected {
            source: CredentialSource::DefaultWorkspace { .. },
            ..
        }
    ));
}

#[test]
fn no_key_is_a_normal_outcome() {
    let store = canned_store(manifest("").expect("manifest"), &[]);
    let inputs = CredentialSelectionInputs {
        api_key: ApiKeyInput::Absent,
        cli_workspace: None,
        sourced_workspace: None,
    };
    assert!(matches!(
        resolve(&inputs, &store),
        CredentialSelection::NoKey
    ));
}

#[test]
fn successful_workspace_selections_and_empty_cached_keys() {
    let store = store();
    let explicit = CredentialSelectionInputs {
        api_key: ApiKeyInput::Absent,
        cli_workspace: Some("a"),
        sourced_workspace: None,
    };
    assert!(
        matches!(resolve(&explicit, &store), CredentialSelection::Selected { secret, source: CredentialSource::ExplicitWorkspace { workspace } } if secret.expose() == "ka" && workspace == "a")
    );
    let sourced = CredentialSelectionInputs {
        api_key: ApiKeyInput::Absent,
        cli_workspace: None,
        sourced_workspace: Some((
            "a",
            OptionSource::ProjectConfig {
                path: PathBuf::from("/repo/linear.toml"),
            },
        )),
    };
    assert!(
        matches!(resolve(&sourced, &store), CredentialSelection::Selected { secret, source: CredentialSource::SourcedWorkspace { workspace, source: OptionSource::ProjectConfig { .. } } } if secret.expose() == "ka" && workspace == "a")
    );
    let empty_sourced_key = ConfigSecret::new(String::new());
    let inputs = CredentialSelectionInputs {
        api_key: ApiKeyInput::Sourced {
            value: &empty_sourced_key,
            source: OptionSource::ProjectConfig {
                path: PathBuf::from("/repo/linear.toml"),
            },
        },
        cli_workspace: None,
        sourced_workspace: None,
    };
    assert!(matches!(
        resolve(&inputs, &store),
        CredentialSelection::Selected {
            source: CredentialSource::DefaultWorkspace { .. },
            ..
        }
    ));
    let raw = ConfigSecret::new("lin_api_fake_unique_marker".to_owned());
    let inputs = CredentialSelectionInputs {
        api_key: ApiKeyInput::Raw {
            value: &raw,
            source: OptionSource::ProjectEnv {
                path: PathBuf::from("/repo/.env"),
            },
        },
        cli_workspace: Some("a"),
        sourced_workspace: None,
    };
    assert!(matches!(
        resolve(&inputs, &store),
        CredentialSelection::EnvWorkspaceConflict
    ));
    let inputs = CredentialSelectionInputs {
        api_key: ApiKeyInput::Raw {
            value: &raw,
            source: OptionSource::Env,
        },
        cli_workspace: None,
        sourced_workspace: None,
    };
    assert!(!format!("{:?}", resolve(&inputs, &store)).contains("lin_api_fake_unique_marker"));
}

#[test]
fn empty_default_cache_yields_no_key_and_empty_explicit_cache_is_missing() {
    let store = canned_store(
        manifest("workspaces=['a']\ndefault='a'").expect("manifest"),
        &[("a", hit(""))],
    );
    let default = CredentialSelectionInputs {
        api_key: ApiKeyInput::Absent,
        cli_workspace: None,
        sourced_workspace: None,
    };
    assert!(matches!(
        resolve(&default, &store),
        CredentialSelection::NoKey
    ));
    let explicit = CredentialSelectionInputs {
        api_key: ApiKeyInput::Absent,
        cli_workspace: Some("a"),
        sourced_workspace: None,
    };
    assert!(matches!(
        resolve(&explicit, &store),
        CredentialSelection::MissingExplicitWorkspace { workspace: "a" }
    ));
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
        ApiKeyInput::Sourced {
            source: OptionSource::ProjectConfig { .. },
            ..
        }
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
        ApiKeyInput::Sourced {
            source: OptionSource::GlobalConfig { .. },
            ..
        }
    ));
}
