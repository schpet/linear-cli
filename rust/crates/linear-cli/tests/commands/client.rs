use linear_cli::auth::{CredentialStore, hydrate, parse_credentials};
use linear_cli::commands::client::prepare_transport;
use linear_cli::config::{
    ConfigInputs, ConfigOptions, OptionInputs, OsFamily, ProcessEnvSnapshot, RawConfigFile,
    SelectedEnv, TransportEnvInputs, parse_config_tier,
};
use linear_cli::error::AppErrorKind;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

fn options(env: &[(&str, &str)]) -> ConfigOptions {
    let process = ConfigInputs {
        cwd: PathBuf::from("/repo"),
        os: OsFamily::Unix,
        process_env: env
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect(),
    };
    let dotenv = SelectedEnv {
        applied: BTreeMap::new(),
        source_path: None,
        diagnostics: vec![],
    };
    ConfigOptions::from_inputs(OptionInputs {
        env: &process,
        dotenv: &dotenv,
        project: None,
        global: None,
    })
    .expect("synthetic options")
}

fn credentials(text: &str) -> CredentialStore {
    let tier = parse_config_tier(RawConfigFile {
        path: PathBuf::from("/fake/credentials.toml"),
        bytes: text.as_bytes().to_vec(),
    })
    .expect("credentials TOML");
    hydrate(parse_credentials(tier).expect("manifest"), vec![]).expect("inline store")
}

fn transport_env(values: &[(&str, &str)]) -> TransportEnvInputs {
    let snapshot = ProcessEnvSnapshot::from_vars_os(
        PathBuf::from("/repo"),
        OsFamily::Unix,
        values
            .iter()
            .map(|(name, value)| (OsString::from(name), OsString::from(value))),
    )
    .expect("synthetic process");
    TransportEnvInputs::from_process(&snapshot)
}

#[test]
fn credential_failures_have_no_command_context_or_suggestion() {
    let empty = credentials("");
    let bad_policy = transport_env(&[("HTTP_PROXY", "http://127.0.0.1:9000")]);
    for (config, workspace, expected) in [
        (
            options(&[]),
            None,
            "No API key configured. Set LINEAR_API_KEY, add api_key to .linear.toml, or run `linear auth login`.",
        ),
        (
            options(&[("LINEAR_API_KEY", "lin_api_fake")]),
            Some("acme"),
            "Cannot use --workspace flag when LINEAR_API_KEY environment variable is set. Either unset LINEAR_API_KEY or remove the --workspace flag.",
        ),
        (
            options(&[]),
            Some("acme"),
            "Workspace \"acme\" not found in credentials. Run `linear auth login` to add it, or `linear auth list` to see configured workspaces.",
        ),
    ] {
        let error = prepare_transport(&config, &empty, workspace, &bad_policy)
            .expect_err("credential failure precedes bad transport policy");
        assert_eq!(error.kind, AppErrorKind::Validation);
        assert_eq!(error.context, None);
        assert_eq!(error.message, expected);
        assert_eq!(error.suggestion, None);
        assert!(!error.display_message().contains("Failed to get user info"));
    }
}

#[test]
fn selected_key_failures_keep_shape_order_and_redaction() {
    let store = credentials("");
    let bad_policy = transport_env(&[("HTTP_PROXY", "http://127.0.0.1:9000")]);
    let key = "lin_api_fake";
    let config = options(&[("LINEAR_API_KEY", key)]);
    let policy = prepare_transport(&config, &store, None, &bad_policy)
        .expect_err("valid key reaches transport policy");
    assert_eq!(policy.kind, AppErrorKind::Validation);
    assert_eq!(policy.context, None);
    assert_eq!(
        policy.message,
        "HTTP_PROXY is not supported by this transport mode"
    );
    assert_eq!(
        policy.suggestion.as_deref(),
        Some(
            "Use direct public roots, an absolute SSL_CERT_FILE, or the documented loopback HTTPS proxy mode."
        )
    );
    assert!(!format!("{policy:?}").contains(key));

    let invalid = "lin_api_fake\nsecret";
    let bad_header = options(&[("LINEAR_API_KEY", invalid)]);
    let header = prepare_transport(&bad_header, &store, None, &bad_policy)
        .expect_err("header failure precedes transport policy");
    assert_eq!(header.kind, AppErrorKind::Validation);
    assert_eq!(header.context, None);
    assert_eq!(header.message, "API key cannot be used as an HTTP header");
    assert_eq!(header.suggestion, None);
    let debug = format!("{header:?}");
    assert!(!debug.contains("lin_api_fake"));
    assert!(!debug.contains("secret"));
}

#[test]
fn ca_bundle_failure_has_no_context_and_keeps_selected_key_private() {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "linear-f06c-ca-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).expect("private temp directory");
    let ca_file = root.join("empty-ca.pem");
    fs::write(&ca_file, []).expect("empty CA bundle");
    let ca_path = ca_file.to_str().expect("UTF-8 temp path");
    let key = "lin_api_fake_selected";
    let config = options(&[("LINEAR_API_KEY", key)]);
    let env = transport_env(&[("SSL_CERT_FILE", ca_path)]);
    let error = prepare_transport(&config, &credentials(""), None, &env)
        .expect_err("transport build reads empty captured CA bundle");
    assert_eq!(error.kind, AppErrorKind::Validation);
    assert_eq!(error.context, None);
    assert_eq!(
        error.message,
        format!("SSL_CERT_FILE: CA bundle {} is empty", ca_file.display())
    );
    assert_eq!(error.suggestion, None);
    assert!(!format!("{error:?}").contains(key));
    fs::remove_dir_all(root).expect("remove private temp directory");
}

#[test]
fn valid_fake_key_prepares_public_endpoint_without_context() {
    let config = options(&[("LINEAR_API_KEY", "lin_api_fake")]);
    let transport = prepare_transport(&config, &credentials(""), None, &transport_env(&[]))
        .expect("prepared transport");
    assert_eq!(transport.endpoint().origin(), "https://api.linear.app");
}
