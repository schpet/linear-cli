use std::cell::RefCell;
use std::collections::VecDeque;
use std::ffi::OsString;
use std::future::ready;
use std::io::{self, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::thread;
use std::time::{Duration, Instant};

use chrono::{TimeZone, Utc};
use cynic::QueryBuilder;
use linear_cli::app::{self, AppContext};
use linear_cli::auth::file::{CredentialFileSource, CredentialReadFailure};
use linear_cli::auth::keyring::UnsupportedKeyringReader;
use linear_cli::cli::DispatchAction;
use linear_cli::cli::clap_input::{self, Invocation};
use linear_cli::commands::milestone_view::{detail_request, fetch_with, json, markdown};
use linear_cli::config::{
    FileKind, FileSource, GitProbeResult, GitRootProbe, OsFamily, ProcessEnvSnapshot,
};
use linear_cli::error::{AppError, AppErrorKind};
use linear_cli::graphql::envelope::{GraphQlRequest, parse_response};
use linear_cli::graphql::operations::milestone_view::{
    DetailVariables, GetMilestoneDetails, GetProjectMilestonesForLookup, LookupVariables,
};
use linear_cli::startup::{self, AppStartupReport};
use serde_json::{Value, json};

const INPUT: &str = "00000000-0000-4000-8000-000000000001";
const RETURNED: &str = "00000000-0000-4000-8000-000000000099";

struct EmptyFiles;
impl FileSource for EmptyFiles {
    fn kind(&self, _path: &Path) -> io::Result<Option<FileKind>> {
        Ok(None)
    }
    fn read_bounded(&self, _path: &Path, _limit: u64) -> io::Result<Vec<u8>> {
        Err(io::Error::from(io::ErrorKind::NotFound))
    }
}
struct NoGit;
impl GitRootProbe for NoGit {
    fn probe(&self) -> GitProbeResult {
        GitProbeResult::SpawnFailure
    }
}
struct EmptyCredentials;
impl CredentialFileSource for EmptyCredentials {
    fn read_credentials(&self, _path: &Path) -> Result<Option<Vec<u8>>, CredentialReadFailure> {
        Ok(None)
    }
}

fn startup(env: &[(&str, &str)]) -> AppStartupReport {
    let cwd = PathBuf::from("/c031-test");
    let variables = env
        .iter()
        .map(|(name, value)| (OsString::from(name), OsString::from(value)));
    let snapshot =
        ProcessEnvSnapshot::from_vars_os(cwd, OsFamily::Unix, variables).expect("test environment");
    let report = startup::load(
        &snapshot,
        &EmptyFiles,
        &NoGit,
        &EmptyCredentials,
        &UnsupportedKeyringReader,
    );
    assert!(report.result.is_ok(), "test startup");
    report
}

fn invoke(
    args: &[&str],
    env: &[(&str, &str)],
    stdout_tty: bool,
) -> (
    Result<linear_cli::error::ExitStatus, AppError>,
    Vec<u8>,
    Vec<u8>,
) {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut context = AppContext {
        startup: startup(env),
        cwd: PathBuf::from("/c031-test"),
        stdout: &mut stdout,
        stderr: &mut stderr,
        stdin_tty: false,
        stdout_tty,
        stderr_tty: false,
        stdout_finalization: None,
    };
    let status = app::run(
        &args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>(),
        &mut context,
    );
    (status, stdout, stderr)
}

fn serve_once(body: Value) -> (String, thread::JoinHandle<Value>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("fake local GraphQL listener");
    listener
        .set_nonblocking(true)
        .expect("nonblocking listener");
    let endpoint = format!("http://{}/graphql", listener.local_addr().expect("address"));
    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error)
                    if error.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline =>
                {
                    thread::sleep(Duration::from_millis(5))
                }
                Err(error) => panic!("expected one local GraphQL request: {error}"),
            }
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("read timeout");
        let mut bytes = Vec::new();
        let mut chunk = [0_u8; 4096];
        let (head_end, content_length) = loop {
            let count = stream.read(&mut chunk).expect("request bytes");
            assert!(count > 0, "request closed before headers");
            bytes.extend_from_slice(&chunk[..count]);
            assert!(bytes.len() < 64 * 1024, "bounded request");
            if let Some(offset) = bytes.windows(4).position(|item| item == b"\r\n\r\n") {
                let head_end = offset + 4;
                let headers = String::from_utf8(bytes[..head_end].to_vec()).expect("headers");
                let length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().expect("length"))
                    })
                    .expect("content length");
                break (head_end, length);
            }
        };
        while bytes.len() - head_end < content_length {
            let count = stream.read(&mut chunk).expect("body bytes");
            assert!(count > 0, "request closed before body");
            bytes.extend_from_slice(&chunk[..count]);
        }
        let request: Value = serde_json::from_slice(&bytes[head_end..head_end + content_length])
            .expect("request JSON");
        let reply = body.to_string();
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}", reply.len()).expect("fake reply");
        request
    });
    (endpoint, server)
}

fn frozen(id: &str) -> Value {
    let path = format!(
        "{}/../../parity/runner/c031-frozen-cases/{id}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_slice(&std::fs::read(path).expect("frozen case")).expect("case JSON")
}

fn page(id: &str, nodes: Value, next: bool, cursor: Value) -> GetMilestoneDetails {
    parse_response(json!({"data":{"projectMilestone":{
        "id":id,"name":"Launch","description":"Ship the release.",
        "targetDate":"2025-03-01","sortOrder":4,
        "createdAt":"2020-01-01T00:00:00Z","updatedAt":"2020-02-01T00:00:00Z",
        "project":{"id":"project-id","name":"Mobile App","slugId":"abc123def456","url":"https://linear.app/alpha/project/mobile-app-abc123def456"},
        "issues":{"nodes":nodes,"pageInfo":{"hasNextPage":next,"endCursor":cursor}}
    }}}).to_string().as_bytes()).expect("typed page")
}

fn issue(id: &str) -> Value {
    json!({"id":id,"identifier":id,"title":"A task","state":{"name":"Completed","type":"completed"}})
}

type Sent = Rc<RefCell<Vec<Value>>>;
type PageResult = Result<GetMilestoneDetails, AppError>;
type ScriptedFetch =
    Box<dyn FnMut(GraphQlRequest<DetailVariables>) -> std::future::Ready<PageResult>>;

fn scripted(pages: Vec<PageResult>) -> (Sent, ScriptedFetch) {
    let sent = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&sent);
    let queue = Rc::new(RefCell::new(VecDeque::from(pages)));
    (
        sent,
        Box::new(move |request| {
            recorded
                .borrow_mut()
                .push(serde_json::to_value(&request).expect("request"));
            ready(queue.borrow_mut().pop_front().expect("unexpected request"))
        }),
    )
}

#[test]
fn documents_and_variables_match_both_frozen_operations() {
    let detail = detail_request(INPUT, None);
    let expected = frozen("c031-all-two-pages-json")["graphql"]["groups"][0]["steps"][0]["operation"]["document"]
        .as_str().expect("document").to_owned();
    let normalize = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
    assert_eq!(normalize(&detail.query), normalize(&expected));
    assert_eq!(
        serde_json::to_value(detail.variables).expect("variables"),
        json!({"id":INPUT,"first":50})
    );
    let later = detail_request(INPUT, Some("cursor-1".to_owned()));
    assert_eq!(
        serde_json::to_value(later.variables).expect("variables"),
        json!({"id":INPUT,"first":50,"after":"cursor-1"})
    );
    let lookup =
        GraphQlRequest::with_variables(GetProjectMilestonesForLookup::build(LookupVariables {
            project_id: "project-id".to_owned(),
        }));
    let expected_lookup =
        frozen("c031-project-uuid-milestone-name")["graphql"]["groups"][0]["steps"][0]["operation"]
            ["document"]
            .as_str()
            .expect("lookup document")
            .to_owned();
    assert_eq!(normalize(&lookup.query), normalize(&expected_lookup));
    assert_eq!(
        serde_json::to_value(lookup.variables).expect("variables"),
        json!({"projectId":"project-id"})
    );
}

#[tokio::test]
async fn default_mode_never_inspects_or_follows_a_missing_cursor() {
    for cursor in [Value::Null, json!("")] {
        let (sent, fetch) = scripted(vec![Ok(page(
            RETURNED,
            json!([issue("APP-1")]),
            true,
            cursor.clone(),
        ))]);
        let milestone = fetch_with(INPUT, INPUT, false, fetch)
            .await
            .expect("one page");
        assert_eq!(sent.borrow().len(), 1);
        assert!(milestone.issues.page_info.has_next_page);
        assert_eq!(
            milestone.issues.page_info.end_cursor.as_deref(),
            cursor.as_str()
        );
        assert!(
            markdown(
                &milestone,
                false,
                Utc.with_ymd_and_hms(2026, 9, 28, 0, 0, 0).unwrap(),
                &Utc
            )
            .contains("**Issues fetched:** 1 (milestone has more — use `--all` for full counts)")
        );
        assert!(
            String::from_utf8(json(&milestone).expect("json"))
                .expect("utf8")
                .contains("\"hasNextPage\": true")
        );
        let json_output: Value =
            serde_json::from_slice(&json(&milestone).expect("json")).expect("JSON output");
        assert_eq!(json_output["issues"]["pageInfo"]["endCursor"], cursor);
    }
}

#[tokio::test]
async fn all_mode_one_page_and_zero_issues_keep_one_request_and_complete_counts() {
    for nodes in [json!([issue("APP-1")]), json!([])] {
        let (sent, fetch) = scripted(vec![Ok(page(RETURNED, nodes.clone(), false, Value::Null))]);
        let milestone = fetch_with(INPUT, INPUT, true, fetch)
            .await
            .expect("one complete page");
        assert_eq!(sent.borrow().len(), 1);
        assert_eq!(
            milestone.issues.nodes.len(),
            nodes.as_array().expect("nodes").len()
        );
        let text = markdown(
            &milestone,
            true,
            Utc.with_ymd_and_hms(2020, 2, 2, 0, 0, 0).unwrap(),
            &Utc,
        );
        if nodes.as_array().expect("nodes").is_empty() {
            assert!(text.ends_with("_No issues in this milestone yet._"));
            assert!(!text.contains("**All Issues:**"));
        } else {
            assert!(text.contains("**Total Issues:** 1"));
            assert!(text.contains("**All Issues:**\n\n- APP-1: A task (Completed)"));
            assert!(!text.contains("Showing "));
        }
        let output: Value =
            serde_json::from_slice(&json(&milestone).expect("json")).expect("JSON output");
        assert_eq!(
            output["issues"]["nodes"]
                .as_array()
                .expect("JSON nodes")
                .len(),
            nodes.as_array().expect("nodes").len()
        );
        assert_eq!(
            output["issues"]["pageInfo"],
            json!({"hasNextPage":false,"endCursor":null})
        );
    }
}

#[test]
fn empty_description_and_null_target_date_are_absent_with_valid_relative_dates() {
    let mut milestone = page(RETURNED, json!([]), false, Value::Null)
        .project_milestone
        .expect("milestone");
    milestone.description = Some(String::new());
    milestone.target_date = None;
    let text = markdown(
        &milestone,
        false,
        Utc.with_ymd_and_hms(2020, 2, 2, 0, 0, 0).unwrap(),
        &Utc,
    );
    assert!(text.contains("**Target Date:** Not set"));
    assert!(!text.contains("## Description"));
    assert!(text.contains("**Created:** 1/1/2020"));
    assert!(text.contains("**Updated:** 1 day ago"));
}

#[test]
fn project_name_and_project_url_require_credentials_before_milestone_url_rejection() {
    let milestone_url = "https://linear.app/alpha/milestone/foo";
    for project in [
        "Mobile App",
        "https://linear.app/alpha/project/mobile-app-abc123def456",
    ] {
        let (result, stdout, stderr) = invoke(
            &["milestone", "view", milestone_url, "--project", project],
            &[],
            false,
        );
        let error = result.expect_err("no key");
        assert_eq!(
            error.message,
            "No API key configured. Set LINEAR_API_KEY, add api_key to .linear.toml, or run `linear auth login`."
        );
        assert_eq!(
            error.context.as_deref(),
            Some("Failed to fetch milestone details")
        );
        assert!(stdout.is_empty());
        assert!(stderr.is_empty());
    }
}

#[test]
fn wrong_workspace_project_url_is_rejected_before_credentials() {
    let project = "https://linear.app/alpha/project/mobile-app-abc123def456";
    let (result, stdout, stderr) = invoke(
        &[
            "milestone",
            "view",
            "Launch",
            "--project",
            project,
            "--workspace",
            "beta",
        ],
        &[],
        false,
    );
    let error = result.expect_err("wrong workspace");
    assert_eq!(
        error.message,
        "That URL is for the \"alpha\" workspace, but this is the \"beta\" workspace."
    );
    assert_eq!(
        error.suggestion.as_deref(),
        Some("Pass --workspace alpha, or use a URL from \"beta\".")
    );
    assert_eq!(
        error.context.as_deref(),
        Some("Failed to fetch milestone details")
    );
    assert!(stdout.is_empty());
    assert!(stderr.is_empty());
}

#[test]
fn lookup_graphql_errors_prefer_presentable_message_then_server_message() {
    for (extensions, expected) in [
        (
            json!({"userPresentableMessage":"Choose another milestone"}),
            "Choose another milestone",
        ),
        (json!({}), "Lookup denied"),
    ] {
        let (endpoint, server) =
            serve_once(json!({"errors":[{"message":"Lookup denied","extensions":extensions}]}));
        let (result, stdout, stderr) = invoke(
            &["milestone", "view", "Launch", "--project", INPUT, "--json"],
            &[
                ("LINEAR_API_KEY", "lin_api_fake"),
                ("LINEAR_GRAPHQL_ENDPOINT", &endpoint),
            ],
            false,
        );
        let request = server.join().expect("fake server");
        let error = result.expect_err("GraphQL error");
        assert_eq!(error.message, expected);
        assert_eq!(
            error.context.as_deref(),
            Some("Failed to fetch milestone details")
        );
        assert_eq!(error.kind, AppErrorKind::GraphQl);
        assert_eq!(request["variables"], json!({"projectId":INPUT}));
        assert!(
            request["query"]
                .as_str()
                .expect("query")
                .contains("GetProjectMilestonesForLookup")
        );
        assert!(stdout.is_empty());
        assert!(stderr.is_empty());
    }
}

#[test]
fn spinner_clears_before_error_and_success_output() {
    let url = "https://linear.app/alpha/milestone/foo";
    let (error, stdout, stderr) = invoke(&["milestone", "view", url], &[], true);
    assert_eq!(error.expect_err("URL error").kind, AppErrorKind::Validation);
    let mut expected = linear_cli::platform::spinner::frame(0).into_bytes();
    expected.extend_from_slice(linear_cli::platform::spinner::CLEAR);
    assert_eq!(stdout, expected);
    assert!(stderr.is_empty());

    let case = frozen("c031-empty-text");
    let body = json!({"data":case["graphql"]["groups"][0]["steps"][0]["response"]["data"]});
    let (endpoint, server) = serve_once(body);
    let (status, stdout, stderr) = invoke(
        &["milestone", "view", INPUT],
        &[
            ("LINEAR_API_KEY", "lin_api_fake"),
            ("LINEAR_GRAPHQL_ENDPOINT", &endpoint),
        ],
        true,
    );
    server.join().expect("fake server");
    assert_eq!(
        status.expect("successful action"),
        linear_cli::error::ExitStatus::Success
    );
    let first_frame = linear_cli::platform::spinner::frame(0);
    assert!(stdout.starts_with(first_frame.as_bytes()));
    let last_clear = stdout
        .windows(linear_cli::platform::spinner::CLEAR.len())
        .rposition(|window| window == linear_cli::platform::spinner::CLEAR)
        .expect("spinner clear before rendered output");
    let rendered = &stdout[last_clear + linear_cli::platform::spinner::CLEAR.len()..];
    assert!(
        rendered
            .windows(b"# Launch".len())
            .any(|window| window == b"# Launch")
    );
    assert!(stderr.is_empty());
}

#[tokio::test]
async fn all_mode_keeps_first_page_metadata_and_original_request_id() {
    let (sent, fetch) = scripted(vec![
        Ok(page(RETURNED, json!([issue("APP-1")]), true, json!("one"))),
        Ok(page(
            "second-id",
            json!([issue("APP-2")]),
            false,
            json!("done"),
        )),
    ]);
    let milestone = fetch_with(INPUT, INPUT, true, fetch)
        .await
        .expect("two pages");
    assert_eq!(milestone.id.inner(), RETURNED);
    assert_eq!(milestone.name, "Launch");
    assert_eq!(
        milestone
            .issues
            .nodes
            .iter()
            .map(|issue| issue.identifier.as_str())
            .collect::<Vec<_>>(),
        ["APP-1", "APP-2"]
    );
    assert_eq!(
        milestone.issues.page_info.end_cursor.as_deref(),
        Some("done")
    );
    let sent = sent.borrow();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0]["variables"], json!({"id":INPUT,"first":50}));
    assert_eq!(
        sent[1]["variables"],
        json!({"id":INPUT,"first":50,"after":"one"})
    );
}

#[tokio::test]
async fn all_mode_uses_returned_id_for_missing_cursor_suggestion() {
    for cursor in [Value::Null, json!("")] {
        let (sent, fetch) = scripted(vec![Ok(page(RETURNED, json!([]), true, cursor))]);
        let error = fetch_with(INPUT, INPUT, true, fetch)
            .await
            .expect_err("cursor error");
        assert_eq!(
            error.message,
            "Linear reported more issues but returned no pagination cursor"
        );
        assert_eq!(
            error.suggestion.as_deref(),
            Some(
                "Retry, or use `linear issue query --milestone 00000000-0000-4000-8000-000000000099 --json` for the full list."
            )
        );
        assert_eq!(sent.borrow().len(), 1);
    }
}

#[tokio::test]
async fn cyclic_cursor_stops_after_the_third_page_without_output() {
    let (sent, fetch) = scripted(vec![
        Ok(page(RETURNED, json!([]), true, json!("a"))),
        Ok(page(RETURNED, json!([]), true, json!("b"))),
        Ok(page(RETURNED, json!([]), true, json!("a"))),
    ]);
    let error = fetch_with(INPUT, INPUT, true, fetch)
        .await
        .expect_err("cycle");
    assert_eq!(
        error.message,
        "Linear repeated an issue pagination cursor on page 3"
    );
    assert_eq!(error.suggestion.as_deref(), Some("Retry the command."));
    assert_eq!(sent.borrow().len(), 3);
}

#[tokio::test]
async fn later_null_root_discards_fetched_issues_and_reports_original_input() {
    let null = parse_response(br#"{"data":{"projectMilestone":null}}"#).expect("null root");
    let (sent, fetch) = scripted(vec![
        Ok(page(RETURNED, json!([issue("APP-1")]), true, json!("one"))),
        Ok(null),
    ]);
    let error = fetch_with("Launch", INPUT, true, fetch)
        .await
        .expect_err("not found");
    assert_eq!(error.message, "Milestone not found: Launch");
    assert_eq!(sent.borrow().len(), 2);
}

#[test]
fn route_and_explicit_empty_project_follow_v3_usage_grammar() {
    let args: Vec<OsString> = ["milestone", "view", INPUT, "--all", "--json"]
        .iter()
        .map(OsString::from)
        .collect();
    let Invocation::Action(action) = clap_input::parse(&args).expect("parse") else {
        panic!("action")
    };
    assert_eq!(action.route.route.action(), DispatchAction::MilestoneView);
    let invalid: [&[&str]; 4] = [
        &["milestone", "view", "--project=", INPUT],
        &["milestone", "view", INPUT, "--project="],
        &["milestone", "view", INPUT, "--project", "-j"],
        &["milestone", "view", INPUT, "--project", "--json"],
    ];
    for argv in invalid {
        let args: Vec<OsString> = argv.iter().map(OsString::from).collect();
        let error = clap_input::parse(&args).expect_err("usage error");
        assert!(matches!(error.kind, AppErrorKind::Usage { .. }));
        assert_eq!(error.message, "Missing value for option \"--project\".");
    }
}
