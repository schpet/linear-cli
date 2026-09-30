use linear_cli::app::block_on_network;
use linear_cli::auth::{
    CredentialStore, LookupFailureCategory, LookupReply, LookupResult, hydrate, parse_credentials,
};
use linear_cli::commands::auth_list::{
    EMPTY_OUTPUT, Outcome, Prepared, Row, RowError, StoredKey, classify, fetch, fetch_with,
    prepare_transports, prepare_with, render,
};
use linear_cli::commands::display::display_width;
use linear_cli::config::{
    ConfigSecret, OsFamily, ProcessEnvSnapshot, RawConfigFile, TransportEnvInputs,
    parse_config_tier,
};
use linear_cli::error::{AppError, AppErrorKind};
use linear_cli::graphql::operations::auth_list::AuthListViewer;
use linear_cli::graphql::transport::{
    ApiKey, CaMode, Deadline, EndpointUrl, GraphQlTransport, ProxyMode, ResponseCap,
    TransportConfig, TransportFailure,
};
use serde_json::json;
use std::cell::Cell;
use std::ffi::OsString;
#[cfg(target_os = "linux")]
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
#[cfg(target_os = "linux")]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
#[cfg(target_os = "linux")]
use std::process::{Command, Output};
#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const VIEWER: &str = r#"{"data":{"viewer":{"name":"Olive","email":"ok@example.invalid","organization":{"name":"Okay Org","urlKey":"okay"}}}}"#;

fn store(text: &str) -> CredentialStore {
    let tier = parse_config_tier(RawConfigFile {
        path: PathBuf::from("/fake/credentials.toml"),
        bytes: text.as_bytes().to_vec(),
    })
    .expect("credentials TOML");
    hydrate(parse_credentials(tier).expect("manifest"), vec![]).expect("inline store")
}

fn metadata_store(replies: Vec<LookupReply>) -> CredentialStore {
    let tier = parse_config_tier(RawConfigFile {
        path: PathBuf::from("/fake/credentials.toml"),
        bytes: b"default='miss'\nworkspaces=['hit','miss','failed']\n".to_vec(),
    })
    .expect("metadata TOML");
    hydrate(parse_credentials(tier).expect("manifest"), replies).expect("fake lookup table")
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

fn viewer(organization: &str, name: &str, email: &str) -> Outcome {
    Outcome::Viewer {
        organization: organization.to_owned(),
        name: name.to_owned(),
        email: email.to_owned(),
    }
}

fn row(workspace: &str, is_default: bool, state: Outcome) -> Row<Outcome> {
    Row {
        workspace: workspace.to_owned(),
        is_default,
        state,
    }
}

fn labels<S>(rows: &[Row<S>]) -> Vec<(&str, bool)> {
    rows.iter()
        .map(|row| (row.workspace.as_str(), row.is_default))
        .collect()
}

#[test]
fn empty_store_output_is_two_exact_lines() {
    assert_eq!(
        EMPTY_OUTPUT,
        "No workspaces configured\nRun `linear auth login` to add a workspace\n"
    );
    assert_eq!(render(&[], false), EMPTY_OUTPUT.as_bytes());
    assert!(classify(&store("")).is_empty());
}

#[test]
fn table_pads_columns_marks_default_and_keeps_error_row_trailing_spaces() {
    let rows = [
        row(
            "zeta",
            true,
            viewer("Org ok1", "Nok1", "ok1@example.invalid"),
        ),
        row("alpha", false, Outcome::Error(RowError::InvalidCredentials)),
        row(
            "a-long-workspace",
            false,
            Outcome::Error(RowError::UnusableKey),
        ),
    ];
    // The shorter error cell keeps its four trailing pad spaces.
    assert_eq!(
        String::from_utf8(render(&rows, false)).expect("UTF-8 table"),
        concat!(
            "  WORKSPACE        ORG NAME            USER\n",
            "* zeta             Org ok1             Nok1 <ok1@example.invalid>\n",
            "  alpha            invalid credentials\n",
            "  a-long-workspace invalid API key    \n",
        )
    );
    // Frozen Deno console `%c` bytes on a color stdout terminal, with the
    // trailing pad inside the red cell.
    assert_eq!(
        String::from_utf8(render(&rows, true)).expect("UTF-8 table"),
        concat!(
            "\x1b[4m  WORKSPACE        ORG NAME            USER\x1b[0m\n",
            "* zeta             Org ok1             Nok1 <ok1@example.invalid>\n",
            "  alpha            \x1b[31minvalid credentials\x1b[39m\x1b[0m\n",
            "  a-long-workspace \x1b[31minvalid API key    \x1b[39m\x1b[0m\n",
        )
    );
    let short = [row("a", false, Outcome::Error(RowError::UnusableKey))];
    assert_eq!(
        render(&short, false),
        b"  WORKSPACE ORG NAME        USER\n  a         invalid API key\n"
    );
}

#[test]
fn width_sums_code_points_with_controls_and_joiners_as_zero() {
    for (text, width) in [
        ("WORKSPACE", 9),
        ("天地", 4),
        ("e\u{301}", 1),
        ("\u{2764}\u{fe0f}", 1),
        ("\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}", 6),
        ("\u{fefb}", 1),
        ("\u{644}\u{627}", 2),
        ("a\tb\x1bc", 3),
        // unicode-width 0.2.2 is newer than Deno's Unicode 15 table here;
        // the reviewed C002-WIDTH-TABLE golden binds the difference.
        ("\u{4dc0}", 2),
    ] {
        assert_eq!(display_width(text), width, "{text:?}");
    }
    let rows = [row(
        "天地",
        false,
        viewer("e\u{301}", "N", "n@example.invalid"),
    )];
    assert_eq!(
        String::from_utf8(render(&rows, false)).expect("UTF-8 table"),
        "  WORKSPACE ORG NAME USER\n  天地      e\u{301}        N <n@example.invalid>\n"
    );
}

#[test]
fn classify_keeps_store_order_and_ignores_raw_keys_and_selection() {
    let rows = classify(&store(
        "default='beta'\nzeta='lin_api_fake_z'\nbeta='lin_api_fake_b'\nempty=''\nlatin='lin_api_fak\u{e9}'\n",
    ));
    assert_eq!(
        labels(&rows),
        [
            ("zeta", false),
            ("beta", true),
            ("empty", false),
            ("latin", false)
        ]
    );
    assert!(matches!(rows[0].state, StoredKey::Usable(_)));
    assert!(matches!(rows[1].state, StoredKey::Usable(_)));
    assert!(matches!(rows[2].state, StoredKey::Unusable));
    assert!(matches!(rows[3].state, StoredKey::Unusable));
    assert!(!format!("{rows:?}").contains("lin_api_fake"));

    let rows = classify(&store("default='ghost'\nsolo='lin_api_fake_s'\n"));
    assert_eq!(labels(&rows), [("solo", false)]);
}

#[test]
fn keyring_miss_and_failure_become_missing_credentials_rows() {
    let rows = classify(&metadata_store(vec![
        LookupReply {
            workspace: "hit".to_owned(),
            result: LookupResult::Hit(ConfigSecret::new("lin_api_fake_hit".to_owned())),
        },
        LookupReply {
            workspace: "miss".to_owned(),
            result: LookupResult::Miss,
        },
        LookupReply {
            workspace: "failed".to_owned(),
            result: LookupResult::Failed(LookupFailureCategory::Unavailable),
        },
    ]));
    assert_eq!(
        labels(&rows),
        [("hit", false), ("miss", true), ("failed", false)]
    );
    assert!(matches!(rows[0].state, StoredKey::Usable(_)));
    assert!(matches!(rows[1].state, StoredKey::Missing));
    assert!(matches!(rows[2].state, StoredKey::Missing));
}

#[test]
fn transport_policy_is_resolved_only_when_some_key_is_usable() {
    let strict = transport_env(&[("HTTP_PROXY", "http://127.0.0.1:9000")]);
    let endpoint = EndpointUrl::parse("http://127.0.0.1:1/graphql").expect("endpoint");
    let unusable = classify(&store("empty=''\nlatin='lin_api_fak\u{e9}'\n"));
    let prepared = prepare_transports(unusable, &endpoint, &strict)
        .expect("no usable key means no policy resolution");
    assert!(prepared.iter().all(|row| matches!(
        row.state,
        Prepared::Done(Outcome::Error(RowError::UnusableKey))
    )));

    let mixed = classify(&store("empty=''\nok='lin_api_fake_ok'\n"));
    let error =
        prepare_transports(mixed, &endpoint, &strict).expect_err("strict proxy policy is fatal");
    assert_eq!(error.kind, AppErrorKind::Validation);
    assert!(error.display_message().starts_with("HTTP_PROXY"));
}

#[test]
fn injected_second_build_failure_is_fatal_before_any_request() {
    let rows = classify(&store(
        "first='lin_api_fake_1'\nmissing=''\nsecond='lin_api_fake_2'\nthird='lin_api_fake_3'\n",
    ));
    let builds = Cell::new(0);
    let error = prepare_with(rows, |_key: ApiKey| {
        builds.set(builds.get() + 1);
        if builds.get() == 2 {
            Err(AppError::new(
                AppErrorKind::Transport,
                "second build failed",
            ))
        } else {
            Ok(builds.get())
        }
    })
    .expect_err("second build aborts preparation");
    assert_eq!(error.display_message(), "second build failed");
    assert_eq!(builds.get(), 2, "no build after the failed one");
    // Preparation owns no request; `fetch_with` is the only request start.
}

fn fixture(name: &str) -> AuthListViewer {
    serde_json::from_value(json!({"viewer": {
        "name": name, "email": format!("{name}@example.invalid"),
        "organization": {"name": format!("{name} Org"), "urlKey": name}}}))
    .expect("schema-valid fixture")
}

fn request_row(workspace: &str) -> Row<Prepared<&'static str>> {
    Row {
        workspace: workspace.to_owned(),
        is_default: false,
        state: Prepared::Request(if workspace == "slow" { "slow" } else { "fast" }),
    }
}

#[tokio::test]
async fn fetch_places_out_of_order_completions_in_source_order() {
    let rows = vec![
        request_row("slow"),
        Row {
            workspace: "missing".to_owned(),
            is_default: true,
            state: Prepared::Done(Outcome::Error(RowError::MissingCredentials)),
        },
        request_row("fast"),
    ];
    let (fast_done, slow_wait) = tokio::sync::oneshot::channel::<()>();
    let fast_done = std::sync::Mutex::new(Some(fast_done));
    let slow_wait = std::sync::Mutex::new(Some(slow_wait));
    let listed = fetch_with(rows, |handle, request| {
        assert_eq!(request.operation_name.as_deref(), Some("AuthListViewer"));
        assert!(request.variables.is_none());
        let wait = (handle == "slow").then(|| slow_wait.lock().expect("lock").take());
        let done = (handle == "fast").then(|| fast_done.lock().expect("lock").take());
        async move {
            if let Some(wait) = wait.flatten() {
                // Bounded so a serial implementation fails instead of hanging.
                tokio::time::timeout(Duration::from_secs(5), wait)
                    .await
                    .expect("fast row runs while the slow row waits")
                    .expect("fast row completed first");
                Ok::<_, TransportFailure>(fixture("Slow"))
            } else {
                done.flatten()
                    .expect("one fast row")
                    .send(())
                    .expect("slow row waits");
                Ok(fixture("Fast"))
            }
        }
    })
    .await
    .expect("listed rows");
    assert_eq!(
        labels(&listed),
        [("slow", false), ("missing", true), ("fast", false)]
    );
    assert_eq!(
        listed[0].state,
        viewer("Slow Org", "Slow", "Slow@example.invalid")
    );
    assert_eq!(
        listed[1].state,
        Outcome::Error(RowError::MissingCredentials)
    );
    assert_eq!(
        listed[2].state,
        viewer("Fast Org", "Fast", "Fast@example.invalid")
    );
}

#[tokio::test]
async fn a_panicking_request_task_is_a_fatal_invariant() {
    let error = fetch_with(vec![request_row("only")], |handle, _request| async move {
        if handle == "fast" {
            panic!("synthetic task panic");
        }
        Ok::<_, TransportFailure>(fixture("unreachable"))
    })
    .await
    .expect_err("task panic is fatal");
    assert_eq!(error.kind, AppErrorKind::Invariant);
    assert_eq!(error.display_message(), "a workspace request task failed");
}

/// Read one complete HTTP/1.1 request (headers plus Content-Length body).
fn read_request(stream: &mut TcpStream) -> String {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("timeout");
    let mut request = Vec::new();
    let mut chunk = [0_u8; 4096];
    let (head_end, content_length) = loop {
        let count = stream.read(&mut chunk).expect("request bytes");
        assert!(count > 0, "request closed before headers");
        request.extend_from_slice(&chunk[..count]);
        assert!(request.len() < 64 * 1024, "request is bounded");
        if let Some(offset) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
            let headers = String::from_utf8(request[..offset].to_vec()).expect("headers");
            let length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().expect("length"))
                })
                .expect("content length");
            break (offset + 4, length);
        }
    };
    while request.len() - head_end < content_length {
        let count = stream.read(&mut chunk).expect("body bytes");
        assert!(count > 0, "request closed before body");
        request.extend_from_slice(&chunk[..count]);
    }
    String::from_utf8(request).expect("request UTF-8")
}

fn respond(stream: &mut TcpStream, status: &str, extra: &str, body: &[u8]) {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
}

fn authorization(request: &str) -> String {
    request
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("authorization")
                .then(|| value.trim().to_owned())
        })
        .expect("Authorization header")
}

/// Serve `count` connections; `reply` picks a response from the request.
fn serve(
    count: usize,
    reply: fn(&str) -> (&'static str, &'static str, Vec<u8>),
) -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("private listener");
    let endpoint = format!("http://{}/graphql", listener.local_addr().expect("address"));
    let server = thread::spawn(move || {
        (0..count)
            .map(|_| {
                let (mut stream, _) = listener.accept().expect("request");
                let request = read_request(&mut stream);
                let (status, extra, body) = reply(&request);
                respond(&mut stream, status, extra, &body);
                request
            })
            .collect()
    });
    (endpoint, server)
}

fn transport(endpoint: &str, key: &str, cap: usize) -> GraphQlTransport {
    GraphQlTransport::new(
        EndpointUrl::parse(endpoint).expect("endpoint"),
        ApiKey::new(key.to_owned()).expect("fake key"),
        TransportConfig {
            proxy: ProxyMode::Direct,
            ca: CaMode::PublicRoots,
            deadline: Deadline::new(Duration::from_secs(5)).expect("deadline"),
            max_response_bytes: ResponseCap::new(cap).expect("cap"),
        },
    )
    .expect("test transport")
}

#[test]
fn loopback_failures_stay_row_local_with_short_stable_cells() {
    let (endpoint, server) = serve(9, |request| {
        match authorization(request).as_str() {
        "lin_api_fake_ok" => ("200 OK", "Content-Type: application/json\r\n", VIEWER.into()),
        "lin_api_fake_401_text" => ("401 Unauthorized", "", b"nope".to_vec()),
        "lin_api_fake_403_graphql" => (
            "403 Forbidden",
            "Content-Type: application/json\r\n",
            br#"{"errors":[{"message":"forbidden"}]}"#.to_vec(),
        ),
        "lin_api_fake_401_large" => ("401 Unauthorized", "", vec![b'x'; 512]),
        "lin_api_fake_graphql" => (
            "200 OK",
            "Content-Type: application/json\r\n",
            br#"{"data":null,"errors":[{"message":"Rate limited","extensions":{"code":"RATELIMITED","userPresentableMessage":"Too many requests"}}]}"#.to_vec(),
        ),
        "lin_api_fake_malformed" => ("200 OK", "", b"not json".to_vec()),
        "lin_api_fake_null" => (
            "200 OK",
            "Content-Type: application/json\r\n",
            br#"{"data":{"viewer":null}}"#.to_vec(),
        ),
        "lin_api_fake_500" => ("500 Internal Server Error", "", b"{}".to_vec()),
        "lin_api_fake_redirect" => (
            "307 Temporary Redirect",
            "Location: http://127.0.0.1:1/graphql\r\n",
            Vec::new(),
        ),
        other => panic!("unexpected key {other}"),
    }
    });
    let names = [
        "ok",
        "401_text",
        "403_graphql",
        "401_large",
        "graphql",
        "malformed",
        "null",
        "500",
        "redirect",
    ];
    let rows = names
        .iter()
        .map(|name| Row {
            workspace: (*name).to_owned(),
            is_default: false,
            state: Prepared::Request(transport(&endpoint, &format!("lin_api_fake_{name}"), 256)),
        })
        .collect::<Vec<_>>();
    let listed = block_on_network(fetch(rows)).expect("row failures are not fatal");
    let requests = server.join().expect("server");
    assert_eq!(requests.len(), names.len(), "one request per usable key");
    for request in &requests {
        let body = request.split_once("\r\n\r\n").expect("HTTP").1;
        let envelope: serde_json::Value = serde_json::from_str(body).expect("JSON body");
        assert_eq!(envelope["operationName"], "AuthListViewer");
        assert!(envelope.get("variables").is_none());
        assert!(
            request
                .to_ascii_lowercase()
                .contains("user-agent: schpet-linear-cli/3.0.0-alpha.1")
        );
    }
    let cells = listed
        .iter()
        .map(|row| match &row.state {
            Outcome::Viewer { organization, .. } => format!("ok:{organization}"),
            Outcome::Error(RowError::Failure(text)) => text.clone(),
            Outcome::Error(error) => format!("{error:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        cells,
        [
            "ok:Okay Org",
            "InvalidCredentials",
            "InvalidCredentials",
            "InvalidCredentials",
            "Too many requests",
            "response body is not valid JSON",
            "response did not match the expected viewer shape",
            "unexpected HTTP status 500 Internal Server Error",
            "unexpected HTTP status 307 Temporary Redirect",
        ]
    );
}

#[test]
fn refused_endpoint_is_a_row_error_without_secret() {
    let rows = vec![Row {
        workspace: "refused".to_owned(),
        is_default: true,
        state: Prepared::Request(transport(
            "http://127.0.0.1:1/graphql",
            "lin_api_fake_refused",
            256,
        )),
    }];
    let listed = block_on_network(fetch(rows)).expect("refusal is row-local");
    let Outcome::Error(error) = &listed[0].state else {
        panic!("refused row");
    };
    assert_eq!(
        error,
        &RowError::Failure(
            "connection to http://127.0.0.1:1 failed: Connection refused (os error 111)".to_owned()
        )
    );
    let output = render(&listed, false);
    assert!(
        !String::from_utf8(output)
            .expect("UTF-8")
            .contains("lin_api")
    );
}

#[test]
fn two_real_requests_overlap_before_either_response() {
    // Each server thread holds its response until the other request has
    // arrived; serial requests would time out on the first connection.
    let (first_seen, first_wait) = mpsc::channel::<()>();
    let (second_seen, second_wait) = mpsc::channel::<()>();
    let mut endpoints = Vec::new();
    let mut servers = Vec::new();
    for (seen, wait) in [(first_seen, second_wait), (second_seen, first_wait)] {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        endpoints.push(format!(
            "http://{}/graphql",
            listener.local_addr().expect("addr")
        ));
        servers.push(thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("request");
            let request = read_request(&mut stream);
            seen.send(()).expect("peer alive");
            wait.recv_timeout(Duration::from_secs(3))
                .expect("requests overlap");
            respond(
                &mut stream,
                "200 OK",
                "Content-Type: application/json\r\n",
                VIEWER.as_bytes(),
            );
            authorization(&request)
        }));
    }
    let rows = endpoints
        .iter()
        .zip(["lin_api_fake_a", "lin_api_fake_b"])
        .map(|(endpoint, key)| Row {
            workspace: key.to_owned(),
            is_default: false,
            state: Prepared::Request(transport(endpoint, key, 1024)),
        })
        .collect::<Vec<_>>();
    let listed = block_on_network(fetch(rows)).expect("both rows");
    assert!(
        listed
            .iter()
            .all(|row| row.state == viewer("Okay Org", "Olive", "ok@example.invalid"))
    );
    let keys = servers
        .into_iter()
        .map(|server| server.join().expect("server"))
        .collect::<Vec<_>>();
    assert_eq!(keys, ["lin_api_fake_a", "lin_api_fake_b"]);
}

#[cfg(target_os = "linux")]
struct Sandbox {
    root: PathBuf,
}

#[cfg(target_os = "linux")]
impl Sandbox {
    fn new(credentials: Option<&[u8]>) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "linear-c002-public-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("home/linear")).expect("config home");
        fs::create_dir_all(root.join("bin")).expect("bin");
        fs::create_dir_all(root.join("cwd")).expect("cwd");
        if let Some(bytes) = credentials {
            fs::write(root.join("home/linear/credentials.toml"), bytes).expect("credentials");
        }
        Self { root }
    }

    fn run(&self, args: &[&str], vars: &[(&str, &str)]) -> Output {
        let home = self.root.join("home");
        let mut command = Command::new(env!("CARGO_BIN_EXE_linear"));
        command
            .args(args)
            .env_clear()
            .current_dir(self.root.join("cwd"))
            .env("HOME", &home)
            .env("XDG_CONFIG_HOME", &home)
            .env("PATH", self.root.join("bin"))
            .env("LINEAR_IGNORE_ENV_FILE", "1");
        for (name, value) in vars {
            command.env(name, value);
        }
        command.output().expect("public binary")
    }

    fn credentials(&self) -> Vec<u8> {
        fs::read(self.root.join("home/linear/credentials.toml")).expect("credentials")
    }
}

#[cfg(target_os = "linux")]
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[cfg(target_os = "linux")]
#[test]
fn public_binary_empty_store_prints_guidance_without_network_or_policy() {
    let sandbox = Sandbox::new(None);
    let output = sandbox.run(
        &["auth", "list"],
        &[
            ("HTTP_PROXY", "http://127.0.0.1:9000"),
            ("LINEAR_API_KEY", "lin_api_fake_raw"),
            ("LINEAR_GRAPHQL_ENDPOINT", "http://127.0.0.1:1/graphql"),
        ],
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, EMPTY_OUTPUT.as_bytes());
    assert!(output.stderr.is_empty());
}

#[cfg(target_os = "linux")]
#[test]
fn public_binary_lists_every_workspace_in_store_order_with_workspace_flags() {
    let bytes =
        b"default='alpha'\nzeta='lin_api_fake_zeta'\nalpha='lin_api_fake_alpha'\nempty=''\n";
    let sandbox = Sandbox::new(Some(bytes));
    for args in [
        vec!["auth", "list"],
        vec!["auth", "list", "--workspace", "zeta"],
        vec!["--workspace", "unknown", "auth", "list"],
    ] {
        let (endpoint, server) = serve(2, |request| {
            match authorization(request).as_str() {
            "lin_api_fake_zeta" => (
                "200 OK",
                "Content-Type: application/json\r\n",
                br#"{"data":{"viewer":{"name":"Zed","email":"z@example.invalid","organization":{"name":"Zeta Org","urlKey":"z"}}}}"#.to_vec(),
            ),
            "lin_api_fake_alpha" => ("401 Unauthorized", "", b"denied".to_vec()),
            other => panic!("unexpected key {other}"),
        }
        });
        let output = sandbox.run(
            &args,
            &[
                ("LINEAR_API_KEY", "lin_api_fake_raw"),
                ("LINEAR_GRAPHQL_ENDPOINT", &endpoint),
                ("NO_COLOR", "1"),
            ],
        );
        assert_eq!(
            output.status.code(),
            Some(0),
            "{args:?}: {:?}",
            output.stderr
        );
        assert!(output.stderr.is_empty(), "{args:?}");
        assert_eq!(
            String::from_utf8(output.stdout).expect("UTF-8"),
            concat!(
                "  WORKSPACE ORG NAME            USER\n",
                "  zeta      Zeta Org            Zed <z@example.invalid>\n",
                "* alpha     invalid credentials\n",
                "  empty     invalid API key    \n",
            ),
            "{args:?}"
        );
        let mut keys = server
            .join()
            .expect("server")
            .iter()
            .map(|request| authorization(request))
            .collect::<Vec<_>>();
        keys.sort();
        assert_eq!(
            keys,
            ["lin_api_fake_alpha", "lin_api_fake_zeta"],
            "{args:?}"
        );
        assert_eq!(sandbox.credentials(), bytes);
    }
}

#[cfg(target_os = "linux")]
#[test]
fn public_binary_policy_failure_is_fatal_before_any_request() {
    let sandbox = Sandbox::new(Some(b"one='lin_api_fake_1'\ntwo='lin_api_fake_2'\n"));
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    listener.set_nonblocking(true).expect("nonblocking");
    let endpoint = format!("http://{}/graphql", listener.local_addr().expect("addr"));
    for (vars, stderr) in [
        (
            vec![("HTTP_PROXY", "http://127.0.0.1:9000")],
            "✗ Failed to list workspaces: HTTP_PROXY is not supported by this transport mode\n  Use direct public roots, an absolute SSL_CERT_FILE, or the documented loopback HTTPS proxy mode.\n",
        ),
        (
            vec![("SSL_CERT_FILE", "/nonexistent/c002-ca.pem")],
            "✗ Failed to list workspaces: SSL_CERT_FILE: CA bundle /nonexistent/c002-ca.pem could not be read\n",
        ),
    ] {
        let mut all = vars.clone();
        all.push(("LINEAR_GRAPHQL_ENDPOINT", &endpoint));
        let output = sandbox.run(&["auth", "list"], &all);
        assert_eq!(output.status.code(), Some(1), "{vars:?}");
        assert!(output.stdout.is_empty(), "{vars:?}");
        assert_eq!(
            String::from_utf8(output.stderr).expect("UTF-8"),
            stderr,
            "{vars:?}"
        );
        assert!(
            matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock),
            "{vars:?}: no request may start"
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn public_binary_unusable_keys_are_rows_even_under_strict_policy() {
    let sandbox = Sandbox::new(Some("empty=''\nlatin='lin_api_fak\u{e9}'\n".as_bytes()));
    let output = sandbox.run(
        &["auth", "list"],
        &[("HTTP_PROXY", "http://127.0.0.1:9000"), ("NO_COLOR", "1")],
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(
        output.stdout,
        b"  WORKSPACE ORG NAME        USER\n  empty     invalid API key\n  latin     invalid API key\n"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn public_binary_keyring_miss_and_failure_render_missing_rows_after_warning() {
    let sandbox = Sandbox::new(Some(b"default='hit'\nworkspaces=['hit','miss','failed']\n"));
    let tool = sandbox.root.join("bin/secret-tool");
    fs::write(
        &tool,
        b"#!/bin/sh\n[ \"$1 $2 $3 $4\" = 'lookup service linear-cli account' ] || exit 21\ncase \"$5\" in\n  hit) printf 'lin_api_fake_hit\\n';;\n  miss) exit 1;;\n  *) printf 'denied\\n' >&2; exit 5;;\nesac\n",
    )
    .expect("fake secret-tool");
    fs::set_permissions(&tool, fs::Permissions::from_mode(0o700)).expect("executable");
    let (endpoint, server) = serve(1, |_| {
        (
            "200 OK",
            "Content-Type: application/json\r\n",
            VIEWER.into(),
        )
    });
    let output = sandbox.run(
        &["auth", "list"],
        &[("LINEAR_GRAPHQL_ENDPOINT", &endpoint), ("NO_COLOR", "1")],
    );
    assert_eq!(output.status.code(), Some(0), "{:?}", output.stderr);
    let stderr = String::from_utf8(output.stderr).expect("UTF-8");
    assert!(
        stderr.contains("\"miss\"") && stderr.contains("\"failed\""),
        "{stderr}"
    );
    assert!(!stderr.contains("denied"), "child stderr is not echoed");
    assert_eq!(
        String::from_utf8(output.stdout).expect("UTF-8"),
        concat!(
            "  WORKSPACE ORG NAME            USER\n",
            "* hit       Okay Org            Olive <ok@example.invalid>\n",
            "  miss      missing credentials\n",
            "  failed    missing credentials\n",
        )
    );
    let requests = server.join().expect("one request");
    assert_eq!(authorization(&requests[0]), "lin_api_fake_hit");
}

#[cfg(target_os = "linux")]
#[test]
fn public_binary_rejects_list_options_and_arguments_before_network() {
    let sandbox = Sandbox::new(Some(b"one='lin_api_fake_1'\n"));
    for args in [
        vec!["auth", "list", "--json"],
        vec!["auth", "list", "extra"],
    ] {
        let output = sandbox.run(
            &args,
            &[
                ("LINEAR_GRAPHQL_ENDPOINT", "http://127.0.0.1:1/graphql"),
                ("NO_COLOR", "1"),
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
