//! The one error type every command returns.
//!
//! An [`Error`] is a message plus an optional chain of context ("Failed to list
//! cycles"), a hint line, and a source error shown under `LINEAR_DEBUG`. Its
//! [`ErrorKind`] exists only where the process must behave differently, and a
//! runtime failure's [`Failure`] class picks its exit status.
use std::error::Error as StdError;
use std::fmt;
use std::num::NonZeroU8;

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Why a command failed at run time, which scripts read from the exit status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    /// Anything not below: Linear rejected the input, a response could not be
    /// used, a local file could not be read. Exit status 1.
    General,
    /// Something the command looked up in Linear does not exist. Exit status 3.
    NotFound,
    /// No usable API key, or Linear rejected the key. Exit status 4.
    Auth,
    /// Linear could not be reached or could not serve the request now (a
    /// network failure, a timeout, rate limiting, a server error). Exit status 5.
    Unavailable,
}

impl Failure {
    pub fn exit_code(self) -> u8 {
        match self {
            Self::General => 1,
            Self::NotFound => 3,
            Self::Auth => 4,
            Self::Unavailable => 5,
        }
    }

    /// The class that describes both failures: the one that needs attention
    /// first. Authentication outranks unavailability, which outranks a general
    /// failure, which outranks a missing entity, so a run reports "not found"
    /// only when nothing worse happened.
    pub fn combine(self, other: Self) -> Self {
        if self.rank() >= other.rank() {
            self
        } else {
            other
        }
    }

    /// The combined class of `failures`, or `None` when there are none.
    pub fn fold(failures: impl IntoIterator<Item = Self>) -> Option<Self> {
        failures.into_iter().reduce(Self::combine)
    }

    fn rank(self) -> u8 {
        match self {
            Self::NotFound => 0,
            Self::General => 1,
            Self::Unavailable => 2,
            Self::Auth => 3,
        }
    }
}

#[derive(Debug)]
pub enum ErrorKind {
    /// A runtime failure: `✗ message`, with its class's exit status.
    Failed(Failure),
    /// Input that parsed but cannot be used, such as a missing required value
    /// or an empty field: reported like [`ErrorKind::Failed`], exit status 2
    /// like any usage error.
    Invalid,
    /// A command-line usage error, rendered and given its exit status by clap.
    Usage(clap::Error),
    /// The user cancelled a prompt (Ctrl-C or Esc) or the editor: exit status
    /// 130 after `Canceled.`.
    Cancelled,
    /// The command already reported this failure itself: its class's exit
    /// status, no message.
    Reported(Failure),
    /// A child process's status passed on, such as 143 after SIGTERM: exit
    /// with it, no message.
    Exit(NonZeroU8),
    /// Stdout was closed by its reader: stop quietly with success.
    BrokenPipe,
}

pub struct Error {
    kind: ErrorKind,
    message: String,
    /// Outermost context first.
    context: Vec<String>,
    hint: Option<Box<str>>,
    debug_detail: Option<Box<str>>,
    source: Option<Box<dyn StdError + Send + Sync>>,
}

const LOGIN_HINT: &str = "Run `linear auth login` to authenticate.";

impl Error {
    fn with_kind(kind: ErrorKind, message: String) -> Self {
        Self {
            kind,
            message,
            context: Vec::new(),
            hint: None,
            debug_detail: None,
            source: None,
        }
    }

    /// A [`Failure::General`] runtime failure.
    pub fn new(message: impl Into<String>) -> Self {
        Self::failed(Failure::General, message)
    }

    /// A runtime failure of class `failure`.
    pub fn failed(failure: Failure, message: impl Into<String>) -> Self {
        Self::with_kind(ErrorKind::Failed(failure), message.into())
    }

    /// A usage error found after parsing; see [`ErrorKind::Invalid`].
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::with_kind(ErrorKind::Invalid, message.into())
    }

    /// Missing or rejected credentials, with a hint to log in.
    pub fn auth(message: impl Into<String>) -> Self {
        Self::failed(Failure::Auth, message).with_hint(LOGIN_HINT)
    }

    /// A Linear entity (issue, team, label…) that does not exist. Not for
    /// local things such as files, which fail with [`Error::new`].
    pub fn not_found(entity: &str, identifier: &str) -> Self {
        Self::failed(
            Failure::NotFound,
            format!("{entity} not found: {identifier}"),
        )
    }

    pub fn cancelled() -> Self {
        Self::with_kind(ErrorKind::Cancelled, "Cancelled".to_owned())
    }

    /// Exit with a child process's `status`, which already reported itself.
    pub fn exit(status: NonZeroU8) -> Self {
        Self::with_kind(ErrorKind::Exit(status), format!("exit status {status}"))
    }

    /// The command printed its own report of a `failure`; exit with its status.
    pub fn reported(failure: Failure) -> Self {
        Self::with_kind(
            ErrorKind::Reported(failure),
            format!("exit status {}", failure.exit_code()),
        )
    }

    pub(crate) fn broken_pipe(source: std::io::Error) -> Self {
        Self::with_kind(ErrorKind::BrokenPipe, "stdout was closed".to_owned()).with_source(source)
    }

    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }

    /// The runtime failure class, or `None` for usage errors, cancellation,
    /// passed-on child statuses and a closed stdout.
    pub fn failure(&self) -> Option<Failure> {
        match &self.kind {
            ErrorKind::Failed(failure) | ErrorKind::Reported(failure) => Some(*failure),
            ErrorKind::Invalid
            | ErrorKind::Usage(_)
            | ErrorKind::Cancelled
            | ErrorKind::Exit(_)
            | ErrorKind::BrokenPipe => None,
        }
    }

    /// The message without context.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Extends the message, as in "...; the comment may already exist".
    pub fn push_message(&mut self, text: &str) {
        self.message.push_str(text);
    }

    pub fn hint(&self) -> Option<&str> {
        self.hint.as_deref()
    }

    pub fn debug_detail(&self) -> Option<&str> {
        self.debug_detail.as_deref()
    }

    /// Prefix the message with `context`, outermost last added.
    pub fn context(mut self, context: impl Into<String>) -> Self {
        self.context.insert(0, context.into());
        self
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into().into_boxed_str());
        self
    }

    pub fn with_source(mut self, source: impl StdError + Send + Sync + 'static) -> Self {
        self.source = Some(Box::new(source));
        self
    }

    pub fn with_debug_detail(mut self, detail: impl Into<String>) -> Self {
        self.debug_detail = Some(detail.into().into_boxed_str());
        self
    }

    /// The process exit status this error ends with.
    pub fn exit_code(&self) -> u8 {
        match &self.kind {
            ErrorKind::Failed(failure) | ErrorKind::Reported(failure) => failure.exit_code(),
            ErrorKind::Invalid => 2,
            ErrorKind::Usage(error) => u8::try_from(error.exit_code()).unwrap_or(2),
            ErrorKind::Cancelled => 130,
            ErrorKind::Exit(status) => status.get(),
            ErrorKind::BrokenPipe => 0,
        }
    }
}

impl From<clap::Error> for Error {
    fn from(error: clap::Error) -> Self {
        let message = error.to_string();
        Self::with_kind(ErrorKind::Usage(error), message)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for context in &self.context {
            write!(f, "{context}: ")?;
        }
        f.write_str(&self.message)
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Error")
            .field("kind", &self.kind)
            .field("message", &self.to_string())
            .field("hint", &self.hint)
            .field("source", &self.source)
            .finish()
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source
            .as_ref()
            .map(|source| -> &(dyn StdError + 'static) { source.as_ref() })
    }
}

/// Attach context to any result whose error converts into [`Error`].
pub trait ResultExt<T> {
    fn context(self, context: impl Into<String>) -> Result<T>;
}

impl<T, E: Into<Error>> ResultExt<T> for std::result::Result<T, E> {
    fn context(self, context: impl Into<String>) -> Result<T> {
        self.map_err(|error| error.into().context(context))
    }
}

#[cfg(test)]
mod tests;
