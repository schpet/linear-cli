//! Prospective public contracts; ignored draft, not compiled or executed.
use linear_cli::{
    auth::{CredentialStore, hydrate, parse_credentials},
    commands::config_generate as command,
    config::{
        ConfigInputs, ConfigOptions, OptionInputs, OsFamily, RawConfigFile, SelectedEnv,
        parse_config_tier,
    },
    error::AppErrorKind,
    graphql::{
        envelope::parse_response,
        operations::config_generate::{Config, ConfigTeam},
    },
    platform::{
        prompt::{PromptKey, PromptOutcome, PromptSession},
        selector::SelectOption,
    },
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, VecDeque},
    io::Cursor,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
fn options(env: &[(&str, &str)], project: &str) -> ConfigOptions {
    let env = ConfigInputs {
        cwd: PathBuf::from("/private/project"),
        os: OsFamily::Unix,
        process_env: env
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    };
    let dotenv = SelectedEnv {
        applied: BTreeMap::new(),
        source_path: None,
        diagnostics: vec![],
    };
    let project = parse_config_tier(RawConfigFile {
        path: PathBuf::from("/private/project/.linear.toml"),
        bytes: project.as_bytes().to_vec(),
    })
    .unwrap();
    ConfigOptions::from_inputs(OptionInputs {
        env: &env,
        dotenv: &dotenv,
        project: Some(&project),
        global: None,
    })
    .unwrap()
}
fn store(toml: &str) -> CredentialStore {
    let tier = parse_config_tier(RawConfigFile {
        path: PathBuf::from("/private/config/credentials.toml"),
        bytes: toml.as_bytes().to_vec(),
    })
    .unwrap();
    hydrate(parse_credentials(tier).unwrap(), vec![]).unwrap()
}
fn response(teams: Vec<Value>, workspace: &str) -> Config {
    parse_response(
        json!({"data":{"viewer":{"organization":{"urlKey":workspace}},"teams":{"nodes":teams}}})
            .to_string()
            .as_bytes(),
    )
    .unwrap()
}
fn team(id: &str, key: &str, name: &str) -> Value {
    json!({"id":id,"key":key,"name":name})
}
#[test]
fn explicit_auth_and_ephemeral_workspace_ignore_project_default_without_store_changes() {
    let config_options = options(
        &[],
        "workspace = \"project-alpha\"\nteam_id = \"ignored\"\nissue_sort = \"priority\"\n",
    );
    let one = store("sole = \"lin_api_fake\"\n");
    assert!(
        matches!(command::workspace_choice(&config_options,&one,None).unwrap(),command::WorkspaceChoice::Only(name) if name=="sole")
    );
    assert_eq!(one.default(), None);
    let multi =
        store("default = \"beta\"\nalpha = \"lin_api_alpha_fake\"\nbeta = \"lin_api_beta_fake\"\n");
    match command::workspace_choice(&config_options, &multi, None).unwrap() {
        command::WorkspaceChoice::Menu {
            options,
            default_index,
        } => {
            assert_eq!(default_index, 1);
            assert_eq!(options[1].label, "beta (default)")
        }
        _ => panic!("workspace prompt required"),
    }
    let no_default = store("alpha = \"lin_api_alpha_fake\"\nbeta = \"lin_api_beta_fake\"\n");
    match command::workspace_choice(&config_options, &no_default, None).unwrap() {
        command::WorkspaceChoice::Menu {
            options,
            default_index,
        } => {
            assert_eq!(default_index, 0);
            assert_eq!(options[0].label, "alpha")
        }
        _ => panic!("first workspace without default"),
    }
    for configured in [
        options(&[("LINEAR_API_KEY", "lin_api_fake")], ""),
        options(&[], "api_key = \"lin_api_fake\"\n"),
    ] {
        assert!(matches!(
            command::workspace_choice(&configured, &multi, None).unwrap(),
            command::WorkspaceChoice::Existing
        ))
    }
    assert!(matches!(
        command::workspace_choice(&config_options, &multi, Some("missing")).unwrap(),
        command::WorkspaceChoice::Existing
    ));
    // Empty raw env shadows configured key, preserving source falsey auth decision.
    let shadow = options(&[("LINEAR_API_KEY", "")], "api_key = \"lin_api_fake\"\n");
    assert!(matches!(
        command::workspace_choice(&shadow, &multi, None).unwrap(),
        command::WorkspaceChoice::Menu { .. }
    ));
}
#[test]
fn native_unselectable_values_are_typed_before_each_prompt_but_single_auto_stays_ordinary() {
    let config_options = options(&[], "");
    for name in [" ", "bad\\nworkspace"] {
        let multi = store(&format!(
            "\"{name}\" = \"lin_api_fake\"\nnormal = \"lin_api_fake\"\n"
        ));
        let error = command::workspace_choice(&config_options, &multi, None).unwrap_err();
        assert_eq!(error.kind, AppErrorKind::Validation);
        assert!(error.message.contains("C082-SOURCE-VALID-MENU-REFUSAL"));
    }
    let single = store("\" \" = \"lin_api_fake\"\n");
    assert!(matches!(
        command::workspace_choice(&config_options, &single, None).unwrap(),
        command::WorkspaceChoice::Only(_)
    ));
    for id in ["", " ", "bad\nID"] {
        let decoded = response(vec![team(id, "K", "Name")], "wire-workspace");
        let error = command::prepare_teams(decoded.teams.nodes).unwrap_err();
        assert_eq!(error.kind, AppErrorKind::Validation);
        assert!(error.message.contains("C082-SOURCE-VALID-MENU-REFUSAL"));
    }
}
#[test]
fn stable_lowercase_names_and_duplicate_ids_choose_first_sorted_key() {
    let data = response(
        vec![
            team("same", "B", "beta"),
            team("same", "A1", "alpha"),
            team("another", "A2", "Alpha"),
        ],
        "returned-workspace",
    );
    let teams = command::prepare_teams(data.teams.nodes).unwrap();
    assert_eq!(
        teams.iter().map(|t| t.key.as_str()).collect::<Vec<_>>(),
        ["A1", "A2", "B"]
    );
    assert_eq!(command::team_key(&teams, "same").unwrap(), "A1");
    let empty: Vec<ConfigTeam> = vec![];
    assert!(
        command::prepare_teams(empty)
            .unwrap_err()
            .message
            .contains("C082-EMPTY-TEAMS")
    );
    let wire = serde_json::to_value(command::request()).unwrap();
    assert_eq!(wire["operationName"], "Config");
    assert!(wire.get("variables").is_none());
    let compact = |text: &str| {
        text.chars()
            .filter(|ch| !ch.is_whitespace() && *ch != ',')
            .collect::<String>()
    };
    assert_eq!(
        compact(wire["query"].as_str().unwrap()),
        compact(
            "query Config { viewer { organization { urlKey } } teams { nodes { id key name } } }"
        )
    );
}
#[test]
fn owned_search_no_match_enter_stays_editable_and_recovers_with_backspace() {
    let choices = [
        SelectOption {
            label: "Alpha (A)".into(),
            value: "a".into(),
        },
        SelectOption {
            label: "Beta (B)".into(),
            value: "b".into(),
        },
    ];
    let mut keys = VecDeque::from([
        PromptKey::Character('Z'),
        PromptKey::Enter,
        PromptKey::Backspace,
        PromptKey::Down,
        PromptKey::Enter,
    ]);
    let mut session = PromptSession::<Cursor<Vec<u8>>, _>::keys(vec![], 80, 24, move || {
        Ok(keys.pop_front().unwrap_or(PromptKey::EndOfInput))
    })
    .unwrap();
    assert_eq!(
        session
            .searchable_select("Select a team:", "Search teams", &choices)
            .unwrap(),
        PromptOutcome::Submitted("b".into())
    );
    let output = String::from_utf8(session.into_output().unwrap()).unwrap();
    assert!(output.contains("No matches"));
    assert!(output.contains("Beta (B)"));
    let mut script = PromptSession::script_cr_or_lf(Cursor::new(b"Beta\r\npriority\r"), vec![]);
    assert_eq!(
        script
            .searchable_select("Select a team:", "Search teams", &choices)
            .unwrap(),
        PromptOutcome::Submitted("b".into())
    );
    script.suspend().unwrap();
    script.resume().unwrap();
    assert_eq!(
        command::sort_prompt(&mut script).unwrap(),
        PromptOutcome::Submitted(command::SortChoice::Priority)
    );
    let mut no_match = PromptSession::script_cr_or_lf(Cursor::new(b"Z\r"), vec![]);
    assert!(
        no_match
            .searchable_select("Select a team:", "Search teams", &choices)
            .unwrap_err()
            .message
            .contains("C082-SEARCH-PROTOCOL")
    );
}
#[test]
fn staged_eof_error_and_exact_raw_template_keep_source_bytes() {
    for stage in ["workspace", "team", "sort order"] {
        let error = command::stage::<String>(PromptOutcome::EndOfInput, stage)
            .unwrap_err()
            .with_context(command::CONTEXT);
        assert_eq!(
            error.display_message(),
            format!("Failed to generate configuration: unexpected EOF while selecting {stage}")
        );
    }
    let actual = command::template(
        "wire\"\\\nworkspace",
        "K\"\\\n界",
        command::SortChoice::Manual,
    );
    assert_eq!(
        actual,
        "# linear cli\n# https://github.com/schpet/linear-cli\n\nworkspace = \"wire\"\\\nworkspace\"\nteam_id = \"K\"\\\n界\"\nissue_sort = \"manual\"\n"
    );
    // Full typed selected fields, not partial selected/root JSON recovery.
    for bad in [
        json!({"viewer":null,"teams":{"nodes":[]}}),
        json!({"viewer":{"organization":{"urlKey":7}},"teams":{"nodes":[]}}),
        json!({"viewer":{"organization":{"urlKey":"wire"}},"teams":{"nodes":[{"id":"a","key":"A","name":null}]}}),
    ] {
        assert!(parse_response::<Config>(json!({"data":bad}).to_string().as_bytes()).is_err());
    }
}
#[test]
fn destination_normalization_and_direct_overwrite_preserve_source_local_effects() {
    assert_eq!(
        command::destination(&command::LateRoot::Fallback, |_| panic!(
            "spawn fallback must not stat"
        )),
        "./.linear.toml"
    );
    assert_eq!(
        command::destination(&command::LateRoot::Completed("".into()), |_| false),
        ".linear.toml"
    );
    if cfg!(unix) {
        assert_eq!(
            command::destination(&command::LateRoot::Completed("//x/./a/../b".into()), |_| {
                false
            }),
            "/x/b/.linear.toml"
        )
    }
    let root = std::env::temp_dir().join(format!(
        "c082-local-effects-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(root.join(".config")).unwrap();
    let path = command::destination(
        &command::LateRoot::Completed(root.to_str().unwrap().into()),
        |p| std::fs::metadata(p).is_ok(),
    );
    let credentials = root.join("credentials.toml");
    std::fs::write(&credentials, b"DUMMY CREDENTIAL STORE UNCHANGED\n").unwrap();
    std::fs::write(&path, b"old api_key/comments overwritten\n").unwrap();
    let contents = command::template("wire", "TEAM", command::SortChoice::Priority);
    let output = command::write_config(&root, &path, &contents).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), contents.as_bytes());
    assert_eq!(
        output,
        format!("Configuration written to {path}\n").as_bytes()
    );
    assert_eq!(
        std::fs::read(credentials).unwrap(),
        b"DUMMY CREDENTIAL STORE UNCHANGED\n"
    );
    std::fs::remove_file(&path).unwrap();
    std::fs::remove_dir(root.join(".config")).unwrap();
    std::fs::write(root.join(".config"), b"not a directory").unwrap();
    let failing = command::destination(
        &command::LateRoot::Completed(root.to_str().unwrap().into()),
        |p| std::fs::metadata(p).is_ok(),
    );
    assert!(failing.ends_with(".config/linear.toml"));
    assert!(command::write_config(&root, &failing, &contents).is_err());
    std::fs::remove_dir_all(root).unwrap();
}

use linear_cli::graphql::transport::{
    ApiKey, CaMode, Deadline, EndpointUrl, GraphQlTransport, ProxyMode, ResponseCap,
    TransportConfig,
};
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::{Duration, Instant},
};
struct Reply {
    status: u16,
    mime: &'static str,
    body: String,
}
impl Reply {
    fn raw(status: u16, mime: &'static str, body: &str) -> Self {
        Self {
            status,
            mime,
            body: body.to_owned(),
        }
    }
}
fn server(replies: Vec<Reply>) -> (GraphQlTransport, thread::JoinHandle<Vec<Value>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let endpoint = format!("http://{}/graphql", listener.local_addr().unwrap());
    let worker = thread::spawn(move || {
        let mut requests = vec![];
        for reply in replies {
            let started = Instant::now();
            let mut socket = loop {
                match listener.accept() {
                    Ok((socket, _)) => break socket,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            started.elapsed() < Duration::from_secs(4),
                            "expected request missing"
                        );
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(error) => panic!("accept: {error}"),
                }
            };
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut bytes = vec![];
            loop {
                let mut byte = [0];
                socket.read_exact(&mut byte).unwrap();
                bytes.push(byte[0]);
                if bytes.ends_with(b"\r\n\r\n") {
                    break;
                }
            }
            let length = std::str::from_utf8(&bytes)
                .unwrap()
                .lines()
                .find_map(|line| {
                    let (key, value) = line.split_once(':')?;
                    key.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .unwrap();
            let mut body = vec![0; length];
            socket.read_exact(&mut body).unwrap();
            requests.push(serde_json::from_slice(&body).unwrap());
            write!(socket, "HTTP/1.1 {} OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", reply.status, reply.mime, reply.body.len(), reply.body).unwrap();
        }
        assert!(listener.accept().is_err(), "unexpected request");
        requests
    });
    let transport = GraphQlTransport::new(
        EndpointUrl::parse(&endpoint).unwrap(),
        ApiKey::new("fixture-key".into()).unwrap(),
        TransportConfig {
            proxy: ProxyMode::Direct,
            ca: CaMode::PublicRoots,
            deadline: Deadline::new(Duration::from_secs(2)).unwrap(),
            max_response_bytes: ResponseCap::new(65536).unwrap(),
        },
    )
    .unwrap();
    (transport, worker)
}

#[tokio::test]
async fn config_fetch_preserves_handled_raw_fallback_and_full_required_decode_without_variables() {
    for (status, mime, body) in [
        (
            200,
            "application/json",
            r#"{"errors":[{"message":""},{"message":"boom"}]}"#,
        ),
        (500, "text/plain", "boom"),
    ] {
        let (transport, worker) = server(vec![Reply::raw(status, mime, body)]);
        let error = command::fetch(&transport)
            .await
            .unwrap_err()
            .with_context(command::CONTEXT);
        let output = error.display_message();
        assert!(output.starts_with("Failed to generate configuration: "));
        assert!(output.contains("boom") && output.contains("request") && output.contains("Config"));
        assert!(!output.contains("ClientError:") && !output.contains("unexpected HTTP status"));
        let requests = worker.join().unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].get("variables").is_none());
    }
    for bad in [
        json!({"viewer":null,"teams":{"nodes":[]}}),
        json!({"viewer":{"organization":{"urlKey":7}},"teams":{"nodes":[]}}),
        json!({"viewer":{"organization":{"urlKey":"wire"}},"teams":{"nodes":[{"id":"a","key":"A","name":null}]}}),
    ] {
        let body = json!({"data":bad}).to_string();
        let (transport, worker) = server(vec![Reply::raw(200, "application/json", &body)]);
        let error = command::fetch(&transport).await.unwrap_err();
        assert!(
            error.message.contains("C082-UNEXPECTED-SHAPE")
                && error.message.contains("no configuration written")
        );
        assert_eq!(worker.join().unwrap().len(), 1);
    }
}

#[cfg(unix)]
#[tokio::test]
async fn fresh_late_git_uses_dotenv_overlay_lossy_js_trim_ignored_exit_and_bounded_reap() {
    use linear_cli::config::{
        FileKind, FileSource, GitProbeResult, GitRootProbe, ProcessEnvSnapshot,
    };
    use std::{ffi::OsString, io, os::unix::fs::PermissionsExt};
    struct Dotenv {
        bytes: Vec<u8>,
    }
    impl FileSource for Dotenv {
        fn kind(&self, path: &std::path::Path) -> io::Result<Option<FileKind>> {
            Ok((path.file_name() == Some(std::ffi::OsStr::new(".env")))
                .then_some(FileKind::Regular))
        }
        fn read_bounded(&self, path: &std::path::Path, max: u64) -> io::Result<Vec<u8>> {
            assert_eq!(path.file_name().unwrap(), ".env");
            assert!(u64::try_from(self.bytes.len()).unwrap() <= max);
            Ok(self.bytes.clone())
        }
    }
    struct NoGit;
    impl GitRootProbe for NoGit {
        fn probe(&self) -> GitProbeResult {
            GitProbeResult::SpawnFailure
        }
    }
    let root = std::env::temp_dir().join(format!(
        "c082-git-public-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let bin = root.join("bin");
    std::fs::create_dir(&bin).unwrap();
    let process = ProcessEnvSnapshot::from_vars_os(
        root.clone(),
        OsFamily::Unix,
        [(OsString::from("HOME"), root.clone().into_os_string())],
    )
    .unwrap();
    let loaded = linear_cli::config::load_startup(
        &process,
        &Dotenv {
            bytes: b"LINEAR_GIT_FIXTURE=qualified-overlay\n".to_vec(),
        },
        &NoGit,
    )
    .result
    .unwrap();
    assert_eq!(
        loaded.child_env.get("LINEAR_GIT_FIXTURE"),
        Some("qualified-overlay")
    );
    let git = bin.join("git");
    let script = |payload: &str| {
        std::fs::write(&git,format!("#!/usr/bin/python3\nimport os,sys,time\nassert sys.argv[1:]==['rev-parse','--show-toplevel']\nassert os.read(0,1)==b''\nassert os.environ['LINEAR_GIT_FIXTURE']=='qualified-overlay'\n{payload}\n")).unwrap();
        std::fs::set_permissions(&git, std::fs::Permissions::from_mode(0o755)).unwrap();
    };
    script("os.write(1,bytes.fromhex('efbbbf20ff207061746820efbbbf0a'));sys.exit(128)");
    assert_eq!(
        command::late_root_with_program(
            &root,
            &loaded.child_env,
            command::GitLimits::default(),
            &git
        )
        .await
        .unwrap(),
        command::LateRoot::Completed("� path".to_owned())
    );
    script("os.write(1,bytes.fromhex('c285'));sys.exit(1)");
    // U+0085 is not JS trim whitespace, unlike native Rust trim.
    assert_eq!(
        command::late_root_with_program(
            &root,
            &loaded.child_env,
            command::GitLimits::default(),
            &git
        )
        .await
        .unwrap(),
        command::LateRoot::Completed("\u{85}".into())
    );
    script("os.write(1,b'x'*100)");
    let bounded = command::GitLimits {
        bytes: 8,
        timeout: Duration::from_secs(2),
    };
    assert!(
        command::late_root_with_program(&root, &loaded.child_env, bounded, &git)
            .await
            .unwrap_err()
            .message
            .contains("C082-GIT-BOUNDS")
    );
    script("time.sleep(10)");
    let timed = command::GitLimits {
        bytes: 8,
        timeout: Duration::from_millis(150),
    };
    let started = Instant::now();
    assert!(
        command::late_root_with_program(&root, &loaded.child_env, timed, &git)
            .await
            .unwrap_err()
            .message
            .contains("deadline")
    );
    assert!(started.elapsed() < Duration::from_secs(2));
    std::fs::remove_file(&git).unwrap();
    assert_eq!(
        command::late_root_with_program(
            &root,
            &loaded.child_env,
            command::GitLimits::default(),
            &git
        )
        .await
        .unwrap(),
        command::LateRoot::Fallback
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn config_mixed_output_refusal_is_only_terminal_stdin_actual_fifo_and_has_helpful_diagnostic() {
    for (input, output) in [(false, false), (false, true), (true, false)] {
        command::check_prompt_topology(input, output).unwrap()
    }
    let error = command::check_prompt_topology(true, true)
        .unwrap_err()
        .with_context(command::CONTEXT);
    assert_eq!(error.kind, AppErrorKind::Validation);
    assert!(error.display_message().starts_with("Failed to generate configuration: Configuration prompts require terminal or regular-file stdout"));
    assert!(!error.message.contains("C082"));
    assert!(error.suggestion.unwrap().contains("piped prompt answers"));
}
