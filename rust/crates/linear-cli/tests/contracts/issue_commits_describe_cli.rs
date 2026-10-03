#![cfg(unix)]

//! Public CLI fixtures with fake HTTP/git/jj and isolated synthetic configuration.
use serde_json::json;
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
            "linear-issue-script-{}-{}",
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
    fn run_command(&self, mut command: Command, input: &[u8]) -> Output {
        let mut child = command.spawn().unwrap();
        child.stdin.take().unwrap().write_all(input).unwrap();
        let mut out = child.stdout.take().unwrap();
        let mut err = child.stderr.take().unwrap();
        let stdout = thread::spawn(move || {
            let mut bytes = Vec::new();
            out.read_to_end(&mut bytes).unwrap();
            bytes
        });
        let stderr = thread::spawn(move || {
            let mut bytes = Vec::new();
            err.read_to_end(&mut bytes).unwrap();
            bytes
        });
        let started = Instant::now();
        let mut timed_out = false;
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if started.elapsed() > Duration::from_secs(5) {
                timed_out = true;
                child.kill().unwrap();
                break child.wait().unwrap();
            }
            thread::sleep(Duration::from_millis(2));
        };
        let output = Output {
            status,
            stdout: stdout.join().unwrap(),
            stderr: stderr.join().unwrap(),
        };
        assert!(!timed_out, "fake native process exceeded test-only bound");
        output
    }
}
impl Drop for Home {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn body(wire: &str) -> &str {
    wire.split_once("\r\n\r\n").unwrap().1
}

#[cfg(unix)]
fn fake_processes(home: &Home, config: serde_json::Value, vcs: &str) {
    use std::os::unix::fs::PermissionsExt;
    let bin = home.0.join("bin");
    std::fs::create_dir(&bin).unwrap();
    std::fs::write(home.0.join(".linear.toml"), format!("vcs = {vcs:?}\n")).unwrap();
    std::fs::write(home.0.join("process-config.json"), config.to_string()).unwrap();
    let script = r#"#!/usr/bin/python3
import sys,pathlib,json,os,signal
root=pathlib.Path(__file__).resolve().parents[1];tool=pathlib.Path(sys.argv[0]).name;args=sys.argv[1:];cfg=json.loads((root/'process-config.json').read_text())
# Startup probe remains separate from the new leaves' process stages.
if tool=='git' and args==['rev-parse','--show-toplevel']: print(root);sys.exit(0)
assert sys.stdin.buffer.read()==b'', 'new leaf child stdin must be NULL'
with (root/'events.jsonl').open('a') as f:f.write(json.dumps({'tool':tool,'args':args,'cwd':os.getcwd(),'stdinNull':True,'dummy':os.environ.get('LINEAR_DUMMY_CHILD_TEST')})+'\n')
if tool=='git':
 assert args==['symbolic-ref','--short','HEAD'];sys.stdout.write(cfg.get('branch','feature/eng-7-work'));sys.stderr.write(cfg.get('branchErr',''));sys.exit(cfg.get('branchCode',0))
assert tool=='jj' and args[0]=='log'
if args[1:3]==['-r','::@']:sys.stdout.write(cfg.get('trailers','Fixes ENG-7\n'));sys.exit(cfg.get('trailerCode',0))
if args[args.index('-T')+1]=='commit_id':sys.stdout.write(cfg.get('checkOut','opaque commit\n'));sys.stderr.write('DUMMY hidden probe stderr');sys.exit(cfg.get('checkCode',0))
assert args==['log','-r',args[2],'-p','--git','--no-graph','-T','builtin_log_compact_full_description']
os.write(1,bytes.fromhex(cfg.get('finalHex','44554d4d592070617463680a524157ff62797465730a')));os.write(2,b'DUMMY inherited stderr\n')
if cfg.get('signal'):os.kill(os.getpid(),cfg['signal'])
sys.exit(cfg.get('finalCode',7))
"#;
    for tool in ["git", "jj"] {
        let path = bin.join(tool);
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
}
#[cfg(unix)]
fn events(home: &Home) -> Vec<serde_json::Value> {
    let path = home.0.join("events.jsonl");
    if !path.exists() {
        return vec![];
    }
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
fn full_details() -> serde_json::Value {
    json!({"identifier":"RETURNED-99","title":"DUMMY\nsecond\t界","description":null,"url":"https://example.invalid/DUMMY#fragment","branchName":"dummy-branch","state":{"name":"Started","color":"#123456"},"assignee":null,"priority":2,"project":null,"projectMilestone":null,"cycle":null,"team":{"activeCycle":null},"labels":{"nodes":[]},"parent":null,"children":{"nodes":[]},"attachments":{"nodes":[]},"documents":{"nodes":[]}})
}
#[cfg(unix)]
#[test]
fn missing_key_commits_infers_once_before_key_error_and_never_queries_or_probes() {
    let home = Home::new();
    fake_processes(&home, json!({"trailers":"Fixes ENG-7\n"}), "jj");
    let server = Server::new(vec![]);
    let mut command = home.command(&server.url, &["issue", "commits"]);
    command.env("PATH", home.0.join("bin"));
    let output = home.run_command(command, b"DUMMY offered input\n");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "✗ Failed to show commits: No API key configured\n  Set LINEAR_API_KEY, add api_key to .linear.toml, or run `linear auth login`.\n"
    );
    let trace = events(&home);
    assert_eq!(trace.len(), 1);
    assert_eq!(
        trace[0]["args"],
        json!([
            "log",
            "-r",
            "::@",
            "-T",
            "trailers.map(|t| if(t.key() == \"Linear-issue\", t.value(), \"\"))",
            "--no-graph"
        ])
    );
    assert!(server.finish().is_empty());
}
#[cfg(unix)]
#[test]
fn jj_gate_precedes_invalid_reference_key_network_and_action_children() {
    let home = Home::new();
    fake_processes(&home, json!({}), "git");
    let server = Server::new(vec![]);
    let mut command = home.command(&server.url, &["issue", "commits", "not-valid"]);
    command.env("PATH", home.0.join("bin"));
    let output = home.run_command(command, b"");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "✗ Failed to show commits: commits is only supported with jj-vcs\n  This command requires jujutsu (jj) version control.\n"
    );
    assert!(events(&home).is_empty());
    assert!(server.finish().is_empty());
}
#[cfg(unix)]
#[test]
fn inherited_raw_ff_and_code7_ignore_probe_status_stderr_and_keep_null_stdin_env() {
    let home = Home::new();
    fake_processes(&home, json!({"checkCode":19}), "jj");
    std::fs::write(
        home.0.join(".env"),
        "LINEAR_DUMMY_CHILD_TEST='DUMMY dotenv value'\n",
    )
    .unwrap();
    let server = Server::new(vec![Reply {
        status: 200,
        headers: "Content-Type: application/json\r\n".into(),
        bytes: br#"{"data":{"issue":{"id":"opaque"}}}"#.to_vec(),
    }]);
    let mut command = home.command(&server.url, &["issue", "commits", "eng-7"]);
    command
        .env("PATH", home.0.join("bin"))
        .env("LINEAR_API_KEY", "dummy_key")
        .env_remove("LINEAR_IGNORE_ENV_FILE");
    let output = home.run_command(command, b"DUMMY ignored stdin\n");
    assert_eq!(output.status.code(), Some(7));
    assert_eq!(output.stdout, b"DUMMY patch\nRAW\xffbytes\n");
    assert_eq!(output.stderr, b"DUMMY inherited stderr\n");
    let trace = events(&home);
    assert_eq!(trace.len(), 2);
    let rev = "description(regex:\"(?m)^Linear-issue:.*ENG-7\")";
    assert_eq!(
        trace[0]["args"],
        json!(["log", "-r", rev, "-T", "commit_id", "--no-graph"])
    );
    assert_eq!(
        trace[1]["args"],
        json!([
            "log",
            "-r",
            rev,
            "-p",
            "--git",
            "--no-graph",
            "-T",
            "builtin_log_compact_full_description"
        ])
    );
    assert!(
        trace
            .iter()
            .all(|row| row["stdinNull"] == true && row["dummy"] == "DUMMY dotenv value")
    );
    let requests = server.finish();
    assert_eq!(requests.len(), 1);
    let request: serde_json::Value = serde_json::from_str(body(&requests[0])).unwrap();
    assert_eq!(request["operationName"], "GetIssueId");
    assert_eq!(request["variables"], json!({"id":"ENG-7"}));
}
#[cfg(unix)]
#[test]
fn child_sigterm_maps143_but_empty_lossy_bom_probe_never_runs_final() {
    for empty in [false, true] {
        let home = Home::new();
        fake_processes(
            &home,
            if empty {
                json!({"checkOut":"\u{feff} \n"})
            } else {
                json!({"signal":15})
            },
            "jj",
        );
        let server = Server::new(vec![Reply {
            status: 200,
            headers: "Content-Type: application/json\r\n".into(),
            bytes: br#"{"data":{"issue":{"id":"opaque"}}}"#.to_vec(),
        }]);
        let mut command = home.command(&server.url, &["issue", "commits", "ENG-7"]);
        command
            .env("PATH", home.0.join("bin"))
            .env("LINEAR_API_KEY", "dummy_key");
        let output = home.run_command(command, b"");
        if empty {
            assert_eq!(output.status.code(), Some(1));
            assert!(output.stdout.is_empty());
            assert_eq!(
                output.stderr,
                "✗ Failed to show commits: Commits not found: ENG-7\n".as_bytes()
            );
            assert_eq!(events(&home).len(), 1);
        } else {
            assert_eq!(output.status.code(), Some(143));
            assert_eq!(output.stdout, b"DUMMY patch\nRAW\xffbytes\n");
            assert_eq!(output.stderr, b"DUMMY inherited stderr\n");
            assert_eq!(events(&home).len(), 2);
        }
        assert_eq!(server.finish().len(), 1);
    }
}
#[cfg(unix)]
#[test]
fn describe_all_ref_aliases_preserve_full_selection_resolved_id_and_raw_output() {
    for flag in [None, Some("--references"), Some("-r"), Some("--ref")] {
        let home = Home::new();
        fake_processes(&home, json!({}), "git");
        let server = Server::new(vec![Reply {
            status: 200,
            headers: "Content-Type: application/json\r\n".into(),
            bytes: json!({"data":{"issue":full_details()}})
                .to_string()
                .into_bytes(),
        }]);
        let mut args = vec!["issue", "describe", "eng-7"];
        if let Some(flag) = flag {
            args.push(flag);
        }
        let mut command = home.command(&server.url, &args);
        command
            .env("PATH", home.0.join("bin"))
            .env("LINEAR_API_KEY", "dummy_key");
        let output = home.run_command(command, b"");
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        let magic = if flag.is_some() {
            "References"
        } else {
            "Fixes"
        };
        assert_eq!(output.stdout,format!("ENG-7 DUMMY\nsecond\t界\n\nLinear-issue: {magic} ENG-7\nLinear-issue-url: https://example.invalid/DUMMY#fragment\n").as_bytes());
        assert!(events(&home).is_empty());
        let requests = server.finish();
        assert_eq!(requests.len(), 1);
        let request: serde_json::Value = serde_json::from_str(body(&requests[0])).unwrap();
        assert_eq!(request["variables"], json!({"id":"ENG-7"}));
        assert_eq!(request["operationName"], "GetIssueDetails");
        let q = request["query"].as_str().unwrap();
        for selection in [
            "projectMilestone",
            "children(first: 250)",
            "attachments(first: 50)",
            "documents(first: 50)",
            "labels(first: 50)",
            "activeCycle",
            "metadata",
        ] {
            assert!(q.contains(selection), "full unused selection {selection}");
        }
    }
}
