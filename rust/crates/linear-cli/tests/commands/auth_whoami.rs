use linear_cli::app::block_on_network;
use linear_cli::auth::{CredentialStore, LookupReply, LookupResult, hydrate, parse_credentials};
use linear_cli::commands::auth_whoami::{prepare_transport, render, run, run_with};
use linear_cli::config::{
    ConfigInputs, ConfigOptions, ConfigSecret, OptionInputs, OsFamily, ProcessEnvSnapshot,
    RawConfigFile, SelectedEnv, TransportEnvInputs, parse_config_tier,
};
use linear_cli::error::{AppError, AppErrorKind};
use linear_cli::graphql::operations::auth_whoami::AuthStatus;
use linear_cli::graphql::transport::{
    ApiKey, CaMode, Deadline, EndpointUrl, GraphQlTransport, ProxyMode, ResponseCap,
    TransportConfig,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::ffi::OsString;
#[cfg(target_os = "linux")]
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
#[cfg(target_os = "linux")]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
#[cfg(target_os = "linux")]
use std::process::Command;
#[cfg(target_os = "linux")]
use std::process::Output;
#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

fn options(env: &[(&str, &str)], project: Option<&str>) -> ConfigOptions {
    options_with_dotenv(env, project, &[])
}

fn options_with_dotenv(
    env: &[(&str, &str)],
    project: Option<&str>,
    dotenv_values: &[(&str, &str)],
) -> ConfigOptions {
    let process = ConfigInputs {
        cwd: PathBuf::from("/repo"),
        os: OsFamily::Unix,
        process_env: env
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect(),
    };
    let dotenv = SelectedEnv {
        applied: dotenv_values
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect::<BTreeMap<_, _>>(),
        source_path: (!dotenv_values.is_empty()).then(|| PathBuf::from("/repo/.env")),
        diagnostics: vec![],
    };
    let project = project.map(|text| {
        parse_config_tier(RawConfigFile {
            path: PathBuf::from("/repo/.linear.toml"),
            bytes: text.as_bytes().to_vec(),
        })
        .expect("project TOML")
    });
    ConfigOptions::from_inputs(OptionInputs {
        env: &process,
        dotenv: &dotenv,
        project: project.as_ref(),
        global: None,
    })
    .expect("options")
}

fn metadata_credentials(replies: Vec<LookupReply>) -> CredentialStore {
    let tier = parse_config_tier(RawConfigFile {
        path: PathBuf::from("/fake/credentials.toml"),
        bytes: b"default='first'\nworkspaces=['first','second']\n".to_vec(),
    })
    .expect("metadata TOML");
    hydrate(parse_credentials(tier).expect("manifest"), replies).expect("fake lookup table")
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

fn status(display_name: &str, admin: bool, guest: bool, logo: serde_json::Value) -> AuthStatus {
    serde_json::from_value(json!({
        "viewer": {
            "id": "user-fake-1", "name": "Alice Example", "displayName": display_name,
            "email": "alice@example.invalid", "admin": admin, "guest": guest,
            "organization": {"name": "Example Workspace", "urlKey": "acme", "logoUrl": logo}
        }
    }))
    .expect("schema-valid fixture")
}

#[test]
fn human_output_matches_frozen_roles_display_and_ignored_logo() {
    let base = b"Workspace: Example Workspace\n  Slug: acme\n  URL: https://linear.app/acme\nUser: Alice Example\n";
    for (display, admin, guest, logo, middle, tail) in [
        (
            "Ali",
            false,
            false,
            json!(null),
            "  Display name: Ali\n",
            "",
        ),
        (
            "Alice Example",
            false,
            false,
            json!("https://example.invalid/logo"),
            "",
            "",
        ),
        (
            "Ali",
            true,
            false,
            json!(null),
            "  Display name: Ali\n",
            "  Role: admin\n",
        ),
        (
            "Ali",
            false,
            true,
            json!(null),
            "  Display name: Ali\n",
            "  Role: guest\n",
        ),
        (
            "Ali",
            true,
            true,
            json!(null),
            "  Display name: Ali\n",
            "  Role: admin\n",
        ),
    ] {
        let mut expected = base.to_vec();
        expected.extend_from_slice(middle.as_bytes());
        expected.extend_from_slice(b"  Email: alice@example.invalid\n");
        expected.extend_from_slice(tail.as_bytes());
        assert_eq!(render(&status(display, admin, guest, logo)), expected);
    }
}

#[tokio::test]
async fn injected_handler_calls_one_auth_status_and_contextualizes_failure() {
    let output = run_with(|request| async move {
        assert_eq!(request.operation_name.as_deref(), Some("AuthStatus"));
        assert!(request.variables.is_none());
        Ok(status("Ali", false, false, json!(null)))
    })
    .await
    .expect("one response");
    assert!(output.starts_with(b"Workspace: Example Workspace\n"));

    let error = run_with(|_| async { Err(AppError::new(AppErrorKind::GraphQl, "token invalid")) })
        .await
        .expect_err("request failure");
    assert_eq!(
        error.display_message(),
        "Failed to get user info: token invalid"
    );
    assert_eq!(error.kind, AppErrorKind::GraphQl);
}

#[test]
fn credential_failures_precede_transport_policy_and_have_one_exact_line() {
    let unsupported_transport = transport_env(&[("HTTP_PROXY", "http://127.0.0.1:9000")]);
    let empty = credentials("");
    let no_key = prepare_transport(&options(&[], None), &empty, None, &unsupported_transport)
        .expect_err("no key");
    assert_eq!(no_key.kind, AppErrorKind::Validation);
    assert_eq!(no_key.suggestion, None);
    assert_eq!(
        no_key.display_message(),
        "Failed to get user info: No API key configured. Set LINEAR_API_KEY, add api_key to .linear.toml, or run `linear auth login`."
    );

    let raw = options(&[("LINEAR_API_KEY", "lin_api_fake_raw")], None);
    let conflict = prepare_transport(&raw, &empty, Some("acme"), &unsupported_transport)
        .expect_err("raw key conflicts with CLI workspace");
    assert_eq!(
        conflict.display_message(),
        "Failed to get user info: Cannot use --workspace flag when LINEAR_API_KEY environment variable is set. Either unset LINEAR_API_KEY or remove the --workspace flag."
    );
    assert_eq!(conflict.suggestion, None);

    let missing = prepare_transport(
        &options(&[], None),
        &empty,
        Some("acme"),
        &unsupported_transport,
    )
    .expect_err("missing explicit workspace");
    assert_eq!(
        missing.display_message(),
        "Failed to get user info: Workspace \"acme\" not found in credentials. Run `linear auth login` to add it, or `linear auth list` to see configured workspaces."
    );
    assert_eq!(missing.suggestion, None);
}

#[test]
fn sourced_api_key_wins_explicit_workspace_and_transport_policy_is_lazy() {
    let store = credentials("default='acme'\nacme='lin_api_fake_acme'\n");
    let config = options(&[], Some("api_key='lin_api_fake_project'"));
    let invalid_transport = transport_env(&[("HTTP_PROXY", "http://127.0.0.1:9000")]);
    let error = prepare_transport(&config, &store, Some("missing"), &invalid_transport)
        .expect_err("credential selected before transport policy");
    assert!(
        error
            .display_message()
            .starts_with("Failed to get user info: HTTP_PROXY")
    );
    let direct = transport_env(&[]);
    let transport = prepare_transport(&config, &store, Some("missing"), &direct)
        .expect("project key wins explicit missing workspace");
    assert_eq!(transport.endpoint().origin(), "https://api.linear.app");
}

#[test]
fn selected_key_header_failure_precedes_transport_policy_without_leaking_key() {
    let config = options(&[("LINEAR_API_KEY", "lin_api_fake\nsecret")], None);
    let invalid_transport = transport_env(&[("HTTP_PROXY", "http://127.0.0.1:9000")]);
    let error = prepare_transport(&config, &credentials(""), None, &invalid_transport)
        .expect_err("invalid selected key cannot reach transport");
    assert_eq!(error.kind, AppErrorKind::Validation);
    assert_eq!(
        error.display_message(),
        "Failed to get user info: API key cannot be used as an HTTP header"
    );
    assert!(!format!("{error:?}").contains("lin_api_fake"));
}

fn serve_auth_status() -> (String, thread::JoinHandle<String>) {
    serve_response(200, br#"{"data":{"viewer":{"id":"user-fake-1","name":"Alice Example","displayName":"Ali","email":"alice@example.invalid","admin":false,"guest":false,"organization":{"name":"Example Workspace","urlKey":"acme","logoUrl":null}}}}"#.to_vec(), Duration::ZERO)
}

fn serve_response(
    status: u16,
    body: Vec<u8>,
    delay: Duration,
) -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("private listener");
    let endpoint = format!("http://{}/graphql", listener.local_addr().expect("address"));
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("one request");
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .expect("timeout");
        let mut request = Vec::new();
        let mut chunk = [0_u8; 4096];
        let (head_end, content_length) = loop {
            let count = stream.read(&mut chunk).expect("request bytes");
            assert!(count > 0, "request closed before headers");
            request.extend_from_slice(&chunk[..count]);
            assert!(request.len() < 64 * 1024, "request is bounded");
            if let Some(offset) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                let head_end = offset + 4;
                let headers =
                    String::from_utf8(request[..head_end].to_vec()).expect("headers UTF-8");
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().expect("length"))
                    })
                    .expect("content length");
                break (head_end, content_length);
            }
        };
        while request.len() - head_end < content_length {
            let count = stream.read(&mut chunk).expect("body bytes");
            assert!(count > 0, "request closed before body");
            request.extend_from_slice(&chunk[..count]);
            assert!(request.len() < 64 * 1024, "request is bounded");
        }
        thread::sleep(delay);
        let response = format!(
            "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.write_all(&body);
        String::from_utf8(request).expect("request UTF-8")
    });
    (endpoint, server)
}

fn transport_for(endpoint: &str, deadline: Duration, cap: usize) -> GraphQlTransport {
    GraphQlTransport::new(
        EndpointUrl::parse(endpoint).expect("endpoint"),
        ApiKey::new("lin_api_fake".to_owned()).expect("fake key"),
        TransportConfig {
            proxy: ProxyMode::Direct,
            ca: CaMode::PublicRoots,
            deadline: Deadline::new(deadline).expect("bounded test deadline"),
            max_response_bytes: ResponseCap::new(cap).expect("bounded test cap"),
        },
    )
    .expect("test transport")
}

#[test]
fn real_loopback_request_proves_selected_key_operation_and_no_variables() {
    for (source, key, project, store, explicit_workspace) in [
        ("raw", "lin_api_fake_raw", None, "", None),
        (
            "project",
            "lin_api_fake_project",
            Some("api_key='lin_api_fake_project'"),
            "default='acme'\nacme='lin_api_fake_default'\n",
            Some("missing"),
        ),
        (
            "inline-default",
            "lin_api_fake_default",
            None,
            "default='acme'\nacme='lin_api_fake_default'\n",
            None,
        ),
        (
            "explicit",
            "lin_api_fake_second",
            None,
            "default='acme'\nacme='lin_api_fake_default'\nsecond='lin_api_fake_second'\n",
            Some("second"),
        ),
        (
            "sourced-workspace",
            "lin_api_fake_second",
            Some("workspace='second'"),
            "default='acme'\nacme='lin_api_fake_default'\nsecond='lin_api_fake_second'\n",
            None,
        ),
        (
            "missing-sourced-fallback",
            "lin_api_fake_default",
            Some("workspace='missing'"),
            "default='acme'\nacme='lin_api_fake_default'\n",
            None,
        ),
    ] {
        let (endpoint, server) = serve_auth_status();
        let env = if source == "raw" {
            vec![
                ("LINEAR_GRAPHQL_ENDPOINT", endpoint.as_str()),
                ("LINEAR_API_KEY", key),
            ]
        } else {
            vec![("LINEAR_GRAPHQL_ENDPOINT", endpoint.as_str())]
        };
        let options = options(&env, project);
        let credentials = credentials(store);
        let transport = prepare_transport(
            &options,
            &credentials,
            explicit_workspace,
            &transport_env(&[]),
        )
        .expect("prepared transport");
        let output =
            block_on_network(async move { run(&transport).await }).expect("AuthStatus request");
        assert_eq!(
            output,
            render(&status("Ali", false, false, json!(null))),
            "{source}"
        );
        let request = server.join().expect("server joined");
        let (headers, body) = request.split_once("\r\n\r\n").expect("HTTP request");
        assert!(
            headers
                .to_ascii_lowercase()
                .contains(&format!("authorization: {key}")),
            "{source}: selected fake key"
        );
        assert!(
            headers
                .to_ascii_lowercase()
                .contains("user-agent: schpet-linear-cli/3.0.0-alpha.1"),
            "{source}: v3 identity"
        );
        let envelope: serde_json::Value = serde_json::from_str(body).expect("request JSON");
        assert_eq!(envelope["operationName"], "AuthStatus", "{source}");
        assert!(envelope.get("variables").is_none(), "{source}");
    }
}

#[test]
fn dotenv_key_is_raw_and_empty_raw_key_falls_through_to_default() {
    for (name, dotenv_key, workspace, expected_key) in [
        ("dotenv", "lin_api_fake_dotenv", None, "lin_api_fake_dotenv"),
        ("empty-fallback", "", None, "lin_api_fake_default"),
    ] {
        let (endpoint, server) = serve_auth_status();
        let config = options_with_dotenv(
            &[("LINEAR_GRAPHQL_ENDPOINT", &endpoint)],
            Some("api_key='lin_api_fake_project'"),
            &[("LINEAR_API_KEY", dotenv_key)],
        );
        let store = credentials("default='acme'\nacme='lin_api_fake_default'\n");
        let transport = prepare_transport(&config, &store, workspace, &transport_env(&[]))
            .expect("selected fake key");
        block_on_network(async move { run(&transport).await }).expect("AuthStatus response");
        let request = server.join().expect("one request");
        assert!(
            request
                .to_ascii_lowercase()
                .contains(&format!("authorization: {expected_key}")),
            "{name}: wrong selected key"
        );
    }
    let config = options_with_dotenv(&[], None, &[("LINEAR_API_KEY", "lin_api_fake_dotenv")]);
    let error = prepare_transport(&config, &credentials(""), Some("acme"), &transport_env(&[]))
        .expect_err("dotenv raw key conflicts with CLI workspace");
    assert_eq!(error.kind, AppErrorKind::Validation);
    assert!(
        error
            .display_message()
            .contains("Cannot use --workspace flag")
    );
}

#[test]
fn fake_keyring_store_selects_default_and_explicit_workspace() {
    for (workspace, expected_key) in [
        (None, "lin_api_fake_first"),
        (Some("second"), "lin_api_fake_second"),
    ] {
        let (endpoint, server) = serve_auth_status();
        let store = metadata_credentials(vec![
            LookupReply {
                workspace: "first".to_owned(),
                result: LookupResult::Hit(ConfigSecret::new("lin_api_fake_first".to_owned())),
            },
            LookupReply {
                workspace: "second".to_owned(),
                result: LookupResult::Hit(ConfigSecret::new("lin_api_fake_second".to_owned())),
            },
        ]);
        let transport = prepare_transport(
            &options(&[("LINEAR_GRAPHQL_ENDPOINT", &endpoint)], None),
            &store,
            workspace,
            &transport_env(&[]),
        )
        .expect("fake metadata credential");
        block_on_network(async move { run(&transport).await }).expect("AuthStatus response");
        let request = server.join().expect("one request");
        assert!(
            request
                .to_ascii_lowercase()
                .contains(&format!("authorization: {expected_key}")),
            "{workspace:?}: wrong selected key"
        );
    }
}

#[test]
fn fake_keyring_miss_and_empty_sourced_workspace_follow_selection_contract() {
    let store = metadata_credentials(vec![
        LookupReply {
            workspace: "first".to_owned(),
            result: LookupResult::Hit(ConfigSecret::new("lin_api_fake_first".to_owned())),
        },
        LookupReply {
            workspace: "second".to_owned(),
            result: LookupResult::Miss,
        },
    ]);
    let options = options(&[], Some("workspace=''"));
    let transport = prepare_transport(&options, &store, None, &transport_env(&[]))
        .expect("empty sourced workspace falls back to default");
    assert_eq!(transport.endpoint().origin(), "https://api.linear.app");

    let error = prepare_transport(&options, &store, Some("second"), &transport_env(&[]))
        .expect_err("explicit missing key cannot fall back");
    assert_eq!(
        error.display_message(),
        "Failed to get user info: Workspace \"second\" not found in credentials. Run `linear auth login` to add it, or `linear auth list` to see configured workspaces."
    );
    let no_default = credentials("second=''\n");
    let no_key = prepare_transport(&options, &no_default, None, &transport_env(&[]))
        .expect_err("empty configured key is absent");
    assert!(no_key.display_message().contains("No API key configured"));
}

#[test]
fn transport_failures_keep_command_context_and_one_request() {
    for (name, status, body, delay, deadline, cap, kind) in [
        (
            "graphql-error",
            200,
            br#"{"data":null,"errors":[{"message":"Fake identity is unavailable"}]}"#.to_vec(),
            Duration::ZERO,
            Duration::from_secs(2),
            1024,
            AppErrorKind::GraphQl,
        ),
        (
            "http-error",
            401,
            br#"{"data":null}"#.to_vec(),
            Duration::ZERO,
            Duration::from_secs(2),
            1024,
            AppErrorKind::Transport,
        ),
        (
            "malformed-json",
            200,
            b"{".to_vec(),
            Duration::ZERO,
            Duration::from_secs(2),
            1024,
            AppErrorKind::Transport,
        ),
        (
            "wrong-shape",
            200,
            br#"{"data":{"viewer":{"id":"user-fake-1"}}}"#.to_vec(),
            Duration::ZERO,
            Duration::from_secs(2),
            1024,
            AppErrorKind::Invariant,
        ),
        (
            "response-cap",
            200,
            vec![b'x'; 256],
            Duration::ZERO,
            Duration::from_secs(2),
            64,
            AppErrorKind::Transport,
        ),
        (
            "deadline",
            200,
            b"{}".to_vec(),
            Duration::from_millis(300),
            Duration::from_millis(100),
            1024,
            AppErrorKind::Transport,
        ),
    ] {
        let (endpoint, server) = serve_response(status, body, delay);
        let transport = transport_for(&endpoint, deadline, cap);
        let error = block_on_network(async move { run(&transport).await }).expect_err(name);
        assert_eq!(error.kind, kind, "{name}");
        assert!(
            error
                .display_message()
                .starts_with("Failed to get user info: "),
            "{name}"
        );
        assert!(
            !error.display_message().contains("lin_api_fake"),
            "{name}: secret leak"
        );
        let request = server.join().expect("server joined cleanly");
        assert!(
            request.contains("\r\n\r\n"),
            "{name}: exactly one complete request"
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn public_binary_selects_fake_keyring_default_and_explicit_workspace() {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let sequence = NEXT.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "linear-c001-keyring-{}-{sequence}",
        std::process::id()
    ));
    let home = root.join("home");
    let bin = root.join("bin");
    let cwd = root.join("cwd");
    fs::create_dir_all(home.join("linear")).expect("private config home");
    fs::create_dir_all(&bin).expect("private bin");
    fs::create_dir_all(&cwd).expect("private cwd");
    let credential_path = home.join("linear/credentials.toml");
    let credential_bytes = b"default='first'\nworkspaces=['first','second']\n";
    fs::write(&credential_path, credential_bytes).expect("synthetic metadata");
    let tool_path = bin.join("secret-tool");
    fs::write(
        &tool_path,
        b"#!/bin/sh\n[ \"$1 $2 $3 $4\" = 'lookup service linear-cli account' ] || exit 21\nprintf '%s\\n' \"$5\" >> \"$C001_TRACE\"\ncase \"$5\" in\n  first) printf 'lin_api_fake_first\\n';;\n  second) printf 'lin_api_fake_second\\n';;\n  *) exit 1;;\nesac\n",
    )
    .expect("synthetic secret-tool");
    fs::set_permissions(&tool_path, fs::Permissions::from_mode(0o700)).expect("executable");
    let trace = root.join("lookup-trace");

    for (args, expected_key) in [
        (vec!["auth", "whoami"], "lin_api_fake_first"),
        (
            vec!["auth", "whoami", "--workspace", "second"],
            "lin_api_fake_second",
        ),
    ] {
        fs::write(&trace, b"").expect("clear trace");
        let (endpoint, server) = serve_auth_status();
        let output = Command::new(env!("CARGO_BIN_EXE_linear"))
            .args(&args)
            .env_clear()
            .current_dir(&cwd)
            .env("HOME", &home)
            .env("XDG_CONFIG_HOME", &home)
            .env("PATH", &bin)
            .env("C001_TRACE", &trace)
            .env("LINEAR_IGNORE_ENV_FILE", "1")
            .env("LINEAR_GRAPHQL_ENDPOINT", endpoint)
            .env("NO_COLOR", "1")
            .output()
            .expect("public binary");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{args:?}: {:?}",
            output.stderr
        );
        assert!(output.stderr.is_empty(), "{args:?}");
        assert!(
            output.stdout.starts_with(b"Workspace: Example Workspace\n"),
            "{args:?}"
        );
        let request = server.join().expect("one GraphQL request");
        assert!(
            request
                .to_ascii_lowercase()
                .contains(&format!("authorization: {expected_key}")),
            "{args:?}: selected wrong key"
        );
        let mut calls = fs::read_to_string(&trace)
            .expect("lookup trace")
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        calls.sort();
        assert_eq!(calls, ["first", "second"], "{args:?}: eager lookup");
        assert_eq!(
            fs::read(&credential_path).expect("credential file"),
            credential_bytes
        );
    }
    fs::remove_dir_all(&root).expect("remove private sandbox");
}

#[cfg(target_os = "linux")]
fn run_public_binary(args: &[&str], vars: &[(&str, &str)]) -> Output {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let sequence = NEXT.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "linear-c001-public-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir(&root).expect("private cwd");
    let mut command = Command::new(env!("CARGO_BIN_EXE_linear"));
    command
        .args(args)
        .env_clear()
        .current_dir(&root)
        .env("HOME", &root)
        .env("XDG_CONFIG_HOME", &root)
        .env("PATH", &root)
        .env("NO_COLOR", "1")
        .env("LINEAR_IGNORE_ENV_FILE", "1");
    for (name, value) in vars {
        command.env(name, value);
    }
    let output = command.output().expect("public binary");
    fs::remove_dir(&root).expect("private cwd remains empty");
    output
}

#[cfg(target_os = "linux")]
#[test]
fn public_binary_debug_errors_are_exact_and_do_not_echo_the_key() {
    const NO_KEY_ERROR: &str = "✗ Failed to get user info: No API key configured. Set LINEAR_API_KEY, add api_key to .linear.toml, or run `linear auth login`.\n";
    const GRAPHQL_ERROR: &str = "✗ Failed to get user info: Invalid API key\n  debug: GraphQL HTTP 200 OK; errors=1; partial_data=false\n";
    for debug in ["1", "true"] {
        let no_key = run_public_binary(&["auth", "whoami"], &[("LINEAR_DEBUG", debug)]);
        assert_eq!(no_key.status.code(), Some(1));
        assert!(no_key.stdout.is_empty());
        assert_eq!(no_key.stderr, NO_KEY_ERROR.as_bytes());

        let (endpoint, server) = serve_response(
            200,
            br#"{"errors":[{"message":"Invalid API key"}]}"#.to_vec(),
            Duration::ZERO,
        );
        let failed = run_public_binary(
            &["auth", "whoami"],
            &[
                ("LINEAR_DEBUG", debug),
                ("LINEAR_API_KEY", "lin_api_fake"),
                ("LINEAR_GRAPHQL_ENDPOINT", &endpoint),
            ],
        );
        assert_eq!(failed.status.code(), Some(1));
        assert!(failed.stdout.is_empty());
        assert_eq!(failed.stderr, GRAPHQL_ERROR.as_bytes());
        let request = server.join().expect("one GraphQL request");
        assert!(request.contains("query AuthStatus"));
        let stderr = String::from_utf8(failed.stderr).expect("UTF-8 error");
        assert!(!stderr.contains("lin_api_fake"));
        assert!(!stderr.to_ascii_lowercase().contains("authorization"));
    }
}

#[cfg(target_os = "linux")]
#[test]
fn public_binary_rejects_whoami_options_and_arguments_before_network() {
    for args in [
        vec!["auth", "whoami", "--json"],
        vec!["auth", "whoami", "extra"],
    ] {
        let output = run_public_binary(
            &args,
            &[
                ("LINEAR_API_KEY", "lin_api_fake"),
                ("LINEAR_GRAPHQL_ENDPOINT", "http://127.0.0.1:1/graphql"),
            ],
        );
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        let native = linear_cli::cli::command()
            .try_get_matches_from(std::iter::once("linear").chain(args.iter().copied()))
            .unwrap_err();
        assert_eq!(
            output.stderr,
            native.render().to_string().as_bytes(),
            "{args:?}"
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn public_binary_raw_key_conflicts_with_workspace_after_route() {
    let output = run_public_binary(
        &["auth", "whoami", "--workspace", "acme"],
        &[
            ("LINEAR_API_KEY", "lin_api_fake"),
            ("LINEAR_GRAPHQL_ENDPOINT", "http://127.0.0.1:1/graphql"),
        ],
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"\xe2\x9c\x97 Failed to get user info: Cannot use --workspace flag when LINEAR_API_KEY environment variable is set. Either unset LINEAR_API_KEY or remove the --workspace flag.\n"
    );
}
