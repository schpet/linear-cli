use linear_cli::app::{self, AppContext};
use linear_cli::auth::file::{CredentialFileSource, CredentialReadFailure};
use linear_cli::auth::keyring::UnsupportedKeyringReader;
use linear_cli::config::{
    FileKind, FileSource, GitProbeResult, GitRootProbe, OsFamily, ProcessEnvSnapshot,
};
use linear_cli::error::{AppError, AppErrorKind, ExitStatus};
use linear_cli::startup::{self, AppStartupReport};
use serde_json::{Value, json};
use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};
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
    let cwd = PathBuf::from("/c058-test");
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
        cwd: PathBuf::from("/c058-test"),
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

fn details() -> Value {
    json!({"identifier":"ENG-7","title":"Repair the widget","description":null,
      "url":"https://linear.app/acme/issue/ENG-7/repair-the-widget", "branchName":"eng-7-widget",
      "state":{"name":"Started","color":"#123456"},"assignee":null,"priority":2,
      "project":null,"projectMilestone":null,"cycle":null,"team":{"activeCycle":null},
      "labels":{"nodes":[]},"parent":null,"children":{"nodes":[]},"attachments":{"nodes":[]},"documents":{"nodes":[]}})
}
#[test]
fn public_title_and_url_preserve_query_and_script_output() {
    let expected_document = include_str!("../../../../../src/utils/linear.ts")
        .split("query GetIssueDetails($id: String!) {")
        .nth(1)
        .expect("query")
        .split("`)")
        .next()
        .expect("query end");
    let expected_document = format!("query GetIssueDetails($id: String!) {{ {expected_document}");
    let tokens = |s: &str| s.split_whitespace().collect::<String>().replace(',', "");
    for (leaf, input, output) in [
        ("title", "eng-7", "Repair the widget"),
        (
            "url",
            "https://linear.app/acme/issue/ENG-7/widget#comment-abcdef12",
            "https://linear.app/acme/issue/ENG-7/repair-the-widget",
        ),
    ] {
        let (endpoint, server) = serve_once(json!({"data":{"issue":details()}}));
        let (status, stdout, stderr) = invoke(
            &["issue", leaf, input],
            &[
                ("LINEAR_API_KEY", "lin_api_fake"),
                ("LINEAR_GRAPHQL_ENDPOINT", &endpoint),
            ],
            false,
        );
        assert_eq!(status.expect("success"), ExitStatus::Success);
        assert_eq!(stdout, format!("{output}\n").as_bytes());
        assert!(stderr.is_empty());
        let request = server.join().expect("server");
        assert_eq!(request["variables"], json!({"id":"ENG-7"}));
        assert_eq!(
            tokens(request["query"].as_str().expect("query")),
            tokens(&expected_document)
        );
    }
}
#[test]
fn explicit_empty_invalid_and_wrong_workspace_fail_before_credentials_or_spinner() {
    for leaf in ["title", "url"] {
        for input in [
            "",
            "ENG-0",
            "7",
            "https://linear.app/acme/team/eng",
            "https://linear.app/foreign/issue/ENG-7",
        ] {
            let (status, stdout, stderr) =
                invoke(&["issue", leaf, input, "--workspace", "acme"], &[], true);
            let error = status.expect_err("reference failure");
            assert_eq!(error.kind, AppErrorKind::Validation);
            assert!(!error.message.contains("No API key"));
            assert_eq!(
                error.context.as_deref(),
                Some(if leaf == "title" {
                    "Failed to get issue title"
                } else {
                    "Failed to get issue URL"
                })
            );
            assert!(stdout.is_empty() && stderr.is_empty());
        }
    }
}
#[test]
fn credential_failure_starts_then_clears_spinner_even_under_ci() {
    for leaf in ["title", "url"] {
        let (result, stdout, _) = invoke(&["issue", leaf, "ENG-7"], &[("CI", "1")], true);
        assert!(result.expect_err("no key").message.contains("No API key"));
        let mut expected = linear_cli::platform::spinner::frame(0).into_bytes();
        expected.extend_from_slice(linear_cli::platform::spinner::CLEAR);
        assert_eq!(stdout, expected);
        let (_, stdout, _) = invoke(&["issue", leaf, "ENG-7"], &[("NO_COLOR", "")], true);
        assert!(stdout.is_empty());
    }
}
#[test]
fn malformed_unprinted_fields_and_graphql_errors_fail_with_command_context() {
    for leaf in ["title", "url"] {
        for response in [
            json!({"data":{"issue":{"title":"Partial", "url":"https://example.invalid"}}}),
            json!({"errors":[{"message":"not found"}]}),
        ] {
            let (endpoint, server) = serve_once(response);
            let (result, stdout, _) = invoke(
                &["issue", leaf, "ENG-7"],
                &[
                    ("LINEAR_API_KEY", "lin_api_fake"),
                    ("LINEAR_GRAPHQL_ENDPOINT", &endpoint),
                ],
                false,
            );
            server.join().expect("server");
            let error = result.expect_err("strict error");
            assert!(matches!(
                error.kind,
                AppErrorKind::GraphQl | AppErrorKind::Invariant
            ));
            assert_eq!(
                error.context.as_deref(),
                Some(if leaf == "title" {
                    "Failed to get issue title"
                } else {
                    "Failed to get issue URL"
                })
            );
            assert!(stdout.is_empty());
        }
    }
}
