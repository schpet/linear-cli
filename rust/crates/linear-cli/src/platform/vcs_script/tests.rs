use super::*;
use crate::config::{FileKind, FileSource, OsFamily, ProcessEnvSnapshot, load_startup};
use std::{
    collections::VecDeque,
    io::Cursor,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

struct NoFiles;
impl FileSource for NoFiles {
    fn kind(&self, _path: &Path) -> io::Result<Option<FileKind>> {
        Ok(None)
    }
    fn read_bounded(&self, _path: &Path, _limit: u64) -> io::Result<Vec<u8>> {
        Err(io::Error::from(io::ErrorKind::NotFound))
    }
}

fn empty_overlay() -> ChildEnvOverlay {
    let snapshot = ProcessEnvSnapshot::from_vars_os(
        PathBuf::from("/repo"),
        OsFamily::Unix,
        std::iter::empty(),
    )
    .expect("empty environment");
    load_startup(&snapshot, &NoFiles)
        .result
        .expect("startup without config")
        .child_env
}

/// Answers every capture with one canned result and records what was run.
struct CannedRunner {
    captures: VecDeque<Captured>,
    requests: Vec<CommandSpec>,
}

impl CannedRunner {
    fn new(code: i32, stdout: &str) -> Self {
        Self {
            captures: VecDeque::from([Captured {
                outcome: ChildOutcome::Code(code),
                stdout: stdout.as_bytes().to_vec(),
                stderr: Vec::new(),
            }]),
            requests: Vec::new(),
        }
    }
}

impl ProcessRunner for CannedRunner {
    fn capture(
        &mut self,
        spec: &CommandSpec,
        _cwd: &Path,
        _env: &ChildEnvOverlay,
    ) -> Result<Captured, Error> {
        self.requests.push(spec.clone());
        Ok(self.captures.pop_front().expect("one capture per test"))
    }
    fn inherit(
        &mut self,
        _spec: &CommandSpec,
        _cwd: &Path,
        _env: &ChildEnvOverlay,
    ) -> Result<ChildOutcome, Error> {
        unreachable!("issue inference never runs an inheriting child");
    }
}

#[test]
fn issue_inference_reads_jj_trailers_or_the_git_branch() {
    for (vcs, code, stdout, expected) in [
        (Vcs::Jj, 0, "Fixes ENG-1\nReferences z9-7", Some("Z9-7")),
        (Vcs::Jj, 9, "Fixes ENG-1", None),
        (Vcs::Git, 0, "feature/abc9-73-next", Some("ABC9-73")),
        (Vcs::Git, 1, "", None),
    ] {
        let mut runner = CannedRunner::new(code, stdout);
        let issue = infer_issue(&mut runner, vcs, Path::new("/repo"), &empty_overlay())
            .expect("inference succeeds");
        assert_eq!(issue.as_deref(), expected, "{vcs:?} {stdout:?}");
        assert_eq!(runner.requests, [inference_spec(vcs)]);
    }
}

#[test]
fn decoded_output_drops_a_bom_replaces_invalid_bytes_and_trims() {
    assert_eq!(decoded_trim(b"\xef\xbb\xbf  ENG-7 \n"), "ENG-7");
    assert_eq!(decoded_trim(b"\xef\xbb\xbf \xff \xc2\x85"), "\u{fffd}");
}

struct FailingReader;
impl Read for FailingReader {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("reader fault"))
    }
}

/// A pipe that stays open until the child is killed.
struct OpenUntilKilled(Arc<AtomicBool>);
impl Read for OpenUntilKilled {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        while !self.0.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(1));
        }
        Ok(0)
    }
}

struct FakeChild {
    killed: Arc<AtomicBool>,
    events: Vec<&'static str>,
    wait_fails: bool,
}

impl ChildControl for FakeChild {
    fn try_wait(&mut self) -> io::Result<Option<ChildOutcome>> {
        if self.wait_fails {
            Err(io::Error::other("wait fault"))
        } else {
            Ok(None)
        }
    }
    fn kill(&mut self) -> io::Result<()> {
        self.events.push("kill");
        self.killed.store(true, Ordering::SeqCst);
        Ok(())
    }
    fn wait(&mut self) -> io::Result<ChildOutcome> {
        self.events.push("wait");
        assert!(
            self.killed.load(Ordering::SeqCst),
            "a fault must kill the child before blocking on it"
        );
        Ok(ChildOutcome::Code(0))
    }
}

/// A reader or wait failure kills the child before any blocking wait, which
/// also closes the other pipe so its reader thread can finish.
#[test]
fn faults_kill_the_child_before_waiting() {
    for wait_fails in [false, true] {
        let killed = Arc::new(AtomicBool::new(false));
        let mut child = FakeChild {
            killed: Arc::clone(&killed),
            events: Vec::new(),
            wait_fails,
        };
        let started = Instant::now();
        let open = OpenUntilKilled(killed);
        let error = if wait_fails {
            collect_captured(&mut child, open, Cursor::new(Vec::new()))
        } else {
            collect_captured(&mut child, open, FailingReader)
        }
        .expect_err("fault is reported");
        assert!(started.elapsed() < Duration::from_secs(5));
        let fault = if wait_fails {
            "wait fault"
        } else {
            "reader fault"
        };
        assert!(error.message().contains(fault), "{error}");
        assert_eq!(child.events, ["kill", "wait"]);
    }
}

/// Output larger than a pipe buffer on both streams is drained without
/// deadlocking the child.
#[cfg(unix)]
#[test]
fn both_pipes_are_drained_past_their_capacity() {
    let mut command = Command::new("/bin/sh");
    command
        .args([
            "-c",
            "head -c 131072 /dev/zero; head -c 131072 /dev/zero >&2; exit 19",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut owner = spawn(&mut command, Program::Jj).expect("spawn sh");
    let stdout = owner.child.stdout.take().expect("stdout pipe");
    let stderr = owner.child.stderr.take().expect("stderr pipe");
    let captured = collect_captured(&mut owner, stdout, stderr).expect("capture");
    assert_eq!(captured.outcome, ChildOutcome::Code(19));
    assert_eq!(captured.stdout, vec![0; 131_072]);
    assert_eq!(captured.stderr, vec![0; 131_072]);
}
