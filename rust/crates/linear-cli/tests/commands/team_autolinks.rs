#![cfg(unix)]

use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

struct Sandbox(PathBuf);
impl Sandbox {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "linear-autolinks-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        for name in ["bin", "config"] {
            fs::create_dir_all(root.join(name)).unwrap();
        }
        Self(fs::canonicalize(root).expect("canonical private autolinks sandbox"))
    }
    fn gh(&self, ending: &str) {
        let script = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > argv.txt\nprintf '%s\\n' \"$PWD\" > cwd.txt\nprintf 'LINEAR_TEAM_ID=%s\\nLINEAR_WORKSPACE=%s\\nGH_REPO=%s\\n' \"$LINEAR_TEAM_ID\" \"$LINEAR_WORKSPACE\" \"$GH_REPO\" > named-env.txt\n/bin/cat > stdin.bin\nprintf 'child stdout\\377\\n'\nprintf 'child stderr\\000\\n' >&2\nprintf 'controlled effect\\n' >> effect.txt\n{ending}\n"
        );
        let path = self.0.join("bin/gh");
        fs::write(&path, script).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_linear"));
        command
            .current_dir(&self.0)
            .env_clear()
            .env("HOME", &self.0)
            .env("XDG_CONFIG_HOME", self.0.join("config"))
            .env("APPDATA", self.0.join("config"))
            .env("PATH", self.0.join("bin"))
            .env("NO_COLOR", "1")
            .env("LINEAR_IGNORE_ENV_FILE", "1")
            .env("LINEAR_GRAPHQL_ENDPOINT", "http://127.0.0.1:1/graphql")
            .args(["team", "autolinks"]);
        command
    }
    fn configured(&self) -> Command {
        let mut command = self.command();
        command
            .env("LINEAR_TEAM_ID", "ab c")
            .env("LINEAR_WORKSPACE", "dummy-workspace");
        command
    }
    fn read(&self, name: &str) -> Vec<u8> {
        fs::read(self.0.join(name)).unwrap()
    }
    fn no_effect(&self) {
        assert!(!self.0.join("argv.txt").exists());
        assert!(!self.0.join("effect.txt").exists());
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

// Drain both descriptors concurrently and bound the real binary invocation.
fn run(mut command: Command, input: &[u8]) -> Output {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let mut stderr = child.stderr.take().unwrap();
    let err = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).unwrap();
        bytes
    });
    child.stdin.take().unwrap().write_all(input).unwrap();
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if start.elapsed() > Duration::from_secs(10) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("bounded autolinks invocation timed out");
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    Output {
        status,
        stdout: out.join().unwrap(),
        stderr: err.join().unwrap(),
    }
}

#[test]
fn success_preserves_shell_free_argv_cwd_binary_streams_and_exactly_one_effect() {
    let sandbox = Sandbox::new();
    sandbox.gh("exit 0");
    let mut command = sandbox.configured();
    command.args(["--workspace", "ignored-flag"]);
    let output = run(command, b"\0\xffstdin\n");
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"child stdout\xff\n");
    assert_eq!(output.stderr, b"child stderr\0\n");
    assert_eq!(sandbox.read("argv.txt"), b"api\nrepos/{owner}/{repo}/autolinks\n-f\nkey_prefix=AB C-\n-f\nurl_template=https://linear.app/dummy-workspace/issue/AB C-<num>\n");
    assert_eq!(
        sandbox.read("cwd.txt"),
        format!("{}\n", sandbox.0.display()).as_bytes()
    );
    assert_eq!(sandbox.read("stdin.bin"), b"\0\xffstdin\n");
    assert_eq!(sandbox.read("effect.txt"), b"controlled effect\n");
}

#[test]
fn exit7_and_child_sigterm_keep_partial_effect_and_double_action_failure_bytes() {
    for ending in ["exit 7", "kill -TERM \"$$\""] {
        let sandbox = Sandbox::new();
        sandbox.gh(ending);
        let output = run(sandbox.configured(), b"failure input\n");
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(output.stdout, b"child stdout\xff\n");
        assert_eq!(output.stderr, b"child stderr\0\n\xe2\x9c\x97 Failed to configure autolinks: Failed to configure autolinks\n");
        assert_eq!(sandbox.read("effect.txt"), b"controlled effect\n");
        assert_eq!(sandbox.read("stdin.bin"), b"failure input\n");
    }
}

#[test]
fn dotenv_child_overlay_and_process_gh_repo_precedence_match_source() {
    for process in [None, Some("dummy-process-owner/dummy-process-repo")] {
        let sandbox = Sandbox::new();
        sandbox.gh("exit 0");
        fs::write(sandbox.0.join(".env"), "LINEAR_TEAM_ID=dummy-team\nLINEAR_WORKSPACE=dummy-workspace\nGH_REPO=dummy-dotenv-owner/dummy-dotenv-repo\n").unwrap();
        let mut command = sandbox.command();
        command.env_remove("LINEAR_IGNORE_ENV_FILE");
        if let Some(value) = process {
            command.env("GH_REPO", value);
        }
        let output = run(command, b"dummy input\n");
        assert!(output.status.success(), "{:?}", output.stderr);
        assert_eq!(
            sandbox.read("named-env.txt"),
            format!(
                "LINEAR_TEAM_ID=dummy-team\nLINEAR_WORKSPACE=dummy-workspace\nGH_REPO={}\n",
                process.unwrap_or("dummy-dotenv-owner/dummy-dotenv-repo")
            )
            .as_bytes()
        );
        assert_eq!(sandbox.read("argv.txt"), b"api\nrepos/{owner}/{repo}/autolinks\n-f\nkey_prefix=DUMMY-TEAM-\n-f\nurl_template=https://linear.app/dummy-workspace/issue/DUMMY-TEAM-<num>\n");
    }
}

#[test]
fn missing_and_explicit_empty_config_validate_in_source_order_before_spawning() {
    for (file, team, workspace, expected) in [
        (
            None,
            None,
            None,
            "Could not determine team id from directory name\n  Run `linear config` to set a team.",
        ),
        (
            Some("team_id = \"file-team\"\nworkspace = \"file-workspace\"\n"),
            Some(""),
            None,
            "Could not determine team id from directory name\n  Run `linear config` to set a team.",
        ),
        (
            None,
            Some("dummy-team"),
            None,
            "workspace is not set via command line, configuration file, or environment",
        ),
        (
            Some("team_id = \"file-team\"\nworkspace = \"file-workspace\"\n"),
            None,
            Some(""),
            "workspace is not set via command line, configuration file, or environment",
        ),
    ] {
        let sandbox = Sandbox::new();
        sandbox.gh("exit 0");
        if let Some(contents) = file {
            fs::write(sandbox.0.join(".linear.toml"), contents).unwrap();
        }
        let mut command = sandbox.command();
        if let Some(value) = team {
            command.env("LINEAR_TEAM_ID", value);
        }
        if let Some(value) = workspace {
            command.env("LINEAR_WORKSPACE", value);
        }
        let output = run(command, b"");
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(
            output.stderr,
            format!("✗ Failed to configure autolinks: {expected}\n").as_bytes()
        );
        sandbox.no_effect();
    }
}

#[test]
fn project_file_configuration_and_environment_priority_work_without_linear_credentials() {
    let sandbox = Sandbox::new();
    sandbox.gh("exit 0");
    fs::write(
        sandbox.0.join(".linear.toml"),
        "team_id = \"file-team\"\nworkspace = \"file-workspace\"\n",
    )
    .unwrap();
    let mut command = sandbox.command();
    command.env("LINEAR_TEAM_ID", "eNv");
    let output = run(command, b"");
    assert!(output.status.success());
    assert_eq!(sandbox.read("argv.txt"), b"api\nrepos/{owner}/{repo}/autolinks\n-f\nkey_prefix=ENV-\n-f\nurl_template=https://linear.app/file-workspace/issue/ENV-<num>\n");
}

#[test]
fn missing_gh_preserves_exact_source_diagnostic_without_effects() {
    let sandbox = Sandbox::new();
    let output = run(sandbox.configured(), b"");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        "✗ Failed to configure autolinks: Failed to spawn 'gh': entity not found\n".as_bytes()
    );
    sandbox.no_effect();
}

#[test]
fn nonexecutable_gh_reports_typed_io_error_and_never_runs_it() {
    let sandbox = Sandbox::new();
    fs::write(sandbox.0.join("bin/gh"), "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(sandbox.0.join("bin/gh"), fs::Permissions::from_mode(0o600)).unwrap();
    let output = run(sandbox.configured(), b"");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .starts_with("✗ Failed to configure autolinks: Failed to spawn 'gh': ")
    );
    sandbox.no_effect();
}

#[test]
fn native_help_extra_positionals_and_json_never_start_gh() {
    for (args, code) in [
        (vec!["--help"], 0),
        (vec!["unexpected"], 2),
        (vec!["--json"], 2),
    ] {
        let sandbox = Sandbox::new();
        sandbox.gh("exit 0");
        let mut command = sandbox.configured();
        command.args(args);
        let output = run(command, b"");
        assert_eq!(output.status.code(), Some(code));
        sandbox.no_effect();
    }
}
