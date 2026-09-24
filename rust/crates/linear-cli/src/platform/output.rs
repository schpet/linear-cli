use std::error::Error;
use std::fmt;
use std::io::{self, Write};

use crate::error::{AppError, AppErrorKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stream {
    Stdout,
    Stderr,
}

impl Stream {
    fn label(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Operation {
    Write,
    Flush,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputPolicy {
    Strict,
    ConsoleLike,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputOutcome {
    Written,
    QuietBrokenPipe,
}

impl Operation {
    fn label(self) -> &'static str {
        match self {
            Self::Write => "write",
            Self::Flush => "flush",
        }
    }
}

#[derive(Debug)]
pub struct OutputFailure {
    pub stream: Stream,
    operation: Operation,
    source: io::Error,
}

impl OutputFailure {
    pub fn is_broken_pipe(&self) -> bool {
        self.source.kind() == io::ErrorKind::BrokenPipe
    }

    fn app_error(self) -> AppError {
        AppError::new(
            AppErrorKind::IoProcess,
            format!(
                "failed to {} {}",
                self.operation.label(),
                self.stream.label()
            ),
        )
        .with_source(self)
    }
}

impl fmt::Display for OutputFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "failed to {} {}",
            self.operation.label(),
            self.stream.label()
        )
    }
}

impl Error for OutputFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

pub fn failed_stream(error: &AppError) -> Option<Stream> {
    error
        .source()
        .and_then(|source| source.downcast_ref::<OutputFailure>())
        .map(|failure| failure.stream)
}

pub struct Output<'a> {
    writer: &'a mut dyn Write,
    stream: Stream,
}

impl<'a> Output<'a> {
    pub fn new(writer: &'a mut dyn Write, stream: Stream) -> Self {
        Self { writer, stream }
    }

    pub fn write(&mut self, bytes: &[u8]) -> Result<(), AppError> {
        self.write_with_policy(bytes, OutputPolicy::Strict)
            .map(|_| ())
    }

    pub fn flush(&mut self) -> Result<(), AppError> {
        self.flush_with_policy(OutputPolicy::Strict).map(|_| ())
    }

    pub fn write_with_policy(
        &mut self,
        bytes: &[u8],
        policy: OutputPolicy,
    ) -> Result<OutputOutcome, AppError> {
        match self.writer.write_all(bytes) {
            Ok(()) => self.flush_with_policy(policy),
            Err(source) => self.resolve_failure(Operation::Write, source, policy),
        }
    }

    pub fn flush_with_policy(&mut self, policy: OutputPolicy) -> Result<OutputOutcome, AppError> {
        match self.writer.flush() {
            Ok(()) => Ok(OutputOutcome::Written),
            Err(source) => self.resolve_failure(Operation::Flush, source, policy),
        }
    }

    fn resolve_failure(
        &self,
        operation: Operation,
        source: io::Error,
        policy: OutputPolicy,
    ) -> Result<OutputOutcome, AppError> {
        let failure = OutputFailure {
            stream: self.stream,
            operation,
            source,
        };
        if policy == OutputPolicy::ConsoleLike
            && failure.stream == Stream::Stdout
            && failure.is_broken_pipe()
        {
            Ok(OutputOutcome::QuietBrokenPipe)
        } else {
            Err(failure.app_error())
        }
    }
}
