use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use linear_cli::config::{
    ConfigFailure, DiagnosticReason, GitProbeError, GitProbeResult, GitRootProbe, OsFamily,
    ProcessEnvError, ProcessEnvSnapshot, RealFileSource, RealGitRootProbe, discover_config_paths,
    load_env,
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct TempTree(PathBuf);

impl TempTree {
    fn new() -> Self {
        let path = env::temp_dir().join(format!(
            "riir-r02b1-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    #[cfg(unix)]
    fn git(&self, body: &str) -> RealGitRootProbe {
        self.git_with(body, Duration::from_secs(10), &[])
    }

    #[cfg(unix)]
    fn git_with(
        &self,
        body: &str,
        timeout: Duration,
        extra_env: &[(&str, &str)],
    ) -> RealGitRootProbe {
        use std::os::unix::fs::PermissionsExt;
        let bin = self.0.join("bin");
        fs::create_dir_all(&bin).unwrap();
        let script = bin.join("git");
        fs::write(&script, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        let mut vars = BTreeMap::new();
        vars.insert(OsString::from("PATH"), bin.into_os_string());
        for (name, value) in extra_env {
            vars.insert(OsString::from(name), OsString::from(value));
        }
        RealGitRootProbe::with_test_environment(self.0.clone(), vars, timeout)
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn git_uses_exact_arguments_and_injected_cwd() {
    let tree = TempTree::new();
    let probe =
        tree.git("test \"$1\" = rev-parse || exit 9\ntest \"$2\" = --show-toplevel || exit 8\npwd");
    let physical_cwd = fs::canonicalize(&tree.0).unwrap();
    assert_eq!(
        probe.probe(),
        GitProbeResult::Completed {
            success: true,
            stdout: format!("{}\n", physical_cwd.display()),
        }
    );
    let inputs = ProcessEnvSnapshot::from_vars_os(tree.0.clone(), OsFamily::Unix, [])
        .unwrap()
        .inputs;
    let paths = discover_config_paths(&inputs, &probe).unwrap();
    assert_eq!(paths.project.len(), 5);
    assert_eq!(paths.project[2].path, physical_cwd.join("linear.toml"));
    let selected = load_env(&inputs, &RealFileSource, &probe).unwrap();
    assert!(selected.applied.is_empty());
}

#[cfg(unix)]
#[test]
fn missing_git_and_nonzero_empty_output_keep_distinct_policies() {
    let tree = TempTree::new();
    let mut env = BTreeMap::new();
    env.insert(OsString::from("PATH"), tree.0.clone().into_os_string());
    let missing =
        RealGitRootProbe::with_test_environment(tree.0.clone(), env, Duration::from_secs(10));
    assert_eq!(missing.probe(), GitProbeResult::SpawnFailure);
    let inputs = ProcessEnvSnapshot::from_vars_os(tree.0.clone(), OsFamily::Unix, [])
        .unwrap()
        .inputs;
    assert_eq!(
        discover_config_paths(&inputs, &missing)
            .unwrap()
            .project
            .len(),
        2
    );
    let nonzero = tree.git("exit 1");
    assert_eq!(
        nonzero.probe(),
        GitProbeResult::Completed {
            success: false,
            stdout: String::new()
        }
    );
    let paths = discover_config_paths(&inputs, &nonzero).unwrap();
    assert_eq!(paths.project.len(), 5);
    assert_eq!(paths.project[4].path, tree.0.join(".config/linear.toml"));
    assert_eq!(
        load_env(&inputs, &RealFileSource, &nonzero)
            .unwrap()
            .source_path,
        None
    );
}

#[cfg(unix)]
#[test]
fn non_executable_git_is_a_spawn_failure() {
    use std::os::unix::fs::PermissionsExt;
    let tree = TempTree::new();
    let bin = tree.0.join("bin");
    fs::create_dir(&bin).unwrap();
    let script = bin.join("git");
    fs::write(&script, b"#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o644)).unwrap();
    let mut vars = BTreeMap::new();
    vars.insert(OsString::from("PATH"), bin.into_os_string());
    let probe =
        RealGitRootProbe::with_test_environment(tree.0.clone(), vars, Duration::from_secs(10));
    assert_eq!(probe.probe(), GitProbeResult::SpawnFailure);
}

#[cfg(unix)]
#[test]
fn nonzero_git_with_valid_root_keeps_config_and_dotenv_policies_distinct() {
    let tree = TempTree::new();
    let root = fs::canonicalize(&tree.0).unwrap();
    let probe = tree.git("pwd; exit 1");
    let inputs = ProcessEnvSnapshot::from_vars_os(tree.0.clone(), OsFamily::Unix, [])
        .unwrap()
        .inputs;
    assert_eq!(
        discover_config_paths(&inputs, &probe).unwrap().project[2].path,
        root.join("linear.toml")
    );
    assert_eq!(
        load_env(&inputs, &RealFileSource, &probe)
            .unwrap()
            .source_path,
        None
    );
}

#[cfg(unix)]
#[test]
fn injected_child_environment_reaches_git_without_snapshot_filtering() {
    let tree = TempTree::new();
    let probe = tree.git_with(
        "test \"$GIT_DIR\" = private-marker || exit 7\npwd",
        Duration::from_secs(10),
        &[("GIT_DIR", "private-marker")],
    );
    assert!(matches!(
        probe.probe(),
        GitProbeResult::Completed { success: true, .. }
    ));
}

#[cfg(unix)]
#[test]
fn git_output_errors_are_typed_even_after_nonzero_exit() {
    let tree = TempTree::new();
    let inputs = ProcessEnvSnapshot::from_vars_os(tree.0.clone(), OsFamily::Unix, [])
        .unwrap()
        .inputs;
    for (script, expected) in [
        ("printf relative; exit 1", GitProbeError::MalformedStdout),
        (
            "printf '/tmp/one\\n/tmp/two\\n'; exit 1",
            GitProbeError::MalformedStdout,
        ),
        ("printf '\\377'; exit 1", GitProbeError::InvalidUtf8),
    ] {
        let probe = tree.git(script);
        assert_eq!(probe.probe(), GitProbeResult::Failed(expected.clone()));
        assert_eq!(
            discover_config_paths(&inputs, &probe),
            Err(ConfigFailure::GitProbe(expected))
        );
    }
}

#[cfg(unix)]
#[test]
fn timeout_and_excess_stdout_fail_within_bound() {
    let tree = TempTree::new();
    let inputs = ProcessEnvSnapshot::from_vars_os(tree.0.clone(), OsFamily::Unix, [])
        .unwrap()
        .inputs;
    // The background child holds stdout after the immediate parent exits.
    let probe = tree.git_with("/bin/sleep 2 & exit 0", Duration::from_millis(250), &[]);
    let start = Instant::now();
    assert_eq!(
        probe.probe(),
        GitProbeResult::Failed(GitProbeError::Timeout)
    );
    assert!(start.elapsed() < Duration::from_secs(1));
    assert_eq!(
        discover_config_paths(&inputs, &probe),
        Err(ConfigFailure::GitProbe(GitProbeError::Timeout))
    );
    let probe = tree.git("printf '%65537s' x");
    assert_eq!(
        probe.probe(),
        GitProbeResult::Failed(GitProbeError::Oversize)
    );
    let error = match load_env(&inputs, &RealFileSource, &probe) {
        Ok(_) => panic!("expected oversized Git output"),
        Err(error) => error,
    };
    assert_eq!(
        error.failure,
        ConfigFailure::GitProbe(GitProbeError::Oversize)
    );
    let probe = tree.git("printf '/%65535s' x");
    assert!(
        matches!(probe.probe(), GitProbeResult::Completed { success: true, stdout } if stdout.len() == 65536)
    );
}

#[cfg(target_os = "linux")]
#[test]
fn hung_git_is_killed_and_reaped_before_timeout_returns() {
    let tree = TempTree::new();
    let pid_file = tree.0.join("git.pid");
    let probe = tree.git_with(
        &format!(
            "printf '%s' \"$$\" > '{}'\nexec /bin/sleep 5",
            pid_file.display()
        ),
        Duration::from_millis(250),
        &[],
    );
    let start = Instant::now();
    assert_eq!(
        probe.probe(),
        GitProbeResult::Failed(GitProbeError::Timeout)
    );
    assert!(start.elapsed() < Duration::from_secs(1));
    let pid = fs::read_to_string(&pid_file).unwrap();
    assert!(!PathBuf::from("/proc").join(pid).exists());
}

#[cfg(unix)]
#[test]
fn earlier_dotenv_warning_survives_fatal_git_failure() {
    let tree = TempTree::new();
    fs::create_dir(tree.0.join(".env")).unwrap();
    let probe = tree.git("printf relative; exit 0");
    let inputs = ProcessEnvSnapshot::from_vars_os(tree.0.clone(), OsFamily::Unix, [])
        .unwrap()
        .inputs;
    let error = match load_env(&inputs, &RealFileSource, &probe) {
        Ok(_) => panic!("expected Git failure"),
        Err(error) => error,
    };
    assert_eq!(
        error.failure,
        ConfigFailure::GitProbe(GitProbeError::MalformedStdout)
    );
    assert_eq!(error.diagnostics.len(), 1);
    assert_eq!(error.diagnostics[0].path, tree.0.join(".env"));
    assert!(matches!(
        error.diagnostics[0].reason,
        DiagnosticReason::Unusable(_)
    ));
}

#[cfg(unix)]
#[test]
fn earlier_dotenv_warning_survives_oversized_root_file() {
    let tree = TempTree::new();
    fs::create_dir(tree.0.join(".env")).unwrap();
    let root = tree.0.join("root");
    fs::create_dir(&root).unwrap();
    fs::write(root.join(".env"), vec![b'x'; 1024 * 1024 + 1]).unwrap();
    let probe = tree.git(&format!("printf '%s\\n' '{}'", root.display()));
    let inputs = ProcessEnvSnapshot::from_vars_os(tree.0.clone(), OsFamily::Unix, [])
        .unwrap()
        .inputs;
    let error = match load_env(&inputs, &RealFileSource, &probe) {
        Ok(_) => panic!("expected oversized root file"),
        Err(error) => error,
    };
    assert_eq!(
        error.failure,
        ConfigFailure::Oversize {
            path: root.join(".env")
        }
    );
    assert_eq!(error.diagnostics.len(), 1);
    assert_eq!(error.diagnostics[0].path, tree.0.join(".env"));
}

#[test]
fn process_snapshot_filters_and_preserves_empty_values() {
    let cwd = PathBuf::from("/tmp/riir-b1");
    let snapshot = ProcessEnvSnapshot::from_vars_os(
        cwd.clone(),
        OsFamily::Unix,
        [
            (OsString::from("LINEAR_API_KEY"), OsString::from("")),
            (
                OsString::from("XDG_CONFIG_HOME"),
                OsString::from("/tmp/xdg"),
            ),
            (
                OsString::from("https_proxy"),
                OsString::from("http://proxy"),
            ),
            (OsString::from("UNRELATED"), OsString::from("ignored")),
            (OsString::from("CI"), OsString::from("")),
        ],
    )
    .unwrap();
    assert_eq!(snapshot.inputs.cwd, cwd);
    assert_eq!(snapshot.inputs.env("LINEAR_API_KEY"), Some(""));
    assert_eq!(snapshot.inputs.env("https_proxy"), Some("http://proxy"));
    assert_eq!(snapshot.inputs.env("CI"), Some(""));
    assert!(!snapshot.inputs.process_env.contains_key("UNRELATED"));
    let windows = ProcessEnvSnapshot::from_vars_os(
        PathBuf::from("C:\\tmp"),
        OsFamily::Windows,
        [
            (OsString::from("linear_api_key"), OsString::from("fake")),
            (OsString::from("ci"), OsString::from("false")),
        ],
    )
    .unwrap();
    assert_eq!(windows.inputs.env("LINEAR_API_KEY"), Some("fake"));
    assert_eq!(windows.inputs.env("CI"), Some("false"));
    assert_eq!(
        windows
            .original_names
            .get("LINEAR_API_KEY")
            .map(String::as_str),
        Some("linear_api_key")
    );
    let duplicate = ProcessEnvSnapshot::from_vars_os(
        PathBuf::from("C:\\tmp"),
        OsFamily::Windows,
        [
            (OsString::from("LINEAR_API_KEY"), OsString::from("one")),
            (OsString::from("linear_api_key"), OsString::from("two")),
        ],
    );
    assert!(
        matches!(duplicate, Err(ProcessEnvError::DuplicateName { name }) if name == "linear_api_key")
    );
}

#[cfg(unix)]
#[test]
fn relevant_non_utf8_env_value_fails_and_unrelated_one_is_ignored() {
    use std::os::unix::ffi::OsStringExt;
    let bad = OsString::from_vec(vec![0xff]);
    let cwd = PathBuf::from("/tmp/riir-b1");
    let result = ProcessEnvSnapshot::from_vars_os(
        cwd.clone(),
        OsFamily::Unix,
        [(OsString::from("LINEAR_API_KEY"), bad.clone())],
    );
    assert!(
        matches!(result, Err(ProcessEnvError::InvalidValue { name }) if name == "LINEAR_API_KEY")
    );
    let transport_result = ProcessEnvSnapshot::from_vars_os(
        cwd.clone(),
        OsFamily::Unix,
        [(OsString::from("HTTPS_PROXY"), bad.clone())],
    );
    assert!(
        matches!(transport_result, Err(ProcessEnvError::InvalidValue { name }) if name == "HTTPS_PROXY")
    );
    let result =
        ProcessEnvSnapshot::from_vars_os(cwd, OsFamily::Unix, [(OsString::from("UNRELATED"), bad)]);
    assert!(result.is_ok());
    let result = ProcessEnvSnapshot::from_vars_os(
        PathBuf::from("/tmp/riir-b1"),
        OsFamily::Unix,
        [(
            OsString::from_vec(b"LINEAR_\xff".to_vec()),
            OsString::from("value"),
        )],
    );
    assert!(matches!(result, Err(ProcessEnvError::InvalidName)));
}
