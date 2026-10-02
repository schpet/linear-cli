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
        let trace = root.join("trace");
        fs::write(
            &executable,
            format!(
                "#!/bin/sh\nTRACE='{}'\nMARKER=private-marker\n{script}\n",
                trace.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        Self { root, executable }
    }

    fn reader(&self, timeout: Duration) -> ProcessKeyringReader {
        ProcessKeyringReader::with_executable(
            ReaderFlavor::SecretTool,
            self.executable.clone().into_os_string(),
        )
        .with_timeout(timeout)
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn exact_lookup_argv_and_null_stdin() {
    let sandbox = Sandbox::new(
        "printf '%s\\n' \"$@\" > \"$TRACE\"; if IFS= read -r line; then exit 8; fi; printf '%s' \"$MARKER\"",
    );
    let key = sandbox
        .reader(Duration::from_secs(10))
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
fn success_ignores_stderr_and_keeps_stdout_verbatim() {
    let sandbox = Sandbox::new("printf 'noisy' >&2; printf 'line\\n'");
    let key = sandbox
        .reader(Duration::from_secs(10))
        .lookup_detailed("demo")
        .unwrap()
        .unwrap();
    assert_eq!(key.expose(), "line\n");
}

#[test]
fn empty_output_and_exit_one_with_blank_stderr_are_misses() {
    let empty = Sandbox::new("exit 0");
    assert!(
        empty
            .reader(Duration::from_secs(10))
            .lookup_detailed("demo")
            .unwrap()
            .is_none()
    );
    let miss = Sandbox::new("printf ' \\n' >&2; exit 1");
    assert!(
        miss.reader(Duration::from_secs(10))
            .lookup_detailed("demo")
            .unwrap()
            .is_none()
    );
    let failure = Sandbox::new("printf 'denied' >&2; exit 1");
    assert_eq!(
        failure
            .reader(Duration::from_secs(10))
            .lookup_detailed("demo")
            .unwrap_err(),
        ProcessLookupFailure::ExitFailure
    );
    let ignored_stdout = Sandbox::new("printf '\\377'; exit 1");
    assert!(
        ignored_stdout
            .reader(Duration::from_secs(10))
            .lookup_detailed("demo")
            .unwrap()
            .is_none()
    );
}

#[test]
fn errors_are_typed_and_do_not_include_child_output() {
    let failed = Sandbox::new("printf 'lin_api_fake_secret' >&2; exit 1");
    let error = failed
        .reader(Duration::from_secs(10))
        .lookup_detailed("demo")
        .unwrap_err();
    assert_eq!(error, ProcessLookupFailure::ExitFailure);
    assert!(!format!("{error:?}").contains("lin_api_fake_secret"));
    let invalid = Sandbox::new("printf '\\377'");
    assert_eq!(
        invalid
            .reader(Duration::from_secs(10))
            .lookup_detailed("demo")
            .unwrap_err(),
        ProcessLookupFailure::InvalidUtf8
    );
    let missing = ProcessKeyringReader::with_executable(
        ReaderFlavor::SecretTool,
        "/definitely/absent/secret-tool".into(),
    );
    assert_eq!(
        missing.lookup_detailed("demo").unwrap_err(),
        ProcessLookupFailure::Spawn(std::io::ErrorKind::NotFound)
    );
    let other_exit = Sandbox::new("exit 4");
    assert_eq!(
        other_exit
            .reader(Duration::from_secs(10))
            .lookup_detailed("demo")
            .unwrap_err(),
        ProcessLookupFailure::ExitFailure
    );
    let denied = Sandbox::new("exit 0");
    fs::set_permissions(&denied.executable, fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(
        denied
            .reader(Duration::from_secs(10))
            .lookup_detailed("demo")
            .unwrap_err(),
        ProcessLookupFailure::Spawn(std::io::ErrorKind::PermissionDenied)
    );
}

#[test]
fn deadline_kills_a_hung_child() {
    let hung = Sandbox::new("printf '%s' \"$$\" > \"$TRACE\"; exec /bin/sleep 30");
    assert_eq!(
        hung.reader(Duration::from_secs(3))
            .lookup_detailed("demo")
            .unwrap_err(),
        ProcessLookupFailure::Timeout
    );
    let pid = fs::read_to_string(hung.root.join("trace"))
        .expect("fake child started and published PID before its fixture deadline");
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while Command::new("/bin/kill")
        .arg("-0")
        .arg(&pid)
        .output()
        .unwrap()
        .status
        .success()
    {
        assert!(std::time::Instant::now() < deadline, "child {pid} survived");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn output_held_open_by_a_descendant_is_read_up_to_the_exit() {
    let sandbox =
        Sandbox::new("/bin/sleep 3 & printf '%s' \"$!\" > \"$TRACE\"; printf 'key'; exit 0");
    let started = std::time::Instant::now();
    let result = sandbox
        .reader(Duration::from_secs(10))
        .lookup_detailed("demo");
    let pid = fs::read_to_string(sandbox.root.join("trace")).unwrap();
    let _ = Command::new("/bin/kill").arg(pid).status();
    assert_eq!(result.unwrap().unwrap().expose(), "key");
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[path = "mac_reader.rs"]
mod mac_reader;
