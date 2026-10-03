//! Each test runs a private fake keyring tool, never the real one.
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use super::*;
use crate::auth::keyring::Keyring;
use crate::auth::keyring::secret_tool::SecretTool;
use crate::auth::keyring::security::Security;

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

    fn tool(&self, name: &'static str) -> Tool {
        let mut tool = Tool::new(name, "unused", ChildEnvOverlay::empty());
        tool.executable = self.executable().into_os_string();
        tool.timeout = Duration::from_secs(30);
        tool
    }

    fn secret_tool(&self) -> SecretTool {
        SecretTool::with_tool(self.tool("secret-tool"))
    }

    fn security(&self) -> Security {
        Security::with_tool(self.tool("security"))
    }
}

/// What a lookup found, with the key spelled out.
fn shown(result: LookupResult) -> String {
    match result {
        LookupResult::Hit(key) => format!("hit {}", key.expose()),
        LookupResult::Miss => "miss".to_owned(),
        LookupResult::Failed(category) => format!("failed {category:?}"),
    }
}

const RECORD_ARGS_AND_STDIN: &str =
    "printf '%s\\n' \"$@\" > \"$TRACE\"; /bin/cat > \"$TRACE.stdin\"";

#[test]
fn secret_tool_lookup_passes_the_workspace_and_no_stdin() {
    let tool = FakeTool::new(
        "printf '%s\\n' \"$@\" > \"$TRACE\"; if IFS= read -r line; then exit 8; fi; printf 'key'",
    );
    assert_eq!(shown(tool.secret_tool().get("dummy space;'中")), "hit key");
    assert_eq!(
        tool.trace(),
        "lookup\nservice\nlinear-cli\naccount\ndummy space;'中\n"
    );
}

#[test]
fn printed_keys_are_trimmed_and_stderr_ignored() {
    let tool = FakeTool::new("printf 'noisy' >&2; printf ' \\tkey\\r\\n'");
    assert_eq!(shown(tool.secret_tool().get("demo")), "hit key");
    assert_eq!(shown(tool.security().get("demo")), "hit key");
}

#[test]
fn secret_tool_misses_and_failures() {
    let lookup = |script| shown(FakeTool::new(script).secret_tool().get("demo"));
    assert_eq!(lookup("exit 0"), "miss");
    assert_eq!(lookup("printf ' \\n'"), "miss");
    // Exit 1 with nothing on stderr is how secret-tool reports no match.
    assert_eq!(lookup("printf ' \\n' >&2; exit 1"), "miss");
    assert_eq!(lookup("printf '\\377'; exit 1"), "miss");
    assert_eq!(lookup("printf 'denied' >&2; exit 1"), "failed Other");
    assert_eq!(lookup("exit 4"), "failed Other");
    assert_eq!(lookup("printf '\\377'"), "failed Other");
}

#[test]
fn security_lookup_names_the_item_and_treats_exit_44_as_a_miss() {
    let tool = FakeTool::new(
        "printf '%s\\n' \"$@\" > \"$TRACE\"; if IFS= read -r line; then exit 8; fi; printf 'key\\n'",
    );
    assert_eq!(shown(tool.security().get("demo")), "hit key");
    assert_eq!(
        tool.trace(),
        "find-generic-password\n-a\ndemo\n-s\nlinear-cli\n-w\n"
    );

    let lookup = |script| shown(FakeTool::new(script).security().get("demo"));
    assert_eq!(
        lookup("printf '\\377' >&2; printf ignored; exit 44"),
        "miss"
    );
    assert_eq!(lookup("printf ' \\t\\r\\n'"), "miss");
    assert_eq!(lookup("exit 1"), "failed Other");
}

#[test]
fn failures_never_carry_the_tool_output() {
    let result = FakeTool::new("printf 'lin_api_fake_secret' >&2; exit 5")
        .security()
        .get("demo");
    assert!(!format!("{result:?}").contains("lin_api_fake_secret"));
}

#[test]
fn a_missing_or_unrunnable_tool_is_classified() {
    let mut missing = FakeTool::new("exit 0").tool("secret-tool");
    missing.executable = "/definitely/absent/secret-tool".into();
    let missing = SecretTool::with_tool(missing);
    assert_eq!(shown(missing.get("demo")), "failed Unavailable");
    assert!(!missing.available());
    let error = missing
        .set("demo", &ConfigSecret::new("lin_secret".to_owned()))
        .expect_err("cannot run");
    assert!(error.to_string().contains("Could not run"), "{error}");
    assert!(error.hint().is_some_and(|hint| hint.contains("libsecret")));

    let denied = FakeTool::new("exit 0");
    fs::set_permissions(denied.executable(), fs::Permissions::from_mode(0o600))
        .expect("drop execute permission");
    assert_eq!(shown(denied.secret_tool().get("demo")), "failed Permission");
}

#[test]
fn a_hung_tool_is_killed_at_the_deadline() {
    let hung = FakeTool::new("printf '%s' \"$$\" > \"$TRACE\"; exec /bin/sleep 60");
    let mut tool = hung.tool("secret-tool");
    tool.timeout = Duration::from_secs(5);
    assert_eq!(
        shown(SecretTool::with_tool(tool).get("demo")),
        "failed Other"
    );
    let pid = hung.trace();
    let deadline = Instant::now() + Duration::from_secs(30);
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
        FakeTool::new("/bin/sleep 60 & printf '%s' \"$!\" > \"$TRACE\"; printf 'key'; exit 0");
    let started = Instant::now();
    let result = tool.secret_tool().get("demo");
    let _ = Command::new("/bin/kill").arg(tool.trace()).status();
    assert_eq!(shown(result), "hit key");
    assert!(started.elapsed() < Duration::from_secs(30));
}

#[test]
fn lookups_work_inside_an_async_runtime() {
    let tool = FakeTool::new("printf 'key'");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let result = runtime.block_on(async { tool.secret_tool().get("demo") });
    assert_eq!(shown(result), "hit key");
}

#[test]
fn security_store_sends_the_command_on_stdin() {
    let tool = FakeTool::new(RECORD_ARGS_AND_STDIN);
    tool.security()
        .set("acme-co", &ConfigSecret::new("lin_api_ab-1.2".to_owned()))
        .expect("store");
    assert_eq!(tool.trace(), "-i\n");
    assert_eq!(
        tool.stdin(),
        "add-generic-password -U -a acme-co -s linear-cli -w lin_api_ab-1.2\n"
    );
}

#[test]
fn security_store_refuses_values_that_would_need_quoting() {
    let tool = FakeTool::new("printf 'ran' > \"$TRACE\"");
    for (workspace, secret) in [("demo", "two words"), ("dummy space", "lin_secret")] {
        let error = tool
            .security()
            .set(workspace, &ConfigSecret::new(secret.to_owned()))
            .expect_err("unquotable");
        assert!(error.to_string().contains("keychain tool"), "{error}");
    }
    assert!(!tool.trace_path().exists());
}

#[test]
fn security_delete_of_a_missing_entry_succeeds() {
    let tool = FakeTool::new("printf '%s\\n' \"$@\" > \"$TRACE\"; exit 44");
    tool.security().delete("demo").expect("delete");
    assert_eq!(
        tool.trace(),
        "delete-generic-password\n-a\ndemo\n-s\nlinear-cli\n"
    );
}

#[test]
fn secret_tool_store_reads_the_secret_from_stdin() {
    let tool = FakeTool::new(RECORD_ARGS_AND_STDIN);
    tool.secret_tool()
        .set("demo", &ConfigSecret::new("dummy key\n中".to_owned()))
        .expect("store");
    assert_eq!(
        tool.trace(),
        "store\n--label\nlinear-cli: demo\nservice\nlinear-cli\naccount\ndemo\n"
    );
    assert_eq!(tool.stdin(), "dummy key\n中");
}

#[test]
fn secret_tool_clear_reports_the_exit_status_and_stderr() {
    let tool = FakeTool::new("printf '%s\\n' \"$@\" > \"$TRACE\"; echo 'locked' >&2; exit 4");
    let error = tool.secret_tool().delete("demo").expect_err("refused");
    assert_eq!(
        error.to_string(),
        "secret-tool clear failed (exit 4): locked"
    );
    assert_eq!(tool.trace(), "clear\nservice\nlinear-cli\naccount\ndemo\n");
}
