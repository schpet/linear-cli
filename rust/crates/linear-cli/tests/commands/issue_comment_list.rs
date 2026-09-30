use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::ready;
use std::rc::Rc;

use chrono::{TimeZone, Utc};
use linear_cli::commands::issue_comment_list::{render_text, request, run_with};
use linear_cli::error::AppError;
use linear_cli::graphql::envelope::{GraphQlRequest, parse_response};
use linear_cli::graphql::operations::issue_comments::{
    GetIssueComments, GetIssueCommentsVariables,
};
use serde_json::{Value, json};

const ISSUE: &str = "ENG-7";
type Sent = Rc<RefCell<Vec<Value>>>;
type PageResult = Result<GetIssueComments, AppError>;
type Request = GraphQlRequest<GetIssueCommentsVariables>;

fn frozen(id: &str) -> Value {
    let path = format!(
        "{}/../../parity/runner/c063-frozen-cases/{id}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_slice(&std::fs::read(path).expect("frozen case")).expect("case JSON")
}

fn frozen_page(id: &str) -> GetIssueComments {
    let case = frozen(id);
    let data = &case["graphql"]["groups"][0]["steps"][0]["response"]["data"];
    parse_response(json!({"data": data}).to_string().as_bytes()).expect("typed page")
}

fn page(nodes: Value, next: bool, cursor: Value) -> GetIssueComments {
    parse_response(
        json!({"data":{
            "issue":{"comments":{"nodes":nodes,"pageInfo":{"hasNextPage":next,"endCursor":cursor}}}
        }})
        .to_string()
        .as_bytes(),
    )
    .expect("typed page")
}

fn scripted(
    pages: Vec<PageResult>,
) -> (Sent, impl FnMut(Request) -> std::future::Ready<PageResult>) {
    let sent = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&sent);
    let queue = Rc::new(RefCell::new(VecDeque::from(pages)));
    let fetch = move |request: Request| {
        recorded.borrow_mut().push(
            serde_json::to_value(request.variables.expect("variables")).expect("variables JSON"),
        );
        ready(queue.borrow_mut().pop_front().expect("unexpected page"))
    };
    (sent, fetch)
}

#[test]
fn request_uses_nested_connection_and_explicit_null_cursor() {
    let first = request(ISSUE, None);
    let query = &first.query;
    assert!(query.contains("issue(id: $id)"), "{query}");
    assert!(query.contains("comments("), "{query}");
    assert!(query.contains("orderBy: createdAt"), "{query}");
    assert!(query.contains("$id: String!"), "{query}");
    let values = serde_json::to_value(first.variables.expect("variables")).expect("JSON");
    assert_eq!(values, json!({"id":ISSUE,"after":null}));
    let second = request(ISSUE, Some(String::new()));
    assert_eq!(
        serde_json::to_value(second.variables.expect("variables")).expect("JSON"),
        json!({"id":ISSUE,"after":""})
    );
}

#[test]
fn frozen_threads_preserve_authors_raw_markdown_quotes_and_orphans() {
    let case = frozen("c063-url-thread-pipe");
    let page = frozen_page("c063-url-thread-pipe");
    let now = Utc
        .with_ymd_and_hms(2026, 9, 30, 0, 0, 0)
        .single()
        .expect("now");
    let nodes = page.issue.expect("issue").comments.nodes;
    assert_eq!(
        render_text(&nodes, now, false),
        case["expected"]["stdout"]["utf8"].as_str().expect("stdout")
    );
    let color = render_text(&nodes, now, true);
    assert!(color.contains("\x1b[1m@github\x1b[22m"));
    assert!(color.contains("# Heading\n\n*bold* and `inline` %% %s 界"));
}

#[tokio::test]
async fn pages_aggregate_without_sorting_json_and_send_the_prior_cursor() {
    let case = frozen("c063-two-page-json");
    let steps = case["graphql"]["groups"][0]["steps"]
        .as_array()
        .expect("steps");
    let pages = steps
        .iter()
        .map(|step| {
            parse_response::<GetIssueComments>(
                json!({"data":step["response"]["data"]})
                    .to_string()
                    .as_bytes(),
            )
            .map_err(AppError::from)
        })
        .collect();
    let (sent, fetch) = scripted(pages);
    let now = Utc
        .with_ymd_and_hms(2026, 9, 28, 0, 0, 0)
        .single()
        .expect("now");
    let output = run_with(ISSUE, ISSUE, fetch, true, false, now)
        .await
        .expect("pages");
    assert_eq!(
        String::from_utf8(output).expect("UTF-8"),
        case["expected"]["stdout"]["utf8"].as_str().expect("stdout")
    );
    let expected: Vec<Value> = steps
        .iter()
        .map(|step| step["operation"]["variables"].clone())
        .collect();
    assert_eq!(*sent.borrow(), expected);
}

#[tokio::test]
async fn cursor_failure_and_later_null_issue_discard_prior_pages() {
    let now = Utc
        .with_ymd_and_hms(2026, 9, 28, 0, 0, 0)
        .single()
        .expect("now");
    for cursor in [Value::Null, json!("same")] {
        let pages = if cursor.is_null() {
            vec![Ok(page(json!([]), true, cursor))]
        } else {
            vec![
                Ok(page(json!([]), true, cursor.clone())),
                Ok(page(json!([]), true, cursor)),
            ]
        };
        let (sent, fetch) = scripted(pages);
        let error = run_with(ISSUE, ISSUE, fetch, true, false, now)
            .await
            .expect_err("cursor");
        assert_eq!(
            error.to_string(),
            "Failed to list comments: Linear reported more comments but did not return a usable cursor"
        );
        assert_eq!(
            error.suggestion.as_deref(),
            Some("Rerun the command; if it persists, report it.")
        );
        assert!(!sent.borrow().is_empty());
    }
    let null_issue: GetIssueComments =
        parse_response(json!({"data":{"issue":null}}).to_string().as_bytes())
            .expect("nullable issue");
    let (sent, fetch) = scripted(vec![
        Ok(page(json!([]), true, json!("next"))),
        Ok(null_issue),
    ]);
    let error = run_with("Original Name", ISSUE, fetch, true, false, now)
        .await
        .expect_err("later null");
    assert_eq!(
        error.to_string(),
        "Failed to list comments: Issue not found: Original Name"
    );
    assert_eq!(error.suggestion, None);
    assert_eq!(sent.borrow().len(), 2);
}

#[test]
fn malformed_required_comment_fields_are_rejected_at_the_response_boundary() {
    let case = frozen("c063-missing-required-body");
    let raw = case["graphql"]["groups"][0]["steps"][0]["response"]["body"]["utf8"]
        .as_str()
        .expect("raw response");
    let error = parse_response::<GetIssueComments>(raw.as_bytes()).expect_err("required body");
    assert!(error.to_string().contains("body"), "{error}");
}

#[tokio::test]
async fn empty_cursor_is_valid_and_empty_list_keeps_source_text() {
    let (sent, fetch) = scripted(vec![
        Ok(page(json!([]), true, json!(""))),
        Ok(page(json!([]), false, Value::Null)),
    ]);
    let now = Utc
        .with_ymd_and_hms(2026, 9, 30, 0, 0, 0)
        .single()
        .expect("now");
    let output = run_with(ISSUE, ISSUE, fetch, false, false, now)
        .await
        .expect("empty pages");
    assert_eq!(output, b"No comments found for this issue\n");
    assert_eq!(
        *sent.borrow(),
        vec![
            json!({"id":ISSUE,"after":null}),
            json!({"id":ISSUE,"after":""})
        ]
    );
}

mod app_boundary {
    use linear_cli::app::{self, AppContext};
    use linear_cli::auth::file::{CredentialFileSource, CredentialReadFailure};
    use linear_cli::auth::keyring::UnsupportedKeyringReader;
    use linear_cli::config::{
        FileKind, FileSource, GitProbeResult, GitRootProbe, OsFamily, ProcessEnvSnapshot,
    };
    use linear_cli::error::{AppError, AppErrorKind};
    use linear_cli::startup::{self, AppStartupReport};
    use std::ffi::OsString;
    use std::io;
    use std::path::{Path, PathBuf};
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
        let cwd = PathBuf::from("/c063-test");
        let variables = env
            .iter()
            .map(|(name, value)| (OsString::from(name), OsString::from(value)));
        let snapshot = ProcessEnvSnapshot::from_vars_os(cwd, OsFamily::Unix, variables)
            .expect("test environment");
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
            cwd: PathBuf::from("/c063-test"),
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

    #[test]
    fn explicit_input_failures_are_resolved_before_credentials() {
        for (input, message) in [
            ("", "Could not determine issue ID"),
            (
                "00000000-0000-4000-9000-000000000063",
                "Could not determine issue ID",
            ),
            ("07", "Could not determine issue ID"),
            ("7", "an integer id was provided, but no team is set"),
            (
                "https://linear.app/acme/project/name-abc123def456",
                "project URL",
            ),
        ] {
            let (status, stdout, _) = invoke(&["issue", "comment", "list", input], &[], false);
            let error = status.expect_err("input error");
            assert!(
                error.to_string().starts_with("Failed to list comments:"),
                "{input}: {error}"
            );
            assert!(error.to_string().contains(message), "{input}: {error}");
            assert!(stdout.is_empty());
        }
    }

    #[test]
    fn native_parser_rejects_unknown_flags_and_extra_arguments_without_actions() {
        for args in [
            vec!["issue", "comment", "list", "ENG-7", "--sort"],
            vec!["issue", "comment", "list", "ENG-7", "extra"],
        ] {
            let (status, stdout, stderr) = invoke(&args, &[], false);
            let error = status.expect_err("usage error");
            assert_eq!(error.kind, AppErrorKind::Usage);
            let native = error.native_parser_error().expect("native clap error");
            assert_eq!(native.exit_code(), 2);
            assert!(native.use_stderr());
            assert!(stdout.is_empty());
            assert!(stderr.is_empty());
        }
        let (status, stdout, stderr) = invoke(&["issue", "comment", "list", "--help"], &[], false);
        let error = status.expect_err("help is returned for main to render");
        let native = error.native_parser_error().expect("native clap help");
        assert_eq!(native.exit_code(), 0);
        assert!(!native.use_stderr());
        assert!(native.to_string().contains("--json"));
        assert!(stdout.is_empty());
        assert!(stderr.is_empty());
    }
}
