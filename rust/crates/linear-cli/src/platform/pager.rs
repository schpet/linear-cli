//! Paging for long terminal output.
//!
//! Output is paged only on a TTY, with
//! more lines than `rows - 2`, or more than 50 when the size is unknown. `PAGER`
//! is split on whitespace into a program and arguments; no shell is involved.
//! When a pager fails, the platform fallbacks run in order, skipping one whose
//! program name exactly matches the failed program. When every pager fails,
//! the output is printed directly with an added line feed.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io::{self, Write};
use std::num::NonZeroU16;
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, ExitStatus, Stdio};

use crate::config::{NoColor, OsFamily};
use crate::error::{AppError, AppErrorKind};
use crate::platform::markdown_terminal::{self, HostSource, RenderOptions};
use crate::platform::output::{Output, OutputOutcome, OutputPolicy, Stream};

/// Line limit used when a TTY reports no size.
pub const UNKNOWN_SIZE_LINE_LIMIT: usize = 50;

/// The operating system family this binary targets.
pub const HOST_OS: OsFamily = if cfg!(windows) {
    OsFamily::Windows
} else {
    OsFamily::Unix
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalSize {
    pub columns: u16,
    pub rows: u16,
}

/// The stdout terminal size, or `None` when stdout is not a terminal or
/// reports a zero dimension.
pub fn stdout_size() -> Option<TerminalSize> {
    usable_size(terminal_size::terminal_size_of(io::stdout()).map(
        |(terminal_size::Width(columns), terminal_size::Height(rows))| TerminalSize {
            columns,
            rows,
        },
    ))
}

/// The terminal-size adapter treats either zero dimension as unavailable.
pub fn usable_size(size: Option<TerminalSize>) -> Option<TerminalSize> {
    size.filter(|size| size.columns > 0 && size.rows > 0)
}

/// Whether rendered output should go to a pager. Lines are counted by
/// splitting on LF, so a trailing LF adds one empty line.
pub fn should_page(
    rendered: &str,
    pager_enabled: bool,
    stdout_tty: bool,
    size: Option<TerminalSize>,
) -> bool {
    if !pager_enabled || !stdout_tty {
        return false;
    }
    let lines = rendered.split('\n').count();
    match size {
        Some(size) => lines > usize::from(size.rows).saturating_sub(2),
        None => lines > UNKNOWN_SIZE_LINE_LIMIT,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PagerCommand {
    pub program: String,
    pub args: Vec<String>,
}

impl PagerCommand {
    fn new(program: &str, args: &[&str]) -> Self {
        Self {
            program: program.to_owned(),
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        }
    }
}

impl fmt::Display for PagerCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.program)?;
        for arg in &self.args {
            write!(f, " {arg}")?;
        }
        Ok(())
    }
}

/// The first pager to try. A nonempty `PAGER` is trimmed and split on
/// whitespace; a whitespace-only value yields an empty program, which fails
/// and falls through to every fallback.
pub fn primary_command(pager: Option<&str>, os: OsFamily) -> PagerCommand {
    match (pager, os) {
        (Some(value), _) if !value.is_empty() => {
            let mut parts = value
                .split(char::is_whitespace)
                .filter(|part| !part.is_empty())
                .map(str::to_owned);
            PagerCommand {
                program: parts.next().unwrap_or_default(),
                args: parts.collect(),
            }
        }
        (Some(_) | None, OsFamily::Unix) => PagerCommand::new("less", &["-R", "-X"]),
        (Some(_) | None, OsFamily::Windows) => PagerCommand::new("more", &[]),
    }
}

/// Fallbacks after `failed_program` failed. The comparison is by exact
/// program name: a failed `less -F` skips `less`, while `/usr/bin/less` does not.
pub fn fallback_commands(failed_program: &str, os: OsFamily) -> Vec<PagerCommand> {
    let chain = match os {
        OsFamily::Unix => vec![
            PagerCommand::new("less", &["-R", "-X"]),
            PagerCommand::new("more", &[]),
            PagerCommand::new("cat", &[]),
        ],
        OsFamily::Windows => vec![
            PagerCommand::new("more", &[]),
            PagerCommand::new("less", &["-R", "-X"]),
        ],
    };
    chain
        .into_iter()
        .filter(|command| command.program != failed_program)
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChildExit {
    Code(i32),
    Signal(i32),
    Unknown,
}

impl From<ExitStatus> for ChildExit {
    fn from(status: ExitStatus) -> Self {
        if let Some(code) = status.code() {
            return Self::Code(code);
        }
        #[cfg(unix)]
        if let Some(signal) = status.signal() {
            return Self::Signal(signal);
        }
        Self::Unknown
    }
}

#[derive(Debug)]
pub enum PagerFailure {
    /// A whitespace-only `PAGER` names no program; nothing is spawned.
    EmptyProgram,
    /// The child closed stdin before accepting the complete input.
    ClosedEarly,
    Spawn(io::Error),
    /// Writing failed. A broken pipe counts only when the pager also failed.
    Write {
        error: io::Error,
        exit: ChildExit,
    },
    Exit(ChildExit),
    Wait(io::Error),
}

#[derive(Debug)]
pub enum PagerAttempt {
    /// Every byte was written and the pager exited successfully.
    Completed,
    /// The pager closed its input before reading everything, then exited
    /// successfully: the user quit early. This counts as a failure, so the
    /// content is shown again through a fallback.
    ClosedEarly,
    Failed(PagerFailure),
}

impl PagerAttempt {
    pub fn succeeded(&self) -> bool {
        match self {
            Self::Completed => true,
            Self::ClosedEarly | Self::Failed(_) => false,
        }
    }
}

/// Runs one pager: it inherits stdout and stderr, receives `input` on stdin,
/// and is waited for after stdin closes.
pub trait PagerRunner {
    fn run(&mut self, command: &PagerCommand, input: &[u8]) -> PagerAttempt;
}

enum BaseEnvironment {
    Inherit,
    Replace(BTreeMap<OsString, OsString>),
}

/// Spawns real pager processes without a shell.
pub struct ProcessPagerRunner {
    base: BaseEnvironment,
    overlay: BTreeMap<String, String>,
}

impl ProcessPagerRunner {
    /// Children inherit the process environment plus the applied dotenv
    /// overlay, which never contains a key the process environment has.
    pub fn inheriting<'a>(overlay: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        Self {
            base: BaseEnvironment::Inherit,
            overlay: collect_overlay(overlay),
        }
    }

    /// A private base environment for deterministic adapter tests.
    #[doc(hidden)]
    pub fn with_test_environment<'a>(
        base: BTreeMap<OsString, OsString>,
        overlay: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Self {
        Self {
            base: BaseEnvironment::Replace(base),
            overlay: collect_overlay(overlay),
        }
    }
}

fn collect_overlay<'a>(
    overlay: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> BTreeMap<String, String> {
    overlay
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect()
}

impl PagerRunner for ProcessPagerRunner {
    fn run(&mut self, command: &PagerCommand, input: &[u8]) -> PagerAttempt {
        if command.program.is_empty() {
            return PagerAttempt::Failed(PagerFailure::EmptyProgram);
        }
        let mut process = Command::new(&command.program);
        process
            .args(&command.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        if let BaseEnvironment::Replace(base) = &self.base {
            process.env_clear().envs(base);
        }
        process.envs(&self.overlay);
        let mut child = match process.spawn() {
            Ok(child) => child,
            Err(error) => return PagerAttempt::Failed(PagerFailure::Spawn(error)),
        };
        // Dropping stdin at the end of this block sends end of input.
        let written = match child.stdin.take() {
            Some(mut stdin) => stdin.write_all(input).and_then(|()| stdin.flush()),
            None => Err(io::Error::other("pager stdin was not captured")),
        };
        let exit = match child.wait() {
            Ok(status) => status,
            Err(error) => return PagerAttempt::Failed(PagerFailure::Wait(error)),
        };
        match written {
            Ok(()) if exit.success() => PagerAttempt::Completed,
            Ok(()) => PagerAttempt::Failed(PagerFailure::Exit(exit.into())),
            Err(error) if error.kind() == io::ErrorKind::BrokenPipe && exit.success() => {
                PagerAttempt::ClosedEarly
            }
            Err(error) => PagerAttempt::Failed(PagerFailure::Write {
                error,
                exit: exit.into(),
            }),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Delivery {
    Paged(PagerCommand),
    /// Written to stdout with the console policy and an added line feed.
    Direct(OutputOutcome),
}

#[derive(Debug)]
pub struct Shown {
    pub delivery: Delivery,
    /// Pagers tried before delivery, in order, with why each failed.
    pub failures: Vec<(PagerCommand, PagerFailure)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PagerRequest<'a> {
    /// False for `--no-pager`.
    pub enabled: bool,
    pub stdout_tty: bool,
    pub size: Option<TerminalSize>,
    /// The process `PAGER`; dotenv files cannot set it.
    pub pager: Option<&'a OsStr>,
    pub os: OsFamily,
}

/// Render Markdown and deliver it to a terminal. A size lookup failure uses
/// an explicit renderer width and the pager's 50-line fallback threshold.
pub fn render_and_show(
    markdown: &str,
    request: &PagerRequest<'_>,
    no_color: NoColor,
    hyperlink_format: Option<&str>,
    host: HostSource,
    runner: &mut dyn PagerRunner,
    stdout: &mut dyn Write,
) -> Result<Shown, AppError> {
    if !request.stdout_tty {
        return Err(AppError::new(
            AppErrorKind::Invariant,
            "terminal Markdown rendering requires TTY stdout",
        ));
    }
    let size = usable_size(request.size);
    let columns = size
        .and_then(|size| NonZeroU16::new(size.columns))
        .unwrap_or(markdown_terminal::FALLBACK_COLUMNS);
    let options = RenderOptions::for_terminal(
        columns,
        no_color,
        request.stdout_tty,
        hyperlink_format,
        host,
    );
    let rendered = markdown_terminal::render(markdown, &options)?;
    show(
        &rendered,
        &PagerRequest { size, ..*request },
        runner,
        stdout,
    )
}

/// Print rendered terminal output, through a pager when it is too long.
pub fn show(
    rendered: &str,
    request: &PagerRequest<'_>,
    runner: &mut dyn PagerRunner,
    stdout: &mut dyn Write,
) -> Result<Shown, AppError> {
    if should_page(rendered, request.enabled, request.stdout_tty, request.size) {
        let pager = request
            .pager
            .map(|value| {
                value.to_str().ok_or_else(|| {
                    AppError::new(AppErrorKind::Validation, "PAGER is not valid UTF-8")
                })
            })
            .transpose()?;
        page(
            rendered,
            primary_command(pager, request.os),
            request.os,
            runner,
            stdout,
        )
    } else {
        Ok(Shown {
            delivery: Delivery::Direct(print_line(rendered, stdout)?),
            failures: Vec::new(),
        })
    }
}

/// Page rendered output, trying fallbacks after `primary` fails. The pager
/// receives exactly the rendered bytes; only direct output adds a line feed.
pub fn page(
    rendered: &str,
    primary: PagerCommand,
    os: OsFamily,
    runner: &mut dyn PagerRunner,
    stdout: &mut dyn Write,
) -> Result<Shown, AppError> {
    // The child writes to the same terminal, so earlier output such as a
    // spinner clear has to reach it first.
    Output::new(stdout, Stream::Stdout).flush_with_policy(OutputPolicy::ConsoleLike)?;
    let fallbacks = fallback_commands(&primary.program, os);
    let mut failures = Vec::new();
    for command in std::iter::once(primary).chain(fallbacks) {
        match runner.run(&command, rendered.as_bytes()) {
            PagerAttempt::Completed => {
                return Ok(Shown {
                    delivery: Delivery::Paged(command),
                    failures,
                });
            }
            PagerAttempt::ClosedEarly => failures.push((command, PagerFailure::ClosedEarly)),
            PagerAttempt::Failed(failure) => failures.push((command, failure)),
        }
    }
    Ok(Shown {
        delivery: Delivery::Direct(print_line(rendered, stdout)?),
        failures,
    })
}

fn print_line(rendered: &str, stdout: &mut dyn Write) -> Result<OutputOutcome, AppError> {
    let mut line = String::with_capacity(rendered.len() + 1);
    line.push_str(rendered);
    line.push('\n');
    Output::new(stdout, Stream::Stdout)
        .write_with_policy(line.as_bytes(), OutputPolicy::ConsoleLike)
}
