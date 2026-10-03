//! Buffered stdout. A reader that closes the pipe early (`linear issue list |
//! head`) ends the command quietly with success; every other write failure is
//! an error.
use std::cell::RefCell;
use std::io::{self, BufWriter, Write};

use crate::error::{Error, Result};

pub struct Stdout {
    inner: RefCell<BufWriter<io::Stdout>>,
}

impl Stdout {
    pub fn new() -> Self {
        Self {
            inner: RefCell::new(BufWriter::new(io::stdout())),
        }
    }

    pub fn write(&self, bytes: &[u8]) -> Result<()> {
        self.inner
            .borrow_mut()
            .write_all(bytes)
            .map_err(write_error)
    }

    pub fn flush(&self) -> Result<()> {
        self.inner.borrow_mut().flush().map_err(write_error)
    }
}

impl Default for Stdout {
    fn default() -> Self {
        Self::new()
    }
}

pub fn write_error(error: io::Error) -> Error {
    if error.kind() == io::ErrorKind::BrokenPipe {
        Error::broken_pipe(error)
    } else {
        Error::new(format!("failed to write to stdout: {error}")).with_source(error)
    }
}

/// Writes to stderr, unbuffered.
pub fn eprint(bytes: &[u8]) -> Result<()> {
    io::stderr().write_all(bytes).map_err(|error| {
        Error::new(format!("failed to write to stderr: {error}")).with_source(error)
    })
}
