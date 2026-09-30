use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::fmt;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use super::source::{
    ConfigInputs, GitIoStage, GitProbeError, GitProbeResult, GitRootProbe, OsFamily,
};

const MAX_GIT_STDOUT_BYTES: u64 = 64 * 1024;
const DEFAULT_GIT_TIMEOUT: Duration = Duration::from_secs(30);

enum ChildEnvironment {
    Inherit,
    Replace(BTreeMap<OsString, OsString>),
}

/// Runs the source-compatible Git root command in the supplied working directory.
/// Production children inherit the complete process environment.
pub struct RealGitRootProbe {
    cwd: PathBuf,
    timeout: Duration,
    environment: ChildEnvironment,
}

impl RealGitRootProbe {
    pub fn new(cwd: PathBuf) -> Self {
        Self {
            cwd,
            timeout: DEFAULT_GIT_TIMEOUT,
            environment: ChildEnvironment::Inherit,
        }
    }

    /// A private process environment and short timeout for deterministic adapter tests.
    #[doc(hidden)]
    pub fn with_test_environment(
        cwd: PathBuf,
        environment: BTreeMap<OsString, OsString>,
        timeout: Duration,
    ) -> Self {
        Self {
            cwd,
            timeout,
            environment: ChildEnvironment::Replace(environment),
        }
    }
}

fn io_failure(stage: GitIoStage, error: &io::Error) -> GitProbeResult {
    GitProbeResult::Failed(GitProbeError::Io {
        stage,
        kind: error.kind(),
    })
}

fn stop_and_reap(child: &mut Child) -> Result<(), GitProbeError> {
    let _ = child.kill();
    child.wait().map(|_| ()).map_err(|error| GitProbeError::Io {
        stage: GitIoStage::Reap,
        kind: error.kind(),
    })
}

fn bounded_probe(probe: &RealGitRootProbe) -> GitProbeResult {
    let mut command = Command::new("git");
    command
        .arg("rev-parse")
        .arg("--show-toplevel")
        .current_dir(&probe.cwd)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .stdout(Stdio::piped());
    if let ChildEnvironment::Replace(environment) = &probe.environment {
        command.env_clear().envs(environment);
    }
    // The Deno loader treats every command-spawn exception as no Git root.
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(_) => return GitProbeResult::SpawnFailure,
    };
    let Some(stdout) = child.stdout.take() else {
        let _ = stop_and_reap(&mut child);
        return GitProbeResult::Failed(GitProbeError::Io {
            stage: GitIoStage::ReadStdout,
            kind: io::ErrorKind::BrokenPipe,
        });
    };
    let (sender, receiver) = mpsc::channel();
    let reader = thread::Builder::new()
        .name("linear-git-stdout".to_owned())
        .spawn(move || {
            let mut bytes = Vec::new();
            let result = stdout
                .take(MAX_GIT_STDOUT_BYTES + 1)
                .read_to_end(&mut bytes)
                .map(|_| bytes);
            let _ = sender.send(result);
        });
    if let Err(error) = reader {
        let _ = stop_and_reap(&mut child);
        return io_failure(GitIoStage::ReadStdout, &error);
    }
    let started = Instant::now();
    let mut status = None;
    let mut output = None;
    loop {
        if status.is_none() {
            match child.try_wait() {
                Ok(done) => status = done,
                Err(error) => {
                    let _ = stop_and_reap(&mut child);
                    return io_failure(GitIoStage::Poll, &error);
                }
            }
        }
        if output.is_none() {
            match receiver.try_recv() {
                Ok(Ok(bytes)) => {
                    if u64::try_from(bytes.len()).is_ok_and(|len| len > MAX_GIT_STDOUT_BYTES) {
                        let _ = stop_and_reap(&mut child);
                        return GitProbeResult::Failed(GitProbeError::Oversize);
                    }
                    output = Some(bytes);
                }
                Ok(Err(error)) => {
                    let _ = stop_and_reap(&mut child);
                    return io_failure(GitIoStage::ReadStdout, &error);
                }
                Err(TryRecvError::Disconnected) => {
                    let _ = stop_and_reap(&mut child);
                    return GitProbeResult::Failed(GitProbeError::Io {
                        stage: GitIoStage::ReadStdout,
                        kind: io::ErrorKind::BrokenPipe,
                    });
                }
                Err(TryRecvError::Empty) => {}
            }
        }
        if let Some(status) = status
            && let Some(bytes) = output.take()
        {
            let stdout = match String::from_utf8(bytes) {
                Ok(stdout) => stdout,
                Err(_) => return GitProbeResult::Failed(GitProbeError::InvalidUtf8),
            };
            // A normal rev-parse response has one path and at most its line ending.
            let one_line = stdout
                .strip_suffix("\r\n")
                .or_else(|| stdout.strip_suffix('\n'))
                .unwrap_or(&stdout);
            let root = one_line.trim();
            if one_line.contains(['\n', '\r', '\0'])
                || (!root.is_empty() && !Path::new(root).is_absolute())
            {
                return GitProbeResult::Failed(GitProbeError::MalformedStdout);
            }
            return GitProbeResult::Completed {
                success: status.success(),
                stdout,
            };
        }
        if started.elapsed() >= probe.timeout {
            if let Err(error) = stop_and_reap(&mut child) {
                return GitProbeResult::Failed(error);
            }
            return GitProbeResult::Failed(GitProbeError::Timeout);
        }
        thread::sleep(Duration::from_millis(5));
    }
}

impl GitRootProbe for RealGitRootProbe {
    fn probe(&self) -> GitProbeResult {
        bounded_probe(self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProcessEnvError {
    InvalidName,
    InvalidValue { name: String },
    DuplicateName { name: String },
}

impl fmt::Display for ProcessEnvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName => f.write_str("a relevant environment variable name is not UTF-8"),
            Self::InvalidValue { name } => write!(f, "{name} is not valid UTF-8"),
            Self::DuplicateName { name } => write!(f, "duplicate environment variable {name}"),
        }
    }
}

impl std::error::Error for ProcessEnvError {}

#[derive(Clone, Eq, PartialEq)]
pub struct ProcessEnvSnapshot {
    pub inputs: ConfigInputs,
    /// Kept losslessly so invalid UTF-8 only fails if a pager is actually used.
    pub pager: Option<OsString>,
    /// Original spelling of a normalized key; mainly useful for Windows diagnostics.
    pub original_names: BTreeMap<String, String>,
}

impl ProcessEnvSnapshot {
    pub fn capture(cwd: PathBuf, os: OsFamily) -> Result<Self, ProcessEnvError> {
        Self::from_vars_os(cwd, os, env::vars_os())
    }

    pub fn from_vars_os(
        cwd: PathBuf,
        os: OsFamily,
        variables: impl IntoIterator<Item = (OsString, OsString)>,
    ) -> Result<Self, ProcessEnvError> {
        let mut process_env = BTreeMap::new();
        let mut original_names = BTreeMap::new();
        let mut pager = None;
        for (raw_name, raw_value) in variables {
            let Some(name) = raw_name.to_str() else {
                if relevant(&raw_name.to_string_lossy(), os) {
                    return Err(ProcessEnvError::InvalidName);
                }
                continue;
            };
            let key = if os == OsFamily::Windows {
                name.to_ascii_uppercase()
            } else {
                name.to_owned()
            };
            if key == "PAGER" {
                if pager.replace(raw_value).is_some() {
                    return Err(ProcessEnvError::DuplicateName { name: key });
                }
                continue;
            }
            if !relevant(&key, os) {
                continue;
            }
            let Some(value) = raw_value.to_str() else {
                return Err(ProcessEnvError::InvalidValue {
                    name: name.to_owned(),
                });
            };
            if process_env.insert(key.clone(), value.to_owned()).is_some() {
                return Err(ProcessEnvError::DuplicateName {
                    name: name.to_owned(),
                });
            }
            original_names.insert(key, name.to_owned());
        }
        Ok(Self {
            pager,
            inputs: ConfigInputs {
                cwd,
                os,
                process_env,
            },
            original_names,
        })
    }
}

fn relevant(name: &str, os: OsFamily) -> bool {
    let key = if os == OsFamily::Windows {
        name.to_ascii_uppercase()
    } else {
        name.to_owned()
    };
    key.starts_with("LINEAR_")
        || key.starts_with("GH_")
        || key.starts_with("GITHUB_")
        || matches!(
            key.as_str(),
            "NO_COLOR"
                | "CI"
                | "TMPDIR"
                | "TMP"
                | "TEMP"
                | "HOME"
                | "XDG_CONFIG_HOME"
                | "APPDATA"
                | "HTTP_PROXY"
                | "HTTPS_PROXY"
                | "ALL_PROXY"
                | "NO_PROXY"
                | "http_proxy"
                | "https_proxy"
                | "all_proxy"
                | "no_proxy"
                | "SSL_CERT_FILE"
                | "SSL_CERT_DIR"
                | "DENO_CERT"
                | "DENO_TLS_CA_STORE"
        )
}
