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
        self.writer.write_all(bytes).map_err(|source| {
            OutputFailure {
                stream: self.stream,
                operation: Operation::Write,
                source,
            }
            .app_error()
        })?;
        self.flush()
    }

    pub fn flush(&mut self) -> Result<(), AppError> {
        self.writer.flush().map_err(|source| {
            OutputFailure {
                stream: self.stream,
                operation: Operation::Flush,
                source,
            }
            .app_error()
        })
    }
}
