//! Public process contracts with isolated fake executable fixtures.
use linear_cli::{
    commands::issue_commits,
    config::{FileKind, FileSource, OsFamily, ProcessEnvSnapshot, Vcs},
    error::Error,
    platform::vcs_script::{
        self, Captured, ChildControl, ChildOutcome, CommandSpec, ProcessRunner, Program,
    },
};
use std::{
    collections::VecDeque,
    io::{self, Cursor, Read},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
struct EmptyFiles;
impl FileSource for EmptyFiles {
    fn kind(&self, _path: &Path) -> io::Result<Option<FileKind>> {
        Ok(None)
    }
    fn read_bounded(&self, _path: &Path, _limit: u64) -> io::Result<Vec<u8>> {
        Err(io::Error::from(io::ErrorKind::NotFound))
    }
}
fn overlay() -> linear_cli::config::ChildEnvOverlay {
    let snapshot = ProcessEnvSnapshot::from_vars_os(
        PathBuf::from("/vcs-script-test"),
        OsFamily::Unix,
        std::iter::empty(),
    )
    .expect("test environment");
    linear_cli::config::load_startup(&snapshot, &EmptyFiles)
        .result
        .expect("test startup")
        .child_env
}
struct Runner {
    captures: VecDeque<Captured>,
    requests: Vec<CommandSpec>,
    final_outcome: ChildOutcome,
}
impl ProcessRunner for Runner {
    fn capture(
        &mut self,
        spec: &CommandSpec,
        _cwd: &Path,
        _env: &linear_cli::config::ChildEnvOverlay,
    ) -> Result<Captured, Error> {
        self.requests.push(spec.clone());
        Ok(self
            .captures
            .pop_front()
            .expect("exact planned captured process"))
    }
    fn inherit(
        &mut self,
        spec: &CommandSpec,
        _cwd: &Path,
        _env: &linear_cli::config::ChildEnvOverlay,
    ) -> Result<ChildOutcome, Error> {
        self.requests.push(spec.clone());
        Ok(self.final_outcome)
    }
}
#[test]
fn child_exit_code_is_checked_and_signal_mapping_preserves_supported_status() {
    for (outcome, expected) in [
        (ChildOutcome::Code(0), 0),
        (ChildOutcome::Code(7), 7),
        (ChildOutcome::Code(255), 255),
        (ChildOutcome::Signal(15), 143),
    ] {
        let code = match issue_commits::child_status(outcome) {
            Ok(()) => 0,
            Err(error) => error.exit_code(),
        };
        assert_eq!(code, expected);
    }
    for code in [-1, 256, i32::MAX, i32::MIN] {
        let error = issue_commits::child_status(ChildOutcome::Code(code)).unwrap_err();
        assert_eq!(
            error.message(),
            format!("Child exit code {code} is outside supported range 0..255")
        );
        assert_eq!(
            error.context(issue_commits::CONTEXT).to_string(),
            format!(
                "Failed to show commits: Child exit code {code} is outside supported range 0..255"
            )
        );
    }
}
#[test]
fn probe_ignores_status_and_stderr_while_exact_final_args_are_opaque() {
    let mut runner = Runner {
        captures: VecDeque::from([Captured {
            outcome: ChildOutcome::Code(19),
            stdout: b"\xef\xbb\xbf  opaque \xff\n".to_vec(),
            stderr: b"DUMMY hidden".to_vec(),
        }]),
        requests: vec![],
        final_outcome: ChildOutcome::Code(7),
    };
    assert_eq!(
        issue_commits::show(&mut runner, "ENG-7", Path::new("/fake"), &overlay())
            .unwrap_err()
            .exit_code(),
        7
    );
    assert_eq!(
        runner.requests,
        vec![
            CommandSpec::new(
                Program::Jj,
                &[
                    "log",
                    "-r",
                    "description(regex:\"(?m)^Linear-issue:.*ENG-7\")",
                    "-T",
                    "commit_id",
                    "--no-graph"
                ]
            ),
            CommandSpec::new(
                Program::Jj,
                &[
                    "log",
                    "-r",
                    "description(regex:\"(?m)^Linear-issue:.*ENG-7\")",
                    "-p",
                    "--git",
                    "--no-graph",
                    "-T",
                    "builtin_log_compact_full_description"
                ]
            )
        ]
    );
    let mut empty = Runner {
        captures: VecDeque::from([Captured {
            outcome: ChildOutcome::Code(0),
            stdout: b"\xef\xbb\xbf \n".to_vec(),
            stderr: vec![],
        }]),
        requests: vec![],
        final_outcome: ChildOutcome::Code(7),
    };
    assert_eq!(
        issue_commits::show(&mut empty, "ENG-7", Path::new("/fake"), &overlay())
            .unwrap_err()
            .message(),
        "Commits not found: ENG-7"
    );
    assert_eq!(empty.requests.len(), 1);
}
#[test]
fn opt_in_inference_preserves_existing_parsers_and_distinct_nonzero_policy() {
    for (vcs, outcome, stdout, stderr, expected) in [
        (
            Vcs::Jj,
            ChildOutcome::Code(0),
            "Fixes ENG-1\nReferences z9-7\n\nFixes ENG-99",
            "",
            Some("Z9-7"),
        ),
        (
            Vcs::Jj,
            ChildOutcome::Code(9),
            "Fixes ENG-1",
            "DUMMY hidden",
            None,
        ),
        (
            Vcs::Git,
            ChildOutcome::Code(0),
            "feature/_bad-eng-0/abc9-73-next",
            "",
            Some("ABC9-73"),
        ),
        (
            Vcs::Git,
            ChildOutcome::Code(1),
            "ENG-9",
            "DUMMY not a symbolic ref",
            None,
        ),
    ] {
        let mut runner = Runner {
            captures: VecDeque::from([Captured {
                outcome,
                stdout: stdout.as_bytes().to_vec(),
                stderr: stderr.as_bytes().to_vec(),
            }]),
            requests: vec![],
            final_outcome: ChildOutcome::Code(0),
        };
        assert_eq!(
            vcs_script::infer_issue(&mut runner, vcs, Path::new("/fake"), &overlay())
                .unwrap()
                .as_deref(),
            expected
        );
        assert_eq!(runner.requests, vec![vcs_script::inference_spec(vcs)]);
    }
    assert_eq!(
        vcs_script::decoded_trim(b"\xef\xbb\xbf \xff \xc2\x85"),
        "\u{fffd}"
    );
    assert_eq!(
        linear_cli::platform::vcs::parse_git_branch(false, "", " DUMMY failure \n")
            .unwrap_err()
            .message(),
        "Failed to get current branch: DUMMY failure"
    );
    assert!(issue_commits::check_vcs(Vcs::Jj).is_ok());
    assert_eq!(
        issue_commits::check_vcs(Vcs::Git).unwrap_err().message(),
        "commits is only supported with jj-vcs"
    );
}
struct FaultReader;
impl Read for FaultReader {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("DUMMY reader fault"))
    }
}
struct BlockUntilKilled(Arc<AtomicBool>);
impl Read for BlockUntilKilled {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        while !self.0.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(1));
        }
        Ok(0)
    }
}
struct Owner {
    release: Arc<AtomicBool>,
    events: Vec<&'static str>,
    wait_fault: bool,
}
impl ChildControl for Owner {
    fn try_wait(&mut self) -> io::Result<Option<ChildOutcome>> {
        self.events.push("try_wait");
        if self.wait_fault {
            Err(io::Error::other("DUMMY wait fault"))
        } else {
            Ok(None)
        }
    }
    fn kill(&mut self) -> io::Result<()> {
        self.events.push("kill");
        self.release.store(true, Ordering::SeqCst);
        Ok(())
    }
    fn wait(&mut self) -> io::Result<ChildOutcome> {
        self.events.push("wait");
        assert!(
            self.release.load(Ordering::SeqCst),
            "must notify owner and kill before blocking wait/join on a fault"
        );
        Ok(ChildOutcome::Code(0))
    }
}
#[test]
fn reader_and_wait_fault_notify_owner_before_wait_and_release_other_pipe() {
    for wait_fault in [false, true] {
        let release = Arc::new(AtomicBool::new(false));
        let done = Arc::new(AtomicBool::new(false));
        let forced = Arc::new(AtomicBool::new(false));
        let (watch_release, watch_done, watch_forced) =
            (release.clone(), done.clone(), forced.clone());
        let watchdog = thread::spawn(move || {
            let start = Instant::now();
            while !watch_done.load(Ordering::SeqCst) && start.elapsed() < Duration::from_secs(1) {
                thread::sleep(Duration::from_millis(1));
            }
            if !watch_done.load(Ordering::SeqCst) {
                watch_forced.store(true, Ordering::SeqCst);
                watch_release.store(true, Ordering::SeqCst);
            }
        });
        let mut owner = Owner {
            release: release.clone(),
            events: vec![],
            wait_fault,
        };
        let error = if wait_fault {
            vcs_script::collect_captured(&mut owner, BlockUntilKilled(release), Cursor::new(vec![]))
                .unwrap_err()
        } else {
            vcs_script::collect_captured(&mut owner, BlockUntilKilled(release), FaultReader)
                .unwrap_err()
        };
        done.store(true, Ordering::SeqCst);
        watchdog.join().unwrap();
        assert!(!forced.load(Ordering::SeqCst));
        assert!(error.message().contains(if wait_fault {
            "DUMMY wait fault"
        } else {
            "DUMMY reader fault"
        }));
        assert_eq!(
            owner
                .events
                .iter()
                .filter(|event| **event != "try_wait")
                .copied()
                .collect::<Vec<_>>(),
            vec!["kill", "wait"]
        );
    }
}
#[cfg(unix)]
#[test]
fn actual_fake_jj_drains_both_pipe_capacities_and_observes_null_stdin() {
    use std::os::unix::fs::PermissionsExt;
    if let Some(root) = std::env::var_os("LINEAR_VCS_CAPACITY_ROOT") {
        let root = PathBuf::from(root);
        assert_eq!(
            std::env::var_os("PATH"),
            Some(root.join("bin").into_os_string())
        );
        let spec = CommandSpec::new(
            Program::Jj,
            &["log", "-r", "DUMMY", "-T", "commit_id", "--no-graph"],
        );
        let captured = vcs_script::NativeProcessRunner
            .capture(&spec, &root, &overlay())
            .unwrap();
        assert_eq!(captured.outcome, ChildOutcome::Code(19));
        assert_eq!(captured.stdout, vec![b'x'; 131072]);
        assert_eq!(captured.stderr, vec![b'y'; 131072]);
        return;
    }
    let root = std::env::temp_dir().join(format!("linear-c067-capacity-{}", std::process::id()));
    assert!(!root.exists());
    std::fs::create_dir(&root).unwrap();
    let bin = root.join("bin");
    std::fs::create_dir(&bin).unwrap();
    let exe = bin.join("jj");
    std::fs::write(&exe,"#!/usr/bin/python3\nimport os,sys\nassert sys.stdin.buffer.read()==b''\nos.write(1,b'x'*131072)\nos.write(2,b'y'*131072)\nsys.exit(19)\n").unwrap();
    std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o700)).unwrap();
    // ChildEnvOverlay contains dotenv changes, not a replacement process env.
    // Isolate the actual ambient PATH without mutating global test-process env.
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "vcs_script::actual_fake_jj_drains_both_pipe_capacities_and_observes_null_stdin",
            "--nocapture",
        ])
        .env_clear()
        .env("PATH", &bin)
        .env("LINEAR_VCS_CAPACITY_ROOT", &root)
        .spawn()
        .unwrap();
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if start.elapsed() > Duration::from_secs(5) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("fake pipe-capacity subprocess exceeded test-only bound");
        }
        thread::sleep(Duration::from_millis(2));
    };
    assert!(status.success());
    std::fs::remove_dir_all(root).unwrap();
}
