use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

use super::mock::MockLinear;

/// The API key `Cli::for_api` configures.
pub const API_KEY: &str = "lin_api_test_key";

const TIMEOUT: Duration = Duration::from_secs(30);

static NEXT_SANDBOX: AtomicU64 = AtomicU64::new(0);

/// A private environment for running the `linear` binary.
///
/// Layout under a fresh temp root (removed on drop):
/// - `home/` is `HOME`; `home/.config/` is `XDG_CONFIG_HOME`
/// - `bin/` is the whole `PATH`, populated by `stub_bin`
/// - `cwd/` is the default working directory (see `cwd`)
pub struct Cli {
    root: PathBuf,
    env: BTreeMap<String, String>,
    stdin: Option<Vec<u8>>,
    cwd: String,
}

impl Cli {
    pub fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "linear-cli-test-{}-{}",
            std::process::id(),
            NEXT_SANDBOX.fetch_add(1, Ordering::Relaxed)
        ));
        if root.exists() {
            std::fs::remove_dir_all(&root).expect("remove stale sandbox");
        }
        for dir in ["home/.config", "bin", "cwd", "calls"] {
            std::fs::create_dir_all(root.join(dir)).expect("create sandbox dir");
        }
        let root = std::fs::canonicalize(root).expect("canonical sandbox root");
        let path = |rel: &str| root.join(rel).display().to_string();
        let env = BTreeMap::from([
            ("HOME".to_owned(), path("home")),
            ("XDG_CONFIG_HOME".to_owned(), path("home/.config")),
            ("PATH".to_owned(), path("bin")),
            ("NO_COLOR".to_owned(), "1".to_owned()),
            ("LINEAR_IGNORE_ENV_FILE".to_owned(), "1".to_owned()),
            ("TZ".to_owned(), "UTC".to_owned()),
            ("LANG".to_owned(), "C.UTF-8".to_owned()),
        ]);
        Self {
            root,
            env,
            stdin: None,
            cwd: "cwd".to_owned(),
        }
    }

    /// A sandbox pointed at `api` and authenticated with `API_KEY` via `LINEAR_API_KEY`.
    pub fn for_api(api: &MockLinear) -> Self {
        Self::new().endpoint(api).env("LINEAR_API_KEY", API_KEY)
    }

    pub fn endpoint(self, api: &MockLinear) -> Self {
        self.env("LINEAR_GRAPHQL_ENDPOINT", &api.url())
    }

    pub fn env(mut self, name: &str, value: &str) -> Self {
        self.env.insert(name.to_owned(), value.to_owned());
        self
    }

    pub fn env_remove(mut self, name: &str) -> Self {
        self.env.remove(name);
        self
    }

    /// Write `contents` to `rel` under the sandbox root, creating parent directories.
    pub fn file(self, rel: &str, contents: &str) -> Self {
        let path = self.path(rel);
        std::fs::create_dir_all(path.parent().expect("file has a parent"))
            .expect("create parent dirs");
        std::fs::write(&path, contents).expect("write sandbox file");
        self
    }

    /// Write the credentials file at its default location.
    pub fn credentials(self, toml: &str) -> Self {
        self.file("home/.config/linear/credentials.toml", toml)
    }

    /// Install an executable `name` on `PATH` that runs the `sh` script body. Every invocation's
    /// argv is recorded for `calls`. The script runs with `PATH=/usr/bin:/bin`.
    pub fn stub_bin(self, name: &str, script: &str) -> Self {
        let log = self.path(&format!("calls/{name}"));
        let wrapper = format!(
            "#!/bin/sh\n\
             {{ for arg in \"$@\"; do printf '%s\\037' \"$arg\"; done; printf '\\036'; }} >> '{}'\n\
             PATH=/usr/bin:/bin\n\
             export PATH\n\
             {script}\n",
            log.display()
        );
        let path = self.path(&format!("bin/{name}"));
        let staged = self.path(&format!("bin/.{name}.new"));
        // A child writer keeps sibling test children from inheriting a script's writable fd.
        let status = Command::new("/bin/sh")
            .args(["-c", "printf '%s' \"$2\" > \"$1\"", "write-stub"])
            .arg(&staged)
            .arg(wrapper)
            .status()
            .expect("run stub writer");
        assert!(status.success(), "write {}: {status}", staged.display());
        std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755))
            .expect("make stub executable");
        std::fs::rename(&staged, &path).expect("publish stub");
        self
    }

    /// Run from `rel` under the sandbox root instead of `cwd/`.
    pub fn cwd(mut self, rel: &str) -> Self {
        std::fs::create_dir_all(self.path(rel)).expect("create working directory");
        self.cwd = rel.to_owned();
        self
    }

    pub fn stdin(mut self, bytes: &[u8]) -> Self {
        self.stdin = Some(bytes.to_vec());
        self
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    pub fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.path(rel)).expect("read sandbox file")
    }

    /// Argv (without the program name) of every invocation of the stub `name`.
    pub fn calls(&self, name: &str) -> Vec<Vec<String>> {
        let path = self.path(&format!("calls/{name}"));
        let log = match std::fs::read_to_string(&path) {
            Ok(log) => log,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
            Err(error) => panic!("read stub log {}: {error}", path.display()),
        };
        log.split_terminator('\u{1e}')
            .map(|call| call.split_terminator('\u{1f}').map(str::to_owned).collect())
            .collect()
    }

    pub fn run(&self, args: &[&str]) -> Run {
        let mut child = Command::new(env!("CARGO_BIN_EXE_linear"))
            .args(args)
            .env_clear()
            .envs(&self.env)
            .current_dir(self.path(&self.cwd))
            .stdin(if self.stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn linear");
        let stdin = self.stdin.clone().map(|bytes| {
            let mut pipe = child.stdin.take().expect("stdin pipe");
            // The CLI may exit without reading all of stdin.
            thread::spawn(move || {
                let _ = pipe.write_all(&bytes);
            })
        });
        let drain = |mut pipe: Box<dyn Read + Send>| {
            thread::spawn(move || {
                let mut bytes = Vec::new();
                pipe.read_to_end(&mut bytes).expect("read child output");
                String::from_utf8(bytes).expect("child output is UTF-8")
            })
        };
        let stdout = drain(Box::new(child.stdout.take().expect("stdout pipe")));
        let stderr = drain(Box::new(child.stderr.take().expect("stderr pipe")));
        let started = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait().expect("wait for linear") {
                break status;
            }
            if started.elapsed() > TIMEOUT {
                child.kill().expect("kill hung linear");
                child.wait().expect("reap hung linear");
                panic!("linear {args:?} did not exit within {TIMEOUT:?}");
            }
            thread::sleep(Duration::from_millis(5));
        };
        if let Some(stdin) = stdin {
            stdin.join().expect("stdin writer");
        }
        Run {
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
            code: status.code().expect("linear exited normally"),
            stdout: stdout.join().expect("stdout reader"),
            stderr: stderr.join().expect("stderr reader"),
        }
    }
}

impl Cli {
    /// Runs `linear` on a pseudo-terminal through script(1), for the prompts
    /// and confirmations that only appear on a terminal. Each step waits until
    /// the screen shows `wait_for` (after the previous step's text), then types
    /// `keys`. The run's `stdout` is what the terminal showed, stdout and
    /// stderr together, with escape sequences and carriage returns removed;
    /// its `stderr` is empty.
    pub fn run_tty(&self, args: &[&str], steps: &[(&str, &str)]) -> Run {
        let binary = env!("CARGO_BIN_EXE_linear");
        let mut command = Command::new("/usr/bin/script");
        if cfg!(target_os = "macos") {
            command.args(["-q", "/dev/null", binary]).args(args);
        } else {
            let quote = |word: &str| format!("'{}'", word.replace('\'', r"'\''"));
            let line: Vec<String> = std::iter::once(binary)
                .chain(args.iter().copied())
                .map(quote)
                .collect();
            command.args(["-q", "-e", "-c", &line.join(" "), "/dev/null"]);
        }
        let mut child = command
            .env_clear()
            .envs(&self.env)
            .env("TERM", "xterm")
            .env("SHELL", "/bin/sh")
            .current_dir(self.path(&self.cwd))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn script");
        let screen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let reader = {
            let screen = std::sync::Arc::clone(&screen);
            let mut pipe = child.stdout.take().expect("stdout pipe");
            thread::spawn(move || {
                let mut buffer = [0; 4096];
                loop {
                    match pipe.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(read) => screen
                            .lock()
                            .expect("screen lock")
                            .extend_from_slice(&buffer[..read]),
                    }
                }
            })
        };
        let shown = || plain_text(&screen.lock().expect("screen lock"));
        let mut stdin = child.stdin.take().expect("stdin pipe");
        let started = Instant::now();
        let mut seen = 0;
        for (wait_for, keys) in steps {
            loop {
                // Prompts wrap at the terminal width, so lines are joined for matching.
                let text = shown().replace('\n', "");
                if let Some(at) = text[seen..].find(wait_for) {
                    seen += at + wait_for.len();
                    break;
                }
                if started.elapsed() > TIMEOUT || child.try_wait().expect("poll script").is_some() {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!(
                        "linear {args:?} never showed {wait_for:?}; the terminal showed:\n{text}"
                    );
                }
                thread::sleep(Duration::from_millis(10));
            }
            stdin.write_all(keys.as_bytes()).expect("type keys");
            stdin.flush().expect("flush keys");
        }
        let status = loop {
            if let Some(status) = child.try_wait().expect("wait for script") {
                break status;
            }
            if started.elapsed() > TIMEOUT {
                child.kill().expect("kill hung script");
                child.wait().expect("reap hung script");
                panic!(
                    "linear {args:?} did not exit within {TIMEOUT:?}:\n{}",
                    shown()
                );
            }
            thread::sleep(Duration::from_millis(5));
        };
        drop(stdin);
        reader.join().expect("screen reader");
        Run {
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
            code: status.code().expect("script exited normally"),
            stdout: shown(),
            stderr: String::new(),
        }
    }
}

/// Terminal output without escape sequences or carriage returns.
fn plain_text(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut plain = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\u{1b}' => match chars.next() {
                // CSI: parameters, then a final byte in @..~.
                Some('[') => {
                    for c in chars.by_ref() {
                        if ('@'..='~').contains(&c) {
                            break;
                        }
                    }
                }
                // OSC: up to BEL or ST.
                Some(']') => {
                    while let Some(c) = chars.next() {
                        if c == '\u{7}' || (c == '\u{1b}' && chars.next_if_eq(&'\\').is_some()) {
                            break;
                        }
                    }
                }
                _ => {}
            },
            '\r' => {}
            c => plain.push(c),
        }
    }
    plain
}

impl Drop for Cli {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.root) {
            eprintln!("failed to remove sandbox {}: {error}", self.root.display());
        }
    }
}

/// The outcome of one `linear` invocation.
#[derive(Debug)]
pub struct Run {
    pub args: Vec<String>,
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Run {
    #[track_caller]
    pub fn success(&self) -> &Self {
        assert_eq!(self.code, 0, "expected success\n{self}");
        self
    }

    /// An ordinary runtime failure: status 1. Assert forwarded or cancellation
    /// statuses exactly on `code`.
    #[track_caller]
    pub fn failure(&self) -> &Self {
        assert_eq!(self.code, 1, "expected a runtime failure\n{self}");
        self
    }

    /// A usage error: from clap, or a value the command rejected after
    /// parsing, such as a missing required value: status 2.
    #[track_caller]
    pub fn usage_error(&self) -> &Self {
        assert_eq!(self.code, 2, "expected a usage error\n{self}");
        self
    }

    #[track_caller]
    pub fn stdout_has(&self, needle: &str) -> &Self {
        assert!(
            self.stdout.contains(needle),
            "stdout lacks {needle:?}\n{self}"
        );
        self
    }

    #[track_caller]
    pub fn stderr_has(&self, needle: &str) -> &Self {
        assert!(
            self.stderr.contains(needle),
            "stderr lacks {needle:?}\n{self}"
        );
        self
    }

    #[track_caller]
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.stdout)
            .unwrap_or_else(|error| panic!("stdout is not JSON: {error}\n{self}"))
    }

    /// The entities of a list command's `--json` output, whatever wrapper surrounds them.
    #[track_caller]
    pub fn json_nodes(&self) -> Vec<Value> {
        super::json::nodes(&self.json())
    }
}

impl std::fmt::Display for Run {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "linear {:?} exited {}\n--- stdout\n{}\n--- stderr\n{}",
            self.args, self.code, self.stdout, self.stderr
        )
    }
}
