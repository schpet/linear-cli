use linear_cli::{
    commands::{
        document_target::{Kind, TargetOptions},
        document_write as command,
    },
    platform::{
        prompt::{PromptKey, PromptOutcome, PromptSession},
        prompt_text::TextOptions,
    },
};
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

struct Sandbox(PathBuf);
impl Sandbox {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "linear-docwrite-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        for name in ["bin", "config", "temp"] {
            fs::create_dir_all(root.join(name)).unwrap();
        }
        Self(root)
    }
    fn command(&self, url: &str, argv: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_linear"));
        c.current_dir(&self.0)
            .env_clear()
            .env("HOME", &self.0)
            .env("XDG_CONFIG_HOME", self.0.join("config"))
            .env("APPDATA", self.0.join("config"))
            .env("PATH", self.0.join("bin"))
            .env("TMPDIR", self.0.join("temp"))
            .env("LINEAR_IGNORE_ENV_FILE", "1")
            .env("LINEAR_API_KEY", "lin_api_fake")
            .env("LINEAR_GRAPHQL_ENDPOINT", url)
            .env("NO_COLOR", "1")
            .args(argv);
        c
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
struct Server {
    url: String,
    stop: Arc<AtomicBool>,
    handle: thread::JoinHandle<Vec<Value>>,
}
impl Server {
    fn new(replies: Vec<Value>) -> Self {
        let socket = TcpListener::bind("127.0.0.1:0").unwrap();
        socket.set_nonblocking(true).unwrap();
        let url = format!("http://{}/graphql", socket.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let ending = stop.clone();
        let handle = thread::spawn(move || {
            let start = Instant::now();
            let mut requests = Vec::new();
            while !ending.load(Ordering::Relaxed) {
                assert!(start.elapsed() < Duration::from_secs(15), "mock deadline");
                match socket.accept() {
                    Ok((mut stream, _)) => {
                        // BSD may inherit the listener's nonblocking flag.
                        stream
                            .set_nonblocking(false)
                            .expect("blocking accepted mock stream");
                        let request = request(&mut stream);
                        assert!(
                            requests.len() < replies.len(),
                            "unexpected extra request: {request}"
                        );
                        let body = replies[requests.len()].to_string();
                        requests.push(request);
                        write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(error) => panic!("mock accept: {error}"),
                }
            }
            requests
        });
        Self { url, stop, handle }
    }
    fn finish(self) -> Vec<Value> {
        self.stop.store(true, Ordering::Relaxed);
        self.handle.join().unwrap()
    }
}
fn request(stream: &mut TcpStream) -> Value {
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut bytes = Vec::new();
    let mut header_end = None;
    let mut length = 0;
    loop {
        let mut buf = [0; 2048];
        let count = stream.read(&mut buf).unwrap();
        assert!(count > 0);
        bytes.extend_from_slice(&buf[..count]);
        if header_end.is_none()
            && let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n")
        {
            header_end = Some(end + 4);
            let text = String::from_utf8(bytes[..end].to_vec()).unwrap();
            length = text
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|value| value.trim().parse::<usize>().unwrap())
                })
                .unwrap();
        }
        if let Some(end) = header_end
            && bytes.len() >= end + length
        {
            return serde_json::from_slice(&bytes[end..end + length]).unwrap();
        }
    }
}
fn run(mut command: Command, input: &[u8], hold: bool) -> Output {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(input).unwrap();
    if !hold {
        drop(stdin);
    }
    let mut out = child.stdout.take().unwrap();
    let mut err = child.stderr.take().unwrap();
    let stdout = thread::spawn(move || {
        let mut b = Vec::new();
        out.read_to_end(&mut b).unwrap();
        b
    });
    let stderr = thread::spawn(move || {
        let mut b = Vec::new();
        err.read_to_end(&mut b).unwrap();
        b
    });
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if start.elapsed() > Duration::from_secs(5) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("CLI deadline");
        }
        thread::sleep(Duration::from_millis(5));
    };
    Output {
        status,
        stdout: stdout.join().unwrap(),
        stderr: stderr.join().unwrap(),
    }
}
fn created() -> Value {
    json!({"data":{"documentCreate":{"success":true,"document":{"id":"created-id","slugId":"slug","title":"Returned","url":"https://linear.app/dummy/document/slug"}}}})
}
fn updated() -> Value {
    json!({"data":{"documentUpdate":{"success":true,"document":{"id":"updated-id","slugId":"slug","title":"Returned","url":"https://linear.app/dummy/document/slug","updatedAt":"2026-09-30T00:00:00Z"}}}})
}
fn guard(nodes: Value, next: bool, cursor: Value) -> Value {
    json!({"data":{"document":{"id":"slug","comments":{"nodes":nodes,"pageInfo":{"hasNextPage":next,"endCursor":cursor}}}}})
}

#[test]
fn six_attachments_serialize_only_selected_target_and_present_empty_content() {
    for (kind, key) in [
        (Kind::Project, "projectId"),
        (Kind::Issue, "issueId"),
        (Kind::Initiative, "initiativeId"),
        (Kind::Team, "teamId"),
        (Kind::Cycle, "cycleId"),
        (Kind::Release, "releaseId"),
    ] {
        let mut input = command::input(None, None);
        input.content = Some(String::new());
        command::attach(&mut input, kind, "target-id".into());
        let request = serde_json::to_value(command::create_request("Title".into(), input)).unwrap();
        assert_eq!(
            request["variables"]["input"],
            json!({"title":"Title","content":"",key:"target-id"})
        );
    }
}
#[test]
fn cardinality_is_independent_of_reference_preparation() {
    assert!(TargetOptions::default().cardinality(true).is_err());
    assert!(TargetOptions::default().cardinality(false).is_ok());
    assert!(
        TargetOptions {
            team: Some("T"),
            cycle: Some("next"),
            ..Default::default()
        }
        .cardinality(true)
        .is_ok()
    );
    assert!(
        TargetOptions {
            project: Some("bad URL"),
            issue: Some("ENG-1"),
            ..Default::default()
        }
        .cardinality(false)
        .unwrap_err()
        .message()
        .contains("--project, --issue")
    );
}
#[test]
fn prompt_text_options_apply_defaults_and_trim_answers() {
    let mut session = PromptSession::script(
        &b"\n \n  text \n\xc2\x85text\xc2\x85\n\xc2\x85text\xc2\x85\n"[..],
        Vec::new(),
    );
    let opt = TextOptions {
        required: true,
        default: Some("ENG"),
    };
    assert_eq!(
        session.text_with_options("Team", opt).unwrap(),
        PromptOutcome::Submitted("ENG".into())
    );
    assert!(session.text_with_options("Team", opt).is_err());
    assert_eq!(
        session
            .text_with_options(
                "Title",
                TextOptions {
                    required: false,
                    default: None
                }
            )
            .unwrap(),
        PromptOutcome::Submitted("text".into())
    );
    assert_eq!(
        session
            .text_with_options(
                "Title",
                TextOptions {
                    required: false,
                    default: None
                }
            )
            .unwrap(),
        PromptOutcome::Submitted("text".into())
    );
    assert_eq!(
        session.text("Existing", 0, |_| Ok(())).unwrap(),
        PromptOutcome::Submitted("text".into())
    );
    assert!(
        TextOptions {
            required: false,
            default: Some("bad\n")
        }
        .preflight()
        .is_err()
    );
    assert_eq!(
        TextOptions {
            required: true,
            default: None
        }
        .answer(" 😀 ")
        .unwrap(),
        "😀"
    );
    assert!(
        TextOptions {
            required: true,
            default: None
        }
        .answer("  ")
        .is_err()
    );
}
#[test]
fn public_prompt_optin_keys_share_editing_and_existing_confirmation_selection() {
    let mut keys = [
        PromptKey::Enter,
        PromptKey::Character(' '),
        PromptKey::Character('x'),
        PromptKey::Enter,
    ]
    .into_iter();
    let mut session: PromptSession<std::io::Empty, Vec<u8>> =
        PromptSession::keys(Vec::new(), 80, 24, move || Ok(keys.next().unwrap())).unwrap();
    assert_eq!(
        session
            .text_with_options(
                "Team",
                TextOptions {
                    required: true,
                    default: Some("ENG")
                }
            )
            .unwrap(),
        PromptOutcome::Submitted("ENG".into())
    );
    assert_eq!(
        session
            .text_with_options(
                "Title",
                TextOptions {
                    required: true,
                    default: None
                }
            )
            .unwrap(),
        PromptOutcome::Submitted("x".into())
    );
    let mut session = PromptSession::script(&b"\n"[..], Vec::new());
    assert_eq!(
        session.confirm("Existing confirmation", false).unwrap(),
        PromptOutcome::Submitted(false)
    );
}
#[test]
fn create_file_precedes_target_lookup_while_update_target_precedes_missing_file() {
    let sandbox = Sandbox::new();
    let server = Server::new(vec![]);
    let output = run(
        sandbox.command(
            &server.url,
            &[
                "document",
                "create",
                "--title",
                "T",
                "--issue",
                "eng-1",
                "--content-file",
                "missing.md",
            ],
        ),
        b"",
        false,
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(server.finish().is_empty());
    let server = Server::new(vec![json!({"data":{"issue":{"id":"issue-id"}}})]);
    let output = run(
        sandbox.command(
            &server.url,
            &[
                "document",
                "update",
                "slug",
                "--issue",
                "eng-1",
                "--content-file",
                "missing.md",
            ],
        ),
        b"",
        false,
    );
    assert_eq!(output.status.code(), Some(1));
    let requests = server.finish();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["variables"], json!({"id":"ENG-1"}));
}
#[test]
fn piped_stdin_is_document_content() {
    for (bytes, hold, content) in [
        (b"Hello, world\n".to_vec(), false, Some("Hello, world")),
        (Vec::new(), false, None),
    ] {
        let sandbox = Sandbox::new();
        let server = Server::new(vec![created()]);
        let before = Instant::now();
        let output = run(
            sandbox.command(
                &server.url,
                &[
                    "document",
                    "c",
                    "--title",
                    "T",
                    "--project",
                    "123e4567-e89b-42d3-a456-426614174000",
                ],
            ),
            &bytes,
            hold,
        );
        assert!(output.status.success(), "{:?}", output.stderr);
        assert!(before.elapsed() < Duration::from_secs(2));
        let requests = server.finish();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0]["variables"]["input"].get("content"),
            content.map(|value| json!(value)).as_ref()
        );
    }
}
#[test]
fn metadata_ignores_piped_content_and_body_file_suppresses_edit() {
    let sandbox = Sandbox::new();
    let server = Server::new(vec![updated()]);
    let output = run(
        sandbox.command(&server.url, &["document", "u", "slug", "--title", "New"]),
        b"ignored body",
        false,
    );
    assert!(output.status.success());
    let requests = server.finish();
    assert_eq!(requests[0]["variables"]["input"], json!({"title":"New"}));
    fs::write(sandbox.0.join("body.md"), b"\xef\xbb\xbf# raw\r\n").unwrap();
    let server = Server::new(vec![updated()]);
    let output = run(
        sandbox.command(
            &server.url,
            &[
                "document",
                "update",
                "slug",
                "--content-file",
                "body.md",
                "--edit",
                "--force",
            ],
        ),
        b"ignored",
        false,
    );
    assert!(output.status.success(), "{:?}", output.stderr);
    let requests = server.finish();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0]["variables"]["input"],
        json!({"content":"# raw\r\n"})
    );
}
#[test]
fn active_empty_quote_stops_before_bad_cursor_and_never_mutates() {
    let sandbox = Sandbox::new();
    let server = Server::new(vec![guard(
        json!([{"id":"c1","quotedText":"","resolvedAt":null,"archivedAt":null}]),
        true,
        Value::Null,
    )]);
    let output = run(
        sandbox.command(
            &server.url,
            &["document", "update", "slug", "--content", "new"],
        ),
        b"",
        false,
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("comment c1 quoting \"\"")
    );
    let requests = server.finish();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["variables"], json!({"id":"slug","after":null}));
}
#[test]
fn guard_empty_cursor_closed_comments_and_ordered_mutation_succeed() {
    let sandbox = Sandbox::new();
    let closed = json!([{"id":"a","quotedText":"closed","resolvedAt":"2026-01-01T00:00:00Z","archivedAt":null},{"id":"a","quotedText":"archived","resolvedAt":null,"archivedAt":"2026-01-01T00:00:00Z"},{"id":"b","quotedText":null,"resolvedAt":null,"archivedAt":null}]);
    let server = Server::new(vec![
        guard(closed, true, json!("")),
        guard(json!([]), false, Value::Null),
        updated(),
    ]);
    let output = run(
        sandbox.command(
            &server.url,
            &["document", "update", "slug", "--content", "new"],
        ),
        b"",
        false,
    );
    assert!(output.status.success(), "{:?}", output.stderr);
    let requests = server.finish();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[1]["variables"], json!({"id":"slug","after":""}));
    assert_eq!(requests[2]["variables"]["input"], json!({"content":"new"}));
}
#[test]
fn missing_and_repeated_guard_cursor_fail_before_mutation() {
    for replies in [
        vec![guard(json!([]), true, Value::Null)],
        vec![
            guard(json!([]), true, json!("same")),
            guard(json!([]), true, json!("same")),
        ],
    ] {
        let sandbox = Sandbox::new();
        let server = Server::new(replies);
        let output = run(
            sandbox.command(
                &server.url,
                &["document", "update", "slug", "--content", "new"],
            ),
            b"",
            false,
        );
        assert_eq!(output.status.code(), Some(1));
        let requests = server.finish();
        assert!(
            requests
                .iter()
                .all(|request| !request["query"].as_str().unwrap().contains("mutation"))
        );
    }
}

#[test]
fn mutation_selected_fields_remain_strict_instead_of_successful_partial_output() {
    use linear_cli::graphql::{
        envelope::parse_response,
        operations::document_write::{CreateDocument, UpdateDocument},
    };
    for document in [
        serde_json::Value::Null,
        json!({"id":"id","slugId":"slug","url":"url"}),
        json!({"id":3,"slugId":"slug","title":"title","url":"url"}),
    ] {
        let response = json!({"data":{"documentCreate":{"success":true,"document":document}}});
        assert!(parse_response::<CreateDocument>(response.to_string().as_bytes()).is_err());
    }
    let response = json!({"data":{"documentUpdate":{"success":true,"document":{"id":"id","slugId":"slug","title":"title","url":"url"}}}});
    assert!(parse_response::<UpdateDocument>(response.to_string().as_bytes()).is_err());
}

#[test]
fn editor_menu_labels_use_literal_forward_slashes() {
    for (name, expected) in [
        ("", None),
        ("editor", Some("editor")),
        ("/usr/bin/editor", Some("editor")),
        ("/usr/bin/", None),
        ("/", None),
        ("/.", Some(".")),
        ("/..", Some("..")),
        (r"C:\tools\editor.exe", Some(r"C:\tools\editor.exe")),
        ("path/editor with spaces", Some("editor with spaces")),
    ] {
        assert_eq!(
            command::editor_label(std::ffi::OsStr::new(name)).as_deref(),
            expected,
            "{name}"
        );
    }
}
