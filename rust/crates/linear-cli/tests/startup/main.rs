#![cfg(unix)]
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io;
#[cfg(target_os = "linux")]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use linear_cli::auth::file::{CredentialFileSource, CredentialReadFailure};
use linear_cli::auth::keyring::KeyringReader;
use linear_cli::auth::{CredentialWarning, LookupFailureCategory, LookupResult};
use linear_cli::config::{
    FileKind, FileSource, GitProbeResult, GitRootProbe, OsFamily, ProcessEnvSnapshot,
};
use linear_cli::startup::{AppStartupDiagnostic, AppStartupError, load, load_with_phase_timeout};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Sandbox(PathBuf);

impl Sandbox {
    fn new() -> Self {
        let number = NEXT.fetch_add(1, Ordering::Relaxed);
        let root =
            std::env::temp_dir().join(format!("linear-startup-{}-{number}", std::process::id()));
        for name in ["cwd", "home/linear", "bin"] {
            fs::create_dir_all(root.join(name)).unwrap();
        }
        Self(root)
    }

    fn process(&self, extras: &[(&str, &str)]) -> ProcessEnvSnapshot {
        let mut variables = BTreeMap::from([
            (OsString::from("HOME"), self.0.join("home").into_os_string()),
            (
                OsString::from("XDG_CONFIG_HOME"),
                self.0.join("home").into_os_string(),
            ),
            (
                OsString::from("LINEAR_IGNORE_ENV_FILE"),
                OsString::from("1"),
            ),
        ]);
        variables.extend(
            extras
                .iter()
                .map(|(key, value)| (OsString::from(key), OsString::from(value))),
        );
        ProcessEnvSnapshot::from_vars_os(self.0.join("cwd"), OsFamily::Unix, variables).unwrap()
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_linear"));
        command
            .env_clear()
            .current_dir(self.0.join("cwd"))
            .env("HOME", self.0.join("home"))
            .env("XDG_CONFIG_HOME", self.0.join("home"))
            .env("PATH", self.0.join("bin"))
            .env("LINEAR_IGNORE_ENV_FILE", "1")
            .env("NO_COLOR", "1");
        command
    }

    fn write_credentials(&self, text: &[u8]) {
        fs::write(self.0.join("home/linear/credentials.toml"), text).unwrap();
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

struct EmptyConfig;
impl FileSource for EmptyConfig {
    fn kind(&self, _path: &Path) -> io::Result<Option<FileKind>> {
        Ok(None)
    }
    fn read_bounded(&self, _path: &Path, _max_bytes: u64) -> io::Result<Vec<u8>> {
        Err(io::Error::from(io::ErrorKind::NotFound))
    }
}
struct InvalidEnv;
impl FileSource for InvalidEnv {
    fn kind(&self, path: &Path) -> io::Result<Option<FileKind>> {
        Ok(path
            .file_name()
            .is_some_and(|name| name == ".env")
            .then_some(FileKind::Regular))
    }
    fn read_bounded(&self, _path: &Path, _max_bytes: u64) -> io::Result<Vec<u8>> {
        Ok(vec![0xff])
    }
}
struct NoGit;
impl GitRootProbe for NoGit {
    fn probe(&self) -> GitProbeResult {
        GitProbeResult::SpawnFailure
    }
}

struct FakeCredentials {
    content: Option<Vec<u8>>,
    reads: AtomicUsize,
}
impl FakeCredentials {
    fn new(content: Option<&str>) -> Self {
        Self {
            content: content.map(str::as_bytes).map(Vec::from),
            reads: AtomicUsize::new(0),
        }
    }
}
impl CredentialFileSource for FakeCredentials {
    fn read_credentials(&self, _path: &Path) -> Result<Option<Vec<u8>>, CredentialReadFailure> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        Ok(self.content.clone())
    }
}

#[derive(Default)]
struct FakeKeyring {
    calls: Mutex<Vec<String>>,
    active: AtomicUsize,
    peak: AtomicUsize,
    delay: Duration,
    gate: Option<Arc<Gate>>,
}

struct Gate {
    target: usize,
    arrived: Mutex<usize>,
    ready: Condvar,
}

impl Gate {
    fn new(target: usize) -> Arc<Self> {
        Arc::new(Self {
            target,
            arrived: Mutex::new(0),
            ready: Condvar::new(),
        })
    }

    fn wait(&self) {
        let mut arrived = self.arrived.lock().unwrap();
        if *arrived < self.target {
            *arrived += 1;
        }
        if *arrived == self.target {
            self.ready.notify_all();
            return;
        }
        let (arrived, outcome) = self
            .ready
            .wait_timeout_while(arrived, Duration::from_secs(2), |arrived| {
                *arrived < self.target
            })
            .unwrap();
        assert!(
            !outcome.timed_out(),
            "only {arrived} lookup workers arrived"
        );
    }
}
impl KeyringReader for FakeKeyring {
    fn lookup(&self, workspace: &str) -> LookupResult {
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(active, Ordering::SeqCst);
        if let Some(gate) = &self.gate {
            gate.wait();
        }
        if workspace == "first" {
            std::thread::sleep(Duration::from_millis(40));
        }
        if !self.delay.is_zero() {
            std::thread::sleep(self.delay);
        }
        self.calls.lock().unwrap().push(workspace.to_owned());
        self.active.fetch_sub(1, Ordering::SeqCst);
        LookupResult::Miss
    }
}

#[test]
fn config_failure_precedes_credentials_read() {
    let sandbox = Sandbox::new();
    let process = sandbox.process(&[("LINEAR_IGNORE_ENV_FILE", "0")]);
    let credentials = FakeCredentials::new(Some("broken = ["));
    let report = load(
        &process,
        &InvalidEnv,
        &NoGit,
        &credentials,
        &FakeKeyring::default(),
    );
    assert!(matches!(report.result, Err(AppStartupError::Config(_))));
    assert_eq!(credentials.reads.load(Ordering::Relaxed), 0);
}

#[test]
fn inline_credentials_skip_keyring_and_are_available_to_commands() {
    let sandbox = Sandbox::new();
    let credentials = FakeCredentials::new(Some("default = 'demo'\ndemo = 'lin_api_fake_demo'\n"));
    let keyring = FakeKeyring::default();
    let report = load(
        &sandbox.process(&[]),
        &EmptyConfig,
        &NoGit,
        &credentials,
        &keyring,
    );
    let loaded = report.result.unwrap();
    assert_eq!(loaded.credentials.default(), Some("demo"));
    assert_eq!(
        loaded.credentials.key("demo").unwrap().expose(),
        "lin_api_fake_demo"
    );
    assert!(keyring.calls.lock().unwrap().is_empty());
}

#[test]
fn metadata_warning_order_is_manifest_order_despite_completion_order() {
    let sandbox = Sandbox::new();
    let credentials = FakeCredentials::new(Some(
        "default = 'missing'\nworkspaces = ['first', 'second']\n",
    ));
    let keyring = FakeKeyring {
        gate: Some(Gate::new(2)),
        ..FakeKeyring::default()
    };
    let report = load(
        &sandbox.process(&[]),
        &EmptyConfig,
        &NoGit,
        &credentials,
        &keyring,
    );
    assert!(report.result.is_ok());
    let warnings = report
        .diagnostics
        .into_iter()
        .filter_map(|diagnostic| match diagnostic {
            AppStartupDiagnostic::Credential(warning) => Some(warning),
            AppStartupDiagnostic::Config(_) => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        warnings,
        vec![
            CredentialWarning::InvalidDefault {
                workspace: "missing".to_owned()
            },
            CredentialWarning::LookupMiss {
                workspace: "first".to_owned()
            },
            CredentialWarning::LookupMiss {
                workspace: "second".to_owned()
            },
        ]
    );
    assert_eq!(
        *keyring.calls.lock().unwrap(),
        ["second".to_owned(), "first".to_owned()]
    );
}

#[test]
fn at_most_eight_lookup_workers_and_phase_timeout_marks_unstarted() {
    let sandbox = Sandbox::new();
    let names = (0..20).map(|n| format!("w{n}")).collect::<Vec<_>>();
    let list = names
        .iter()
        .map(|name| format!("'{name}'"))
        .collect::<Vec<_>>()
        .join(", ");
    let credentials = FakeCredentials::new(Some(&format!("workspaces = [{list}]\n")));
    let keyring = FakeKeyring {
        delay: Duration::from_millis(20),
        ..FakeKeyring::default()
    };
    let report = load_with_phase_timeout(
        &sandbox.process(&[]),
        &EmptyConfig,
        &NoGit,
        &credentials,
        &keyring,
        Duration::from_millis(1),
    );
    assert!(report.result.is_ok());
    assert!(keyring.peak.load(Ordering::SeqCst) <= 8);
    assert_eq!(report.diagnostics.len(), 20);
    assert!(report.diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        AppStartupDiagnostic::Credential(CredentialWarning::LookupFailed {
            category: LookupFailureCategory::Other,
            ..
        })
    )));
    assert!(report.diagnostics.iter().all(|diagnostic| matches!(
        diagnostic,
        AppStartupDiagnostic::Credential(CredentialWarning::LookupFailed {
            category: LookupFailureCategory::Other,
            ..
        }) | AppStartupDiagnostic::Credential(CredentialWarning::LookupMiss { .. })
    )));
}

#[test]
fn bounded_pool_requests_every_workspace_once_and_returns_ordered_warnings() {
    let sandbox = Sandbox::new();
    let names = (0..24).map(|n| format!("w{n}")).collect::<Vec<_>>();
    let list = names
        .iter()
        .map(|name| format!("'{name}'"))
        .collect::<Vec<_>>()
        .join(", ");
    let credentials = FakeCredentials::new(Some(&format!("workspaces = [{list}]\n")));
    let keyring = FakeKeyring {
        delay: Duration::from_millis(5),
        gate: Some(Gate::new(8)),
        ..FakeKeyring::default()
    };
    let report = load(
        &sandbox.process(&[]),
        &EmptyConfig,
        &NoGit,
        &credentials,
        &keyring,
    );
    assert!(report.result.is_ok());
    let mut calls = keyring.calls.lock().unwrap().clone();
    calls.sort();
    let mut expected = names.clone();
    expected.sort();
    assert_eq!(calls, expected);
    assert_eq!(keyring.peak.load(Ordering::SeqCst), 8);
    assert_eq!(
        report
            .diagnostics
            .into_iter()
            .map(|diagnostic| match diagnostic {
                AppStartupDiagnostic::Credential(CredentialWarning::LookupMiss { workspace }) => {
                    workspace
                }
                other => panic!("unexpected diagnostic: {other:?}"),
            })
            .collect::<Vec<_>>(),
        names
    );
}

#[test]
fn actual_binary_loads_inline_and_rejects_malformed_before_help() {
    let sandbox = Sandbox::new();
    let absent = sandbox.command().arg("--version").output().unwrap();
    assert!(absent.status.success());
    sandbox.write_credentials(b"default = 'demo'\ndemo = 'lin_api_fake_demo'\n");
    let inline = sandbox.command().arg("--help").output().unwrap();
    assert!(inline.status.success());
    assert!(inline.stderr.is_empty());
    sandbox.write_credentials(b"default = [");
    let malformed = sandbox.command().arg("--help").output().unwrap();
    assert_eq!(malformed.status.code(), Some(1));
    assert!(malformed.stdout.is_empty());
    assert!(
        String::from_utf8(malformed.stderr)
            .unwrap()
            .contains("invalid TOML")
    );
    sandbox.write_credentials(b"\xef\xbb\xbfdefault = 'demo'\n");
    let bom = sandbox.command().arg("--help").output().unwrap();
    assert_eq!(bom.status.code(), Some(1));
    assert!(
        String::from_utf8(bom.stderr)
            .unwrap()
            .contains("byte-order mark")
    );
}

#[cfg(target_os = "linux")]
#[test]
fn actual_binary_keyring_child_inherits_process_env_without_dotenv_overlay() {
    let sandbox = Sandbox::new();
    sandbox.write_credentials(b"workspaces = ['demo']\n");
    fs::write(
        sandbox.0.join("cwd/.env"),
        b"LINEAR_API_KEY=lin_api_dotenv_only\n",
    )
    .unwrap();
    let tool = sandbox.0.join("bin/secret-tool");
    fs::write(
        &tool,
        b"#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$TRACE\"\nif IFS= read -r line; then exit 9; fi\nif [ \"${LINEAR_API_KEY+x}\" = x ]; then exit 10; fi\nprintf '%s' \"$MARKER\"\n",
    )
    .unwrap();
    fs::set_permissions(&tool, fs::Permissions::from_mode(0o700)).unwrap();
    let trace = sandbox.0.join("trace");
    let output = sandbox
        .command()
        .env("LINEAR_IGNORE_ENV_FILE", "0")
        .env("TRACE", &trace)
        .env("MARKER", "private-process-marker")
        .arg("--help")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(
        fs::read_to_string(trace).unwrap(),
        "lookup\nservice\nlinear-cli\naccount\ndemo\n"
    );
}

// Linux-only private PATH probe; global Mac readers use injected fake tests,
// never the actual /usr/bin/security service from portable startup checks.
#[cfg(target_os = "linux")]
#[test]
fn actual_binary_metadata_warns_with_private_missing_linux_tool() {
    let sandbox = Sandbox::new();
    sandbox.write_credentials(b"default = 'missing'\nworkspaces = ['demo']\n");
    let metadata = sandbox.command().arg("--version").output().unwrap();
    assert!(metadata.status.success());
    let stderr = String::from_utf8(metadata.stderr).unwrap();
    assert!(stderr.contains("Default workspace \"missing\""));
    let expected = "keyring tool unavailable";
    assert!(stderr.contains(&format!(
        "Failed to read keyring for workspace \"demo\": {expected}"
    )));
}
