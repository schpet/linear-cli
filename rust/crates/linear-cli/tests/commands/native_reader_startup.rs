//! Dummy metadata hydration through public startup/commands. No OS store calls.
use linear_cli::{
    app::{AppContext, finalize, run},
    auth::file::{CredentialFileSource, CredentialReadFailure},
    auth::{
        LookupResult,
        keyring::{
            KeyringReader,
            windows_spec::{WindowsReadFailure, classify_windows_lookup},
        },
    },
    commands::{
        auth_default::{self, DefaultAction},
        auth_token,
    },
    config::{
        ConfigSecret, FileKind, FileSource, GitProbeResult, GitRootProbe, OsFamily,
        ProcessEnvSnapshot,
    },
    error::ExitStatus,
    startup::{load, render_startup_diagnostic},
};
use std::{
    ffi::OsString,
    io::{self, BufRead, Read, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

struct MemoryFiles {
    dotenv: Option<&'static str>,
    project: Option<&'static str>,
}
impl MemoryFiles {
    fn bytes(&self, path: &Path) -> Option<&'static str> {
        match path.file_name()?.to_str()? {
            ".env" => self.dotenv,
            ".linear.toml" => self.project,
            _ => None,
        }
    }
}
impl FileSource for MemoryFiles {
    fn kind(&self, path: &Path) -> io::Result<Option<FileKind>> {
        Ok(self.bytes(path).map(|_| FileKind::Regular))
    }
    fn read_bounded(&self, path: &Path, _max: u64) -> io::Result<Vec<u8>> {
        self.bytes(path)
            .map(|text| text.as_bytes().to_vec())
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }
}
struct NoGit;
impl GitRootProbe for NoGit {
    fn probe(&self) -> GitProbeResult {
        GitProbeResult::SpawnFailure
    }
}
struct Metadata;
impl CredentialFileSource for Metadata {
    fn read_credentials(&self, _: &Path) -> Result<Option<Vec<u8>>, CredentialReadFailure> {
        Ok(Some(
            b"default='alpha'\nworkspaces=['alpha','beta']\n".to_vec(),
        ))
    }
}
enum Reply {
    MacDummy,
    WindowsDummy,
    WindowsMalformed,
    Missing,
}
struct FakeReader {
    reply: Reply,
    calls: Mutex<Vec<String>>,
}
impl FakeReader {
    fn new(reply: Reply) -> Self {
        Self {
            reply,
            calls: Mutex::new(Vec::new()),
        }
    }
    fn assert_eager_once(&self) {
        let mut calls = self.calls.lock().unwrap().clone();
        calls.sort();
        assert_eq!(calls, ["alpha", "beta"]);
    }
}
impl KeyringReader for FakeReader {
    fn lookup(&self, workspace: &str) -> LookupResult {
        self.calls.lock().unwrap().push(workspace.to_owned());
        match self.reply {
            Reply::MacDummy => LookupResult::Hit(ConfigSecret::new(format!("dummy_{workspace}"))),
            Reply::WindowsDummy => classify_windows_lookup(Ok(format!("dummy_{workspace}")
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect())),
            Reply::WindowsMalformed => classify_windows_lookup(Ok(vec![0x00, 0xd8])),
            Reply::Missing => classify_windows_lookup(Err(WindowsReadFailure::NoEntry)),
        }
    }
}
fn fixture_root() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(r"C:\p10b")
    } else {
        PathBuf::from("/p10b")
    }
}

fn process(extras: &[(&str, &str)]) -> ProcessEnvSnapshot {
    let root = fixture_root();
    let mut values = vec![
        (OsString::from("HOME"), root.join("home").into_os_string()),
        (
            OsString::from("XDG_CONFIG_HOME"),
            root.join("config").into_os_string(),
        ),
        (
            OsString::from("LINEAR_IGNORE_ENV_FILE"),
            OsString::from("1"),
        ),
        (OsString::from("NO_COLOR"), OsString::from("1")),
    ];
    for (name, value) in extras {
        let name = OsString::from(name);
        values.retain(|(existing, _)| existing != &name);
        values.push((name, OsString::from(value)));
    }
    ProcessEnvSnapshot::from_vars_os(root, OsFamily::Unix, values).unwrap()
}

#[test]
fn metadata_for_both_adapters_hydrates_token_and_default_without_extra_lookup() {
    for reply in [Reply::MacDummy, Reply::WindowsDummy] {
        let reader = FakeReader::new(reply);
        let loaded = load(
            &process(&[]),
            &MemoryFiles {
                dotenv: None,
                project: None,
            },
            &NoGit,
            &Metadata,
            &reader,
        )
        .result
        .unwrap();
        assert_eq!(
            auth_token::run(&loaded.config.options, &loaded.credentials, None).unwrap(),
            b"dummy_alpha\n"
        );
        assert!(matches!(
            auth_default::prepare(&loaded.credentials, Some("alpha")).unwrap(),
            DefaultAction::Output(_)
        ));
        reader.assert_eager_once();
    }
}

#[test]
fn raw_and_project_keys_keep_eager_metadata_failure_warning_and_no_postresolution_lookup() {
    for project in [false, true] {
        let reader = FakeReader::new(Reply::WindowsMalformed);
        let vars = if project {
            vec![]
        } else {
            vec![("LINEAR_API_KEY", "dummy_raw")]
        };
        let report = load(
            &process(&vars),
            &MemoryFiles {
                dotenv: None,
                project: project.then_some("api_key='dummy_project'\n"),
            },
            &NoGit,
            &Metadata,
            &reader,
        );
        let warnings = report
            .diagnostics
            .iter()
            .map(|d| render_startup_diagnostic(d, false))
            .collect::<String>();
        assert_eq!(
            warnings,
            "Warning: Failed to read keyring for workspace \"alpha\": lookup failed\nWarning: Failed to read keyring for workspace \"beta\": lookup failed\n"
        );
        let loaded = report.result.unwrap();
        assert_eq!(
            auth_token::run(&loaded.config.options, &loaded.credentials, None).unwrap(),
            if project {
                b"dummy_project\n".to_vec()
            } else {
                b"dummy_raw\n".to_vec()
            }
        );
        reader.assert_eager_once();
    }
    let missing = FakeReader::new(Reply::Missing);
    let report = load(
        &process(&[]),
        &MemoryFiles {
            dotenv: None,
            project: None,
        },
        &NoGit,
        &Metadata,
        &missing,
    );
    assert_eq!(
        render_startup_diagnostic(&report.diagnostics[0], false),
        "Warning: No keyring entry for workspace \"alpha\". Run `linear auth login` to re-authenticate.\n"
    );
    missing.assert_eager_once();
}

#[test]
fn process_empty_suppresses_dotenv_and_backend_stays_selectable() {
    let reader = FakeReader::new(Reply::WindowsDummy);
    let loaded = load(
        &process(&[("LINEAR_IGNORE_ENV_FILE", "0"), ("LINEAR_API_KEY", "")]),
        &MemoryFiles {
            dotenv: Some("LINEAR_API_KEY=dummy_dotenv\n"),
            project: None,
        },
        &NoGit,
        &Metadata,
        &reader,
    )
    .result
    .unwrap();
    assert_eq!(
        auth_token::run(&loaded.config.options, &loaded.credentials, None).unwrap(),
        b"dummy_alpha\n"
    );
    reader.assert_eager_once();
}

#[test]
fn ordinary_user_read_sends_one_dummy_header_after_global_metadata_hydration() {
    for reply in [Reply::MacDummy, Reply::WindowsDummy] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}/graphql", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(value) => break value,
                    Err(e)
                        if e.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline =>
                    {
                        thread::sleep(Duration::from_millis(5))
                    }
                    Err(e) => panic!("bounded fake HTTP accept: {e}"),
                }
            };
            stream
                .set_nonblocking(false)
                .expect("blocking accepted mock stream");
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut input = io::BufReader::new(stream.try_clone().unwrap());
            let mut headers = Vec::new();
            let mut length = None;
            let mut used = 0;
            loop {
                let mut line = String::new();
                assert!(input.read_line(&mut line).unwrap() > 0);
                used += line.len();
                assert!(used <= 65536);
                if line == "\r\n" {
                    break;
                }
                if let Some((name, value)) = line.split_once(':')
                    && name.eq_ignore_ascii_case("content-length")
                {
                    length = Some(value.trim().parse::<usize>().unwrap());
                }
                headers.push(line);
            }
            let mut body = vec![0; length.unwrap()];
            assert!(body.len() <= 65536);
            input.read_exact(&mut body).unwrap();
            let payload=b"{\"data\":{\"viewer\":{\"organization\":{\"users\":{\"nodes\":[],\"pageInfo\":{\"hasNextPage\":false,\"endCursor\":null}}}}}}";
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",payload.len()).unwrap();
            stream.write_all(payload).unwrap();
            assert!(matches!(listener.accept(),Err(e) if e.kind()==io::ErrorKind::WouldBlock));
            (
                headers,
                serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
            )
        });
        let reader = FakeReader::new(reply);
        let startup = load(
            &process(&[("LINEAR_GRAPHQL_ENDPOINT", &endpoint)]),
            &MemoryFiles {
                dotenv: None,
                project: None,
            },
            &NoGit,
            &Metadata,
            &reader,
        );
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut context = AppContext {
            startup,
            cwd: fixture_root(),
            stdout: &mut stdout,
            stderr: &mut stderr,
            stdin_tty: false,
            stdout_tty: false,
            stderr_tty: false,
            stdout_finalization: None,
        };
        let args = vec!["user".to_owned(), "list".to_owned(), "--json".to_owned()];
        let result = run(&args, &mut context);
        assert!(matches!(
            finalize(result, &mut context).unwrap(),
            ExitStatus::Success
        ));
        drop(context);
        let (headers, request) = server.join().unwrap();
        assert!(
            headers
                .iter()
                .any(|h| h.eq_ignore_ascii_case("authorization: dummy_alpha\r\n"))
        );
        assert!(
            request["query"]
                .as_str()
                .unwrap()
                .contains("GetOrganizationMembers")
        );
        assert_eq!(
            request["variables"],
            serde_json::json!({"includeDisabled":false,"first":100})
        );
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&stdout).unwrap(),
            serde_json::json!({"nodes":[],"pageInfo":{"hasNextPage":false,"endCursor":null}})
        );
        assert!(stderr.is_empty());
        reader.assert_eager_once();
    }
}
