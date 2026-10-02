//! Actual CLI raw wire, response stages and runtime/file effects; all synthetic.
use std::{
    io::{Read, Write},
    net::TcpListener,
    process::{Command, Output, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
struct Reply {
    status: u16,
    headers: String,
    bytes: Vec<u8>,
}

struct Server {
    url: String,
    stop: Arc<AtomicBool>,
    join: thread::JoinHandle<Vec<String>>,
}
impl Server {
    fn new(replies: Vec<Reply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}/graphql", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let done = stop.clone();
        let join = thread::spawn(move || {
            let mut requests = vec![];
            let start = Instant::now();
            while !done.load(Ordering::SeqCst) {
                let (mut stream, _) = match listener.accept() {
                    Ok(v) => v,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(start.elapsed() < Duration::from_secs(12));
                        thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(e) => panic!("{e}"),
                };
                stream
                    .set_nonblocking(false)
                    .expect("blocking accepted mock stream");
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut bytes = vec![];
                loop {
                    let mut chunk = [0; 8192];
                    let n = stream.read(&mut chunk).unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&chunk[..n]);
                    let text = std::str::from_utf8(&bytes).unwrap();
                    if let Some((headers, body)) = text.split_once("\r\n\r\n") {
                        let length = headers
                            .lines()
                            .find_map(|line| {
                                let (k, v) = line.split_once(':')?;
                                k.eq_ignore_ascii_case("content-length")
                                    .then(|| v.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        if body.len() >= length {
                            break;
                        }
                    }
                }
                requests.push(String::from_utf8(bytes).unwrap());
                let reply = &replies[requests.len() - 1];
                write!(
                    stream,
                    "HTTP/1.1 {} Fixture\r\n{}Content-Length: {}\r\nConnection: close\r\n\r\n",
                    reply.status,
                    reply.headers,
                    reply.bytes.len()
                )
                .unwrap();
                stream.write_all(&reply.bytes).unwrap();
            }
            requests
        });
        Self { url, stop, join }
    }
    fn finish(self) -> Vec<String> {
        self.stop.store(true, Ordering::SeqCst);
        self.join.join().unwrap()
    }
}
static SERIAL: AtomicUsize = AtomicUsize::new(0);
struct Home(std::path::PathBuf);
impl Home {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "linear-auth-mutation-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn command(&self, endpoint: &str, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_linear"));
        command
            .args(args)
            .current_dir(&self.0)
            .env_clear()
            .env("HOME", &self.0)
            .env("XDG_CONFIG_HOME", &self.0)
            .env("APPDATA", &self.0)
            .env("PATH", "")
            .env("NO_COLOR", "1")
            .env("CI", "1")
            .env("LINEAR_IGNORE_ENV_FILE", "1")
            .env("LINEAR_GRAPHQL_ENDPOINT", endpoint)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }
    fn run(&self, endpoint: &str, args: &[&str], input: &str) -> Output {
        let mut child = self.command(endpoint, args).spawn().unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    }
}
impl Drop for Home {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn viewer() -> Reply {
    Reply {status:200,headers:"Content-Type: application/json\r\n".into(),
 bytes:br#"{"data":{"viewer":{"name":"DUMMY User","email":"dummy@example.invalid","organization":{"name":"DUMMY Organization","urlKey":"dummy"}}}}"#.to_vec()}
}
fn file(home: &Home) -> std::path::PathBuf {
    home.0.join("linear/credentials.toml")
}
#[test]
fn login_rejects_a_key_that_cleans_to_empty_before_any_request() {
    let home = Home::new();
    let server = Server::new(vec![]);
    let out = home.run(
        &server.url,
        &["auth", "login", "--key", " !!! ", "--plaintext"],
        "",
    );
    assert_eq!(out.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("No API key provided"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!file(&home).exists());
    assert!(server.finish().is_empty());
}
#[test]
fn full_login_decode_refuses_corrupt_fields_before_any_local_write() {
    let home = Home::new();
    let server=Server::new(vec![Reply {status:200,headers:"Content-Type: application/json\r\n".into(),bytes:br#"{"data":{"viewer":{"name":null,"email":"dummy","organization":{"name":"org","urlKey":"dummy"}}}}"#.to_vec()}]);
    let out = home.run(
        &server.url,
        &["auth", "login", "--key", "dummy_key", "--plaintext"],
        "",
    );
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert!(!file(&home).exists());
    assert_eq!(server.finish().len(), 1);
}
#[test]
fn effective_env_and_dotenv_warn_but_empty_process_value_and_toml_do_not() {
    for kind in ["env", "dotenv", "empty-process", "toml"] {
        let home = Home::new();
        let server = Server::new(vec![viewer()]);
        if matches!(kind, "dotenv" | "empty-process") {
            std::fs::write(home.0.join(".env"), "LINEAR_API_KEY=dummy_env\n").unwrap();
        }
        if kind == "toml" {
            std::fs::write(home.0.join(".linear.toml"), "api_key='dummy_toml'\n").unwrap();
        }
        let mut command = home.command(
            &server.url,
            &["auth", "login", "--key", "dummy_explicit", "--plaintext"],
        );
        command.env_remove("LINEAR_IGNORE_ENV_FILE");
        if kind == "env" {
            command.env("LINEAR_API_KEY", "dummy_env");
        }
        if kind == "empty-process" {
            command.env("LINEAR_API_KEY", "");
        }
        let out = command.output().unwrap();
        assert!(
            out.status.success(),
            "{kind}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let text = String::from_utf8(out.stdout).unwrap();
        assert_eq!(
            text.contains("Warning: LINEAR_API_KEY"),
            matches!(kind, "env" | "dotenv"),
            "{kind}: {text}"
        );
        let requests = server.finish();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].contains("authorization: dummy_explicit"));
    }
}
#[cfg(target_os = "linux")]
#[test]
fn post_auth_backend_401_is_classified_by_whole_inner_catch_without_write() {
    use std::os::unix::fs::PermissionsExt;
    let home = Home::new();
    let bin = home.0.join("bin");
    std::fs::create_dir(&bin).unwrap();
    let exe = bin.join("secret-tool");
    std::fs::write(&exe,"#!/bin/sh\nif [ \"$#\" -eq 0 ]; then exit 2; fi\nprintf '%s\\n' \"$1\" >> \"$HOME/events\"\nif [ \"$1\" = store ]; then /bin/cat >/dev/null; echo 'DUMMY401backend' >&2; exit 3; fi\nexit 1\n").unwrap();
    std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o700)).unwrap();
    let server = Server::new(vec![viewer()]);
    let mut command = home.command(&server.url, &["auth", "login", "--key", "dummy_key"]);
    command.env("PATH", &bin);
    let out = command.output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert_eq!(
        String::from_utf8(out.stderr).unwrap(),
        "✗ Failed to login: Invalid API key\n  Check that your API key is correct and not expired.\n"
    );
    assert!(!file(&home).exists());
    assert_eq!(std::fs::read(home.0.join("events")).unwrap(), b"store\n");
    assert_eq!(server.finish().len(), 1);
}
#[test]
fn all_three_new_leaves_keep_eager_strict_credential_startup() {
    for leaf in ["login", "logout", "migrate"] {
        let home = Home::new();
        std::fs::create_dir(home.0.join("linear")).unwrap();
        std::fs::write(file(&home), "workspaces=23\n").unwrap();
        let out = home
            .command("http://127.0.0.1:1/graphql", &["auth", leaf])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1));
        assert!(out.stdout.is_empty());
        assert!(
            String::from_utf8(out.stderr)
                .unwrap()
                .contains("invalid credentials file")
        );
        assert_eq!(std::fs::read(file(&home)).unwrap(), b"workspaces=23\n");
    }
}
#[cfg(target_os = "linux")]
#[test]
fn source_warning_lines_are_plain_on_piped_stdout() {
    use std::os::unix::fs::PermissionsExt;
    let home = Home::new();
    std::fs::create_dir(home.0.join("linear")).unwrap();
    std::fs::write(file(&home), "default='alpha'\nalpha='dummy_old'\n").unwrap();
    let bin = home.0.join("bin");
    std::fs::create_dir(&bin).unwrap();
    let exe = bin.join("secret-tool");
    std::fs::write(&exe, "#!/bin/sh\nexit 2\n").unwrap();
    std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o700)).unwrap();
    let server = Server::new(vec![viewer()]);
    let mut command = home.command(&server.url, &["auth", "login", "--key", "dummy_explicit"]);
    command
        .env("PATH", &bin)
        .env("LINEAR_API_KEY", "dummy_env")
        .env_remove("NO_COLOR");
    let mut child = command.spawn().unwrap();
    child.stdin.take().unwrap().write_all(b"n\r").unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(!text.contains('\x1b'), "{text}");
    assert!(text.ends_with("Remove it from your shell config to use multi-workspace auth.\n"));
    assert_eq!(server.finish().len(), 1);
}

#[cfg(target_os = "linux")]
#[test]
fn signal_completed_availability_still_stores_then_saves_metadata() {
    use std::os::unix::fs::PermissionsExt;
    let home = Home::new();
    let bin = home.0.join("bin");
    std::fs::create_dir(&bin).unwrap();
    let exe = bin.join("secret-tool");
    std::fs::write(&exe, "#!/bin/sh\nif [ \"$#\" -eq 0 ]; then kill -TERM \"$$\"; fi\nprintf '%s\\n' \"$1\" >> \"$HOME/events\"\nif [ \"$1\" = store ]; then /bin/cat > \"$HOME/stored-input\"; exit 0; fi\nexit 1\n").unwrap();
    std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o700)).unwrap();
    let server = Server::new(vec![viewer()]);
    let mut command = home.command(&server.url, &["auth", "login", "--key", "dummy_key"]);
    command.env("PATH", &bin);
    let out = command.output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stderr.is_empty());
    assert_eq!(std::fs::read(home.0.join("events")).unwrap(), b"store\n");
    assert_eq!(
        std::fs::read(home.0.join("stored-input")).unwrap(),
        b"dummy_key"
    );
    assert_eq!(
        std::fs::read(file(&home)).unwrap(),
        b"default = \"dummy\"\nworkspaces = [\"dummy\"]\n"
    );
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "Logged in to workspace: DUMMY Organization (dummy)\n  User: DUMMY User <dummy@example.invalid>\n  Set as default workspace\n"
    );
    assert_eq!(server.finish().len(), 1);
}

#[cfg(target_os = "linux")]
#[test]
fn store_and_clear_killed_by_a_signal_report_the_signal() {
    use std::os::unix::fs::PermissionsExt;
    for action in ["store", "clear"] {
        let home = Home::new();
        let bin = home.0.join("bin");
        std::fs::create_dir(&bin).unwrap();
        let exe = bin.join("secret-tool");
        std::fs::write(&exe, "#!/bin/sh\nif [ \"$#\" -eq 0 ]; then exit 2; fi\nif [ \"$1\" = lookup ]; then printf dummy_stored; exit 0; fi\nprintf '%s\\n' \"$1\" >> \"$HOME/events\"\nif [ \"$1\" = store ]; then /bin/cat >/dev/null; fi\necho 'DUMMY signaled operation' >&2\nkill -TERM \"$$\"\n").unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o700)).unwrap();
        let initial = b"default = \"zeta\"\nworkspaces = [\"zeta\"]\n";
        if action == "clear" {
            std::fs::create_dir(home.0.join("linear")).unwrap();
            std::fs::write(file(&home), initial).unwrap();
        }
        let server = Server::new(if action == "store" {
            vec![viewer()]
        } else {
            vec![]
        });
        let args = if action == "store" {
            vec!["auth", "login", "--key", "dummy_key"]
        } else {
            vec!["auth", "logout", "zeta", "--force"]
        };
        let mut command = home.command(&server.url, &args);
        command.env("PATH", &bin);
        let out = command.output().unwrap();
        assert_eq!(out.status.code(), Some(1));
        assert!(out.stdout.is_empty());
        let context = if action == "store" {
            "Failed to login: Failed to authenticate: Failed to store API key in system keyring for workspace \"dummy\""
        } else {
            "Failed to logout: Failed to remove API key from system keyring for workspace \"zeta\""
        };
        assert_eq!(
            String::from_utf8(out.stderr).unwrap(),
            format!(
                "✗ {context}: secret-tool {action} failed (signal: 15 (SIGTERM)): DUMMY signaled operation\n"
            )
        );
        assert_eq!(
            std::fs::read(home.0.join("events")).unwrap(),
            format!("{action}\n").as_bytes()
        );
        if action == "clear" {
            assert_eq!(std::fs::read(file(&home)).unwrap(), initial);
        } else {
            assert!(!file(&home).exists());
        }
        assert_eq!(server.finish().len(), usize::from(action == "store"));
    }
}
