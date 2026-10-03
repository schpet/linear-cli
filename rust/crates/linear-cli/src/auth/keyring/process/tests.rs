//! Each test runs a private fake keyring tool, never the real one.
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use super::*;

/// A shell script standing in for the keyring tool. `$TRACE` names a file
/// the script may write for the test to inspect.
struct FakeTool {
    dir: tempfile::TempDir,
}

impl FakeTool {
    fn new(script: &str) -> Self {
        let dir = tempfile::tempdir().expect("temp dir");
        let tool = Self { dir };
        fs::write(
            tool.executable(),
            format!(
                "#!/bin/sh\nTRACE='{}'\n{script}\n",
                tool.trace_path().display()
            ),
        )
        .expect("write fake tool");
        fs::set_permissions(tool.executable(), fs::Permissions::from_mode(0o700))
            .expect("make fake tool executable");
        tool
    }

    fn executable(&self) -> PathBuf {
        self.dir.path().join("tool")
    }

    fn trace_path(&self) -> PathBuf {
        self.dir.path().join("trace")
    }

    fn trace(&self) -> String {
        fs::read_to_string(self.trace_path()).expect("fake tool wrote its trace")
    }

    fn stdin(&self) -> String {
        fs::read_to_string(self.dir.path().join("trace.stdin")).expect("fake tool saved stdin")
    }

    fn reader(&self, flavor: ReaderFlavor) -> ProcessKeyringReader {
        ProcessKeyringReader::with_executable(flavor, self.executable().into_os_string())
            .with_timeout(Duration::from_secs(10))
    }

    fn lookup(&self, flavor: ReaderFlavor) -> Result<Option<ConfigSecret>, ProcessLookupFailure> {
        self.reader(flavor).lookup_detailed("demo")
    }

    fn backend(&self, flavor: ReaderFlavor) -> ProcessMutationBackend {
        ProcessMutationBackend::with_executable(
            flavor,
            self.executable().into_os_string(),
            ChildEnvOverlay::empty(),
        )
    }
}

fn found(result: Result<Option<ConfigSecret>, ProcessLookupFailure>) -> String {
    result
        .expect("lookup succeeds")
        .expect("key found")
        .expose()
        .to_owned()
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(future)
}

const RECORD_ARGS_AND_STDIN: &str =
    "printf '%s\\n' \"$@\" > \"$TRACE\"; /bin/cat > \"$TRACE.stdin\"";

#[test]
fn secret_tool_lookup_passes_arguments_and_no_stdin() {
    let tool = FakeTool::new(
        "printf '%s\\n' \"$@\" > \"$TRACE\"; if IFS= read -r line; then exit 8; fi; printf 'key'",
    );
    assert_eq!(found(tool.lookup(ReaderFlavor::SecretTool)), "key");
    assert_eq!(tool.trace(), "lookup\nservice\nlinear-cli\naccount\ndemo\n");
}

#[test]
fn secret_tool_output_is_kept_verbatim_and_stderr_ignored() {
    let tool = FakeTool::new("printf 'noisy' >&2; printf 'line\\n'");
    assert_eq!(found(tool.lookup(ReaderFlavor::SecretTool)), "line\n");
}

#[test]
fn secret_tool_misses_and_failures() {
    let lookup = |script| FakeTool::new(script).lookup(ReaderFlavor::SecretTool);
    assert_eq!(lookup("exit 0"), Ok(None));
    // Exit 1 with nothing on stderr is how secret-tool reports no match.
    assert_eq!(lookup("printf ' \\n' >&2; exit 1"), Ok(None));
    assert_eq!(lookup("printf '\\377'; exit 1"), Ok(None));
    assert_eq!(
        lookup("printf 'denied' >&2; exit 1"),
        Err(ProcessLookupFailure::ExitFailure)
    );
    assert_eq!(lookup("exit 4"), Err(ProcessLookupFailure::ExitFailure));
    assert_eq!(
        lookup("printf '\\377'"),
        Err(ProcessLookupFailure::InvalidUtf8)
    );
}

#[test]
fn security_lookup_trims_the_key_and_treats_exit_44_as_a_miss() {
    let tool = FakeTool::new(
        "printf '%s\\n' \"$@\" > \"$TRACE\"; if IFS= read -r line; then exit 8; fi; printf ' \\tkey\\r\\n'",
    );
    assert_eq!(found(tool.lookup(ReaderFlavor::MacSecurity)), "key");
    assert_eq!(
        tool.trace(),
        "find-generic-password\n-a\ndemo\n-s\nlinear-cli\n-w\n"
    );

    let lookup = |script| FakeTool::new(script).lookup(ReaderFlavor::MacSecurity);
    assert_eq!(
        lookup("printf '\\377' >&2; printf ignored; exit 44"),
        Ok(None)
    );
    assert_eq!(lookup("printf ' \\t\\r\\n'"), Ok(None));
    assert_eq!(lookup("exit 1"), Err(ProcessLookupFailure::ExitFailure));
    assert_eq!(
        lookup("printf '\\377'"),
        Err(ProcessLookupFailure::InvalidUtf8)
    );
}

#[test]
fn failures_never_carry_the_tool_output() {
    let error = FakeTool::new("printf 'lin_api_fake_secret' >&2; exit 5")
        .lookup(ReaderFlavor::MacSecurity)
        .expect_err("nonzero exit");
    assert_eq!(error, ProcessLookupFailure::ExitFailure);
    assert!(!format!("{error:?}").contains("lin_api_fake_secret"));
}

#[test]
fn spawn_failures_are_classified() {
    let missing = ProcessKeyringReader::with_executable(
        ReaderFlavor::SecretTool,
        "/definitely/absent/secret-tool".into(),
    );
    assert_eq!(
        missing.lookup_detailed("demo"),
        Err(ProcessLookupFailure::Spawn(std::io::ErrorKind::NotFound))
    );
    let denied = FakeTool::new("exit 0");
    fs::set_permissions(denied.executable(), fs::Permissions::from_mode(0o600))
        .expect("drop execute permission");
    assert_eq!(
        denied.lookup(ReaderFlavor::SecretTool),
        Err(ProcessLookupFailure::Spawn(
            std::io::ErrorKind::PermissionDenied
        ))
    );
}

#[test]
fn a_hung_tool_is_killed_at_the_deadline() {
    let hung = FakeTool::new("printf '%s' \"$$\" > \"$TRACE\"; exec /bin/sleep 30");
    assert_eq!(
        hung.reader(ReaderFlavor::SecretTool)
            .with_timeout(Duration::from_secs(3))
            .lookup_detailed("demo"),
        Err(ProcessLookupFailure::Timeout)
    );
    let pid = hung.trace();
    let deadline = Instant::now() + Duration::from_secs(10);
    while Command::new("/bin/kill")
        .arg("-0")
        .arg(&pid)
        .output()
        .expect("run kill")
        .status
        .success()
    {
        assert!(Instant::now() < deadline, "child {pid} survived");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn output_held_open_by_a_descendant_does_not_delay_the_result() {
    let tool =
        FakeTool::new("/bin/sleep 3 & printf '%s' \"$!\" > \"$TRACE\"; printf 'key'; exit 0");
    let started = Instant::now();
    let result = tool.lookup(ReaderFlavor::SecretTool);
    let _ = Command::new("/bin/kill").arg(tool.trace()).status();
    assert_eq!(found(result), "key");
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn security_store_sends_the_command_on_stdin() {
    let tool = FakeTool::new(RECORD_ARGS_AND_STDIN);
    block_on(
        tool.backend(ReaderFlavor::MacSecurity)
            .store("demo", &ConfigSecret::new("lin_secret".to_owned())),
    )
    .expect("store");
    assert_eq!(tool.trace(), "-i\n");
    assert_eq!(
        tool.stdin(),
        "add-generic-password -U -a demo -s linear-cli -w lin_secret\n"
    );
}

#[test]
fn security_store_refuses_values_that_would_need_quoting() {
    let tool = FakeTool::new("printf 'ran' > \"$TRACE\"");
    let error = block_on(
        tool.backend(ReaderFlavor::MacSecurity)
            .store("demo", &ConfigSecret::new("two words".to_owned())),
    )
    .expect_err("unquotable");
    assert!(error.to_string().contains("keychain tool"), "{error}");
    assert!(!tool.trace_path().exists());
}

#[test]
fn security_delete_of_a_missing_entry_succeeds() {
    let tool = FakeTool::new("printf '%s\\n' \"$@\" > \"$TRACE\"; exit 44");
    block_on(tool.backend(ReaderFlavor::MacSecurity).delete("demo")).expect("delete");
    assert_eq!(
        tool.trace(),
        "delete-generic-password\n-a\ndemo\n-s\nlinear-cli\n"
    );
}

#[test]
fn secret_tool_store_reads_the_secret_from_stdin() {
    let tool = FakeTool::new(RECORD_ARGS_AND_STDIN);
    block_on(
        tool.backend(ReaderFlavor::SecretTool)
            .store("demo", &ConfigSecret::new("lin_secret".to_owned())),
    )
    .expect("store");
    assert_eq!(
        tool.trace(),
        "store\n--label\nlinear-cli: demo\nservice\nlinear-cli\naccount\ndemo\n"
    );
    assert_eq!(tool.stdin(), "lin_secret");
}
