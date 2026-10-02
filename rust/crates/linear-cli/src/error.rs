//! The one error type every command returns.
//!
//! An [`Error`] is a message plus an optional chain of context ("Failed to list
//! cycles"), a hint line, and a source error shown under `LINEAR_DEBUG`. Its
//! [`ErrorKind`] exists only where the process must behave differently.
use std::error::Error as StdError;
use std::fmt;
use std::num::NonZeroU8;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    /// An ordinary failure: `✗ message`, exit status 1.
    Other,
    /// Missing or rejected credentials; rendered with a login hint.
    Auth,
    /// The requested entity does not exist.
    NotFound,
    /// A command-line usage error reported by clap: exit status 2.
    Usage,
    /// The user cancelled a prompt (Ctrl-C or Esc): exit status 130, no message.
    Cancelled,
    /// The command already reported its failure: exit with this status, no message.
    Exit(NonZeroU8),
    /// Stdout was closed by its reader: stop quietly with success.
    BrokenPipe,
}

pub struct Error {
    kind: ErrorKind,
    message: String,
    /// Outermost context first.
    context: Vec<String>,
    hint: Option<String>,
    debug_detail: Option<String>,
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

    pub fn new(message: impl Into<String>) -> Self {
        Self::with_kind(ErrorKind::Other, message.into())
    }

    pub fn auth(message: impl Into<String>) -> Self {
        Self::with_kind(ErrorKind::Auth, message.into()).with_hint(LOGIN_HINT)
    }

    pub fn not_found(entity: &str, identifier: &str) -> Self {
        Self::with_kind(
            ErrorKind::NotFound,
            format!("{entity} not found: {identifier}"),
        )
    }

    pub fn cancelled() -> Self {
        Self::with_kind(ErrorKind::Cancelled, "Cancelled".to_owned())
    }

    /// The command printed its own failure report; exit with `status`.
    pub fn exit(status: NonZeroU8) -> Self {
        Self::with_kind(ErrorKind::Exit(status), format!("exit status {status}"))
    }

    /// Exit status 1 after the command printed its own failure report.
    pub fn reported() -> Self {
        Self::exit(NonZeroU8::MIN)
    }

    pub(crate) fn broken_pipe(source: std::io::Error) -> Self {
        Self::with_kind(ErrorKind::BrokenPipe, "stdout was closed".to_owned()).with_source(source)
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
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

    pub fn has_context(&self) -> bool {
        !self.context.is_empty()
    }

    /// Prefix the message with `context`, outermost last added.
    pub fn context(mut self, context: impl Into<String>) -> Self {
        self.context.insert(0, context.into());
        self
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn with_source(mut self, source: impl StdError + Send + Sync + 'static) -> Self {
        self.source = Some(Box::new(source));
        self
    }

    pub fn with_debug_detail(mut self, detail: impl Into<String>) -> Self {
        self.debug_detail = Some(detail.into());
        self
    }

    pub fn usage_error(&self) -> Option<&clap::Error> {
        self.source.as_deref()?.downcast_ref::<clap::Error>()
    }

    /// The process exit status this error ends with.
    pub fn exit_code(&self) -> u8 {
        match self.kind {
            ErrorKind::Other | ErrorKind::Auth | ErrorKind::NotFound => 1,
            ErrorKind::Usage => self
                .usage_error()
                .map_or(2, |error| u8::try_from(error.exit_code()).unwrap_or(2)),
            ErrorKind::Cancelled => 130,
            ErrorKind::Exit(status) => status.get(),
            ErrorKind::BrokenPipe => 0,
        }
    }
}

impl From<clap::Error> for Error {
    fn from(error: clap::Error) -> Self {
        Self::with_kind(ErrorKind::Usage, error.to_string()).with_source(error)
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
