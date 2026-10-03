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
        std::fs::write(&path, wrapper).expect("write stub");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("make stub executable");
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

    /// A clap usage error.
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
