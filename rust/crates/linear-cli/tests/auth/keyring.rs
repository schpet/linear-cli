use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use linear_cli::auth::keyring::{ProcessKeyringReader, ProcessLookupFailure, ReaderFlavor};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Sandbox {
    root: PathBuf,
    executable: PathBuf,
}

impl Sandbox {
    fn new(script: &str) -> Self {
        let number = NEXT.fetch_add(1, Ordering::Relaxed);
        let root =
            std::env::temp_dir().join(format!("linear-keyring-{}-{number}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let executable = root.join("secret-tool");
        fs::write(&executable, format!("#!/bin/sh\n{script}\n")).unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        Self { root, executable }
    }

    fn reader(&self, timeout: Duration) -> ProcessKeyringReader {
        let environment = BTreeMap::from([
            (OsString::from("PATH"), OsString::from(&self.root)),
            (
                OsString::from("TRACE"),
                self.root.join("trace").into_os_string(),
            ),
            (OsString::from("MARKER"), OsString::from("private-marker")),
        ]);
        ProcessKeyringReader::with_test_environment(
            ReaderFlavor::SecretTool,
            self.executable.clone().into_os_string(),
            environment,
            timeout,
        )
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn exact_lookup_argv_null_stdin_and_complete_private_environment() {
    let sandbox = Sandbox::new(
        "printf '%s\\n' \"$@\" > \"$TRACE\"; if IFS= read -r line; then exit 8; fi; printf '%s' \"$MARKER\"",
    );
    let key = sandbox
        .reader(Duration::from_secs(2))
        .lookup_detailed("demo")
        .unwrap()
        .unwrap();
    assert_eq!(key.expose(), "private-marker");
    assert_eq!(
        fs::read_to_string(sandbox.root.join("trace")).unwrap(),
        "lookup\nservice\nlinear-cli\naccount\ndemo\n"
    );
}

#[test]
fn success_ignores_stderr_and_keeps_stdout_verbatim_except_one_bom() {
    let sandbox = Sandbox::new("printf 'noisy' >&2; printf '\\357\\273\\277line\\n'");
    let key = sandbox
        .reader(Duration::from_secs(2))
        .lookup_detailed("demo")
        .unwrap()
        .unwrap();
    assert_eq!(key.expose(), "line\n");
}

#[test]
fn empty_output_and_exit_one_with_js_trimmed_stderr_are_misses() {
    let empty = Sandbox::new("exit 0");
    assert!(
        empty
            .reader(Duration::from_secs(2))
            .lookup_detailed("demo")
            .unwrap()
            .is_none()
    );
    let miss = Sandbox::new("printf '\\357\\273\\277' >&2; exit 1");
    assert!(
        miss.reader(Duration::from_secs(2))
            .lookup_detailed("demo")
            .unwrap()
            .is_none()
    );
    let not_trimmed = Sandbox::new("printf '\\302\\205' >&2; exit 1");
    assert_eq!(
        not_trimmed
            .reader(Duration::from_secs(2))
            .lookup_detailed("demo")
            .unwrap_err(),
        ProcessLookupFailure::ExitFailure
    );
    let ignored_stdout = Sandbox::new("printf '\\377'; exit 1");
    assert!(
        ignored_stdout
            .reader(Duration::from_secs(2))
            .lookup_detailed("demo")
            .unwrap()
            .is_none()
    );
}

#[test]
fn errors_are_typed_and_do_not_include_child_output() {
    let failed = Sandbox::new("printf 'lin_api_fake_secret' >&2; exit 1");
    let error = failed
        .reader(Duration::from_secs(2))
        .lookup_detailed("demo")
        .unwrap_err();
    assert_eq!(error, ProcessLookupFailure::ExitFailure);
    assert!(!format!("{error:?}").contains("lin_api_fake_secret"));
    let invalid = Sandbox::new("printf '\\377'");
    assert_eq!(
        invalid
            .reader(Duration::from_secs(2))
            .lookup_detailed("demo")
            .unwrap_err(),
        ProcessLookupFailure::InvalidUtf8
    );
    let missing = ProcessKeyringReader::with_test_environment(
        ReaderFlavor::SecretTool,
        OsString::from("/definitely/absent/secret-tool"),
        BTreeMap::new(),
        Duration::from_secs(1),
    );
    assert_eq!(
        missing.lookup_detailed("demo").unwrap_err(),
        ProcessLookupFailure::Spawn(std::io::ErrorKind::NotFound)
    );
    let other_exit = Sandbox::new("exit 4");
    assert_eq!(
        other_exit
            .reader(Duration::from_secs(2))
            .lookup_detailed("demo")
            .unwrap_err(),
        ProcessLookupFailure::ExitFailure
    );
    let denied = Sandbox::new("exit 0");
    fs::set_permissions(&denied.executable, fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(
        denied
            .reader(Duration::from_secs(2))
            .lookup_detailed("demo")
            .unwrap_err(),
        ProcessLookupFailure::Spawn(std::io::ErrorKind::PermissionDenied)
    );
}

#[test]
fn stdout_cap_and_deadline_stop_and_reap_child() {
    let exact = Sandbox::new("/usr/bin/head -c 65536 /dev/zero");
    assert_eq!(
        exact
            .reader(Duration::from_secs(3))
            .lookup_detailed("demo")
            .unwrap()
            .unwrap()
            .expose()
            .len(),
        65536
    );
    let oversized = Sandbox::new("/usr/bin/head -c 65537 /dev/zero");
    assert_eq!(
        oversized
            .reader(Duration::from_secs(3))
            .lookup_detailed("demo")
            .unwrap_err(),
        ProcessLookupFailure::StdoutTooLarge
    );
    let hung = Sandbox::new("printf '%s' \"$$\" > \"$TRACE\"; exec /bin/sleep 2");
    assert_eq!(
        hung.reader(Duration::from_millis(250))
            .lookup_detailed("demo")
            .unwrap_err(),
        ProcessLookupFailure::Timeout
    );
    let pid = fs::read_to_string(hung.root.join("trace")).unwrap();
    assert!(
        !Command::new("/bin/kill")
            .arg("-0")
            .arg(pid)
            .output()
            .unwrap()
            .status
            .success()
    );
    let stderr_flood = Sandbox::new("/usr/bin/head -c 16385 /dev/zero >&2");
    assert_eq!(
        stderr_flood
            .reader(Duration::from_secs(3))
            .lookup_detailed("demo")
            .unwrap_err(),
        ProcessLookupFailure::StderrTooLarge
    );
    let stderr_exact = Sandbox::new("/usr/bin/head -c 16384 /dev/zero >&2");
    assert!(
        stderr_exact
            .reader(Duration::from_secs(3))
            .lookup_detailed("demo")
            .unwrap()
            .is_none()
    );
}

#[test]
fn exited_child_with_descendant_holding_pipe_returns_within_grace() {
    let sandbox = Sandbox::new("/bin/sleep 3 & printf '%s' \"$!\" > \"$TRACE\"; exit 0");
    let started = std::time::Instant::now();
    let result = sandbox
        .reader(Duration::from_secs(2))
        .lookup_detailed("demo");
    let pid = fs::read_to_string(sandbox.root.join("trace")).unwrap();
    let pid: i32 = pid.parse().unwrap();
    let _ = Command::new("/bin/kill").arg(pid.to_string()).status();
    assert_eq!(result.unwrap_err(), ProcessLookupFailure::PipeHeldOpen);
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[path = "mac_reader.rs"]
mod mac_reader;
