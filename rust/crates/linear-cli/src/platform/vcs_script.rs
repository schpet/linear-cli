//! Running jj/git for `issue commits` and `issue describe` with captured output.
use crate::{
    config::{ChildEnvOverlay, Vcs},
    error::{AppError, AppErrorKind},
    platform::vcs,
};
use std::{
    io::{self, Read},
    path::Path,
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Program {
    Git,
    Jj,
}
impl Program {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Git => "git",
            Self::Jj => "jj",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandSpec {
    pub program: Program,
    pub args: Vec<String>,
}
impl CommandSpec {
    pub fn new(program: Program, args: &[&str]) -> Self {
        Self {
            program,
            args: args.iter().map(|value| (*value).to_owned()).collect(),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChildOutcome {
    Code(i32),
    Signal(i32),
}
impl ChildOutcome {
    pub const fn success(self) -> bool {
        matches!(self, Self::Code(0))
    }
}
#[derive(Debug)]
pub struct Captured {
    pub outcome: ChildOutcome,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}
pub trait ProcessRunner {
    fn capture(
        &mut self,
        spec: &CommandSpec,
        cwd: &Path,
        env: &ChildEnvOverlay,
    ) -> Result<Captured, AppError>;
    fn inherit(
        &mut self,
        spec: &CommandSpec,
        cwd: &Path,
        env: &ChildEnvOverlay,
    ) -> Result<ChildOutcome, AppError>;
}
pub struct NativeProcessRunner;
fn process_error(stage: &str, error: io::Error) -> AppError {
    AppError::new(
        AppErrorKind::IoProcess,
        format!("Failed to {stage}: {error}"),
    )
    .with_source(error)
}
fn outcome(status: std::process::ExitStatus) -> io::Result<ChildOutcome> {
    if let Some(code) = status.code() {
        return Ok(ChildOutcome::Code(code));
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return Ok(ChildOutcome::Signal(signal));
        }
    }
    Err(io::Error::other(
        "VCS child has no observable exit code or signal",
    ))
}
/// Public fault seam; production is the owned exact child, not process-name lookup.
pub trait ChildControl {
    fn try_wait(&mut self) -> io::Result<Option<ChildOutcome>>;
    fn kill(&mut self) -> io::Result<()>;
    fn wait(&mut self) -> io::Result<ChildOutcome>;
}
struct OwnedChild {
    child: Child,
    reaped: bool,
}
impl ChildControl for OwnedChild {
    fn try_wait(&mut self) -> io::Result<Option<ChildOutcome>> {
        match self.child.try_wait()? {
            Some(status) => {
                self.reaped = true;
                outcome(status).map(Some)
            }
            None => Ok(None),
        }
    }
    fn kill(&mut self) -> io::Result<()> {
        self.child.kill()
    }
    fn wait(&mut self) -> io::Result<ChildOutcome> {
        let status = self.child.wait()?;
        self.reaped = true;
        outcome(status)
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if !self.reaped {
            // Best-effort cancellation guard; ordinary failures use abort_child and
            // report cleanup details. No universal descendant-lifetime claim.
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}
fn abort_child(owner: &mut impl ChildControl, error: AppError) -> AppError {
    let killed = owner.kill();
    let reaped = owner.wait();
    if killed.is_err() || reaped.is_err() {
        error.with_debug_detail(format!(
            "VCS cleanup kill={killed:?}; wait={reaped:?}; external process effects may remain"
        ))
    } else {
        error
    }
}
fn read_all(mut reader: impl Read) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;
    Ok(bytes)
}
enum ReaderDone {
    Stdout(io::Result<Vec<u8>>),
    Stderr(io::Result<Vec<u8>>),
}
/// Reader errors are sent directly to the owner BEFORE blocking wait or joins.
/// The 10ms receive poll is not a deadline or an output cap.
pub fn collect_captured<O, E>(
    owner: &mut impl ChildControl,
    stdout: O,
    stderr: E,
) -> Result<Captured, AppError>
where
    O: Read + Send,
    E: Read + Send,
{
    thread::scope(|scope| {
        let (sender, receiver) = mpsc::channel();
        let out_sender = sender.clone();
        let out = scope.spawn(move || {
            let result = read_all(stdout);
            let _ = out_sender.send(ReaderDone::Stdout(result));
        });
        let err = scope.spawn(move || {
            let result = read_all(stderr);
            let _ = sender.send(ReaderDone::Stderr(result));
        });
        let mut output = None;
        let mut error_output = None;
        let mut status = None;
        let result = loop {
            match receiver.recv_timeout(Duration::from_millis(10)) {
                Ok(ReaderDone::Stdout(Ok(bytes))) => {
                    if output.replace(bytes).is_some() {
                        break Err(abort_child(
                            owner,
                            AppError::new(AppErrorKind::Invariant, "duplicate VCS stdout result"),
                        ));
                    }
                }
                Ok(ReaderDone::Stderr(Ok(bytes))) => {
                    if error_output.replace(bytes).is_some() {
                        break Err(abort_child(
                            owner,
                            AppError::new(AppErrorKind::Invariant, "duplicate VCS stderr result"),
                        ));
                    }
                }
                Ok(ReaderDone::Stdout(Err(error))) => {
                    break Err(abort_child(owner, process_error("read VCS stdout", error)));
                }
                Ok(ReaderDone::Stderr(Err(error))) => {
                    break Err(abort_child(owner, process_error("read VCS stderr", error)));
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected)
                    if output.is_some() && error_output.is_some() => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    break Err(abort_child(
                        owner,
                        AppError::new(
                            AppErrorKind::Invariant,
                            "VCS reader terminated without a result",
                        ),
                    ));
                }
            }
            if status.is_none() {
                status = match owner.try_wait() {
                    Ok(value) => value,
                    Err(error) => {
                        break Err(abort_child(owner, process_error("wait for VCS", error)));
                    }
                };
            }
            if let Some(outcome) = status
                && output.is_some()
                && error_output.is_some()
            {
                match (output.take(), error_output.take()) {
                    (Some(stdout), Some(stderr)) => {
                        break Ok(Captured {
                            outcome,
                            stdout,
                            stderr,
                        });
                    }
                    _ => {
                        break Err(abort_child(
                            owner,
                            AppError::new(
                                AppErrorKind::Invariant,
                                "completed VCS reader result disappeared",
                            ),
                        ));
                    }
                }
            }
            // Once both readers finish, a disconnected channel must not spin
            // while the child runs. Blocking wait is safe only AFTER both EOFs.
            if output.is_some() && error_output.is_some() && status.is_none() {
                status = match owner.wait() {
                    Ok(value) => Some(value),
                    Err(error) => {
                        break Err(abort_child(owner, process_error("wait for VCS", error)));
                    }
                };
                match (status, output.take(), error_output.take()) {
                    (Some(outcome), Some(stdout), Some(stderr)) => {
                        break Ok(Captured {
                            outcome,
                            stdout,
                            stderr,
                        });
                    }
                    _ => {
                        break Err(abort_child(
                            owner,
                            AppError::new(
                                AppErrorKind::Invariant,
                                "completed VCS capture state disappeared",
                            ),
                        ));
                    }
                }
            }
        };
        if out.join().is_err() || err.join().is_err() {
            return Err(abort_child(
                owner,
                AppError::new(AppErrorKind::Invariant, "VCS reader panicked"),
            ));
        }
        result
    })
}
fn command(spec: &CommandSpec, cwd: &Path, env: &ChildEnvOverlay) -> Command {
    let mut command = Command::new(spec.program.name());
    command
        .args(&spec.args)
        .current_dir(cwd)
        .envs(env.iter())
        .stdin(Stdio::null());
    command
}
fn spawn(command: &mut Command, program: Program) -> Result<OwnedChild, AppError> {
    command
        .spawn()
        .map(|child| OwnedChild {
            child,
            reaped: false,
        })
        .map_err(|error| {
            let message = if error.kind() == io::ErrorKind::NotFound {
                format!("Failed to spawn '{}': entity not found", program.name())
            } else {
                format!("Failed to spawn '{}': {error}", program.name())
            };
            AppError::new(AppErrorKind::IoProcess, message).with_source(error)
        })
}
impl ProcessRunner for NativeProcessRunner {
    fn capture(
        &mut self,
        spec: &CommandSpec,
        cwd: &Path,
        env: &ChildEnvOverlay,
    ) -> Result<Captured, AppError> {
        let mut command = command(spec, cwd, env);
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut owner = spawn(&mut command, spec.program)?;
        let stdout =
            owner.child.stdout.take().ok_or_else(|| {
                AppError::new(AppErrorKind::Invariant, "VCS stdout was not piped")
            })?;
        let stderr =
            owner.child.stderr.take().ok_or_else(|| {
                AppError::new(AppErrorKind::Invariant, "VCS stderr was not piped")
            })?;
        collect_captured(&mut owner, stdout, stderr)
    }
    fn inherit(
        &mut self,
        spec: &CommandSpec,
        cwd: &Path,
        env: &ChildEnvOverlay,
    ) -> Result<ChildOutcome, AppError> {
        let mut command = command(spec, cwd, env);
        command.stdout(Stdio::inherit()).stderr(Stdio::inherit());
        let mut owner = spawn(&mut command, spec.program)?;
        owner
            .wait()
            .map_err(|error| abort_child(&mut owner, process_error("wait for VCS", error)))
    }
}
/// Process output as text: one leading BOM dropped, invalid UTF-8 replaced, trimmed.
pub fn decoded_trim(bytes: &[u8]) -> String {
    let decoded = String::from_utf8_lossy(bytes);
    decoded
        .strip_prefix('\u{feff}')
        .unwrap_or(&decoded)
        .trim()
        .to_owned()
}
pub fn inference_spec(vcs: Vcs) -> CommandSpec {
    match vcs {
        Vcs::Git => CommandSpec::new(Program::Git, &["symbolic-ref", "--short", "HEAD"]),
        Vcs::Jj => CommandSpec::new(
            Program::Jj,
            &["log", "-r", "::@", "-T", vcs::JJ_TEMPLATE, "--no-graph"],
        ),
    }
}
pub fn infer_issue(
    runner: &mut impl ProcessRunner,
    vcs: Vcs,
    cwd: &Path,
    env: &ChildEnvOverlay,
) -> Result<Option<String>, AppError> {
    let captured = runner.capture(&inference_spec(vcs), cwd, env)?;
    let stdout = decoded_trim(&captured.stdout);
    match vcs {
        Vcs::Git => vcs::parse_git_branch(
            captured.outcome.success(),
            &stdout,
            &decoded_trim(&captured.stderr),
        ),
        Vcs::Jj => Ok(if captured.outcome.success() {
            vcs::parse_jj_trailers(&stdout)
        } else {
            None
        }),
    }
}
