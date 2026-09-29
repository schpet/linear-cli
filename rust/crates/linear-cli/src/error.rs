use crate::cli::Route;
use std::error::Error;
use std::fmt;
use std::num::NonZeroU8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExitStatus {
    Success,
    HandledFailure,
    UsageFailure,
    ChildCode(NonZeroU8),
}

impl ExitStatus {
    pub fn code(self) -> u8 {
        match self {
            Self::Success => 0,
            Self::HandledFailure => 1,
            Self::UsageFailure => 2,
            Self::ChildCode(code) => code.get(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppErrorKind {
    Validation,
    Usage { route: Route },
    NotFound,
    Auth,
    GraphQl,
    Transport,
    IoProcess,
    Cancellation,
    Invariant,
    Unimplemented,
}

#[derive(Debug)]
pub struct AppError {
    pub kind: AppErrorKind,
    pub message: String,
    pub context: Option<String>,
    pub suggestion: Option<String>,
    debug_detail: Option<String>,
    source: Option<Box<dyn Error + Send + Sync>>,
}

impl AppError {
    pub fn usage(route: Route, message: impl Into<String>) -> Self {
        Self::new(AppErrorKind::Usage { route }, message)
    }
    pub fn new(kind: AppErrorKind, message: impl Into<String>) -> Self {
        let suggestion = match kind {
            AppErrorKind::Auth => Some("Run `linear auth login` to authenticate.".to_owned()),
            _ => None,
        };
        Self {
            kind,
            message: message.into(),
            context: None,
            suggestion,
            debug_detail: None,
            source: None,
        }
    }

    /// Preserve clap's rendering, stream and exit code without rewording it.
    pub fn native_parser(route: Route, error: clap::Error) -> Self {
        Self::usage(route, error.to_string()).with_source(error)
    }

    pub fn native_parser_error(&self) -> Option<&clap::Error> {
        self.source.as_deref()?.downcast_ref::<clap::Error>()
    }

    pub fn not_found(entity: &str, identifier: &str) -> Self {
        Self::new(
            AppErrorKind::NotFound,
            format!("{entity} not found: {identifier}"),
        )
    }

    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        let context = context.into();
        self.context = Some(match self.context.take() {
            Some(inner) => format!("{context}: {inner}"),
            None => context,
        });
        self
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    pub fn with_source(mut self, source: impl Error + Send + Sync + 'static) -> Self {
        self.source = Some(Box::new(source));
        self
    }

    pub fn with_debug_detail(mut self, detail: impl Into<String>) -> Self {
        self.debug_detail = Some(detail.into());
        self
    }

    pub fn debug_detail(&self) -> Option<&str> {
        self.debug_detail.as_deref()
    }

    pub fn display_message(&self) -> String {
        match &self.context {
            Some(context) => format!("{context}: {}", self.message),
            None => self.message.clone(),
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_message())
    }
}

impl Error for AppError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_ref()
            .map(|source| -> &(dyn Error + 'static) { source.as_ref() })
    }
}
