//! Bounded Linux `secret-tool lookup` adapter.
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use tokio::time::{self, Instant};

use crate::auth::{LookupFailureCategory, LookupResult};
use crate::config::ConfigSecret;

use super::KeyringReader;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const PIPE_GRACE: Duration = Duration::from_millis(500);
const MAX_STDOUT: usize = 64 * 1024;
const MAX_STDERR: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinuxLookupFailure {
    Runtime(io::ErrorKind),
    Spawn(io::ErrorKind),
    ReadStdout(io::ErrorKind),
    ReadStderr(io::ErrorKind),
    Wait(io::ErrorKind),
    MissingPipe,
    Timeout,
    PipeHeldOpen,
    StdoutTooLarge,
    StderrTooLarge,
    InvalidUtf8,
    ExitFailure,
}

impl LinuxLookupFailure {
    fn category(self) -> LookupFailureCategory {
        match self {
            Self::Spawn(io::ErrorKind::NotFound) => LookupFailureCategory::Unavailable,
            Self::Spawn(io::ErrorKind::PermissionDenied) => LookupFailureCategory::Permission,
            _ => LookupFailureCategory::Other,
        }
    }
}

enum Environment {
    Inherit,
    Replace(BTreeMap<OsString, OsString>),
}

pub struct LinuxKeyringReader {
    executable: OsString,
    environment: Environment,
    timeout: Duration,
}

impl Default for LinuxKeyringReader {
    fn default() -> Self {
        Self::new()
    }
}

impl LinuxKeyringReader {
    pub fn new() -> Self {
        Self {
            executable: OsString::from("secret-tool"),
            environment: Environment::Inherit,
            timeout: DEFAULT_TIMEOUT,
        }
    }

    /// Confine a test to its private executable and complete private environment.
    #[doc(hidden)]
    pub fn with_test_environment(
        executable: OsString,
        environment: BTreeMap<OsString, OsString>,
        timeout: Duration,
    ) -> Self {
        Self {
            executable,
            environment: Environment::Replace(environment),
            timeout,
        }
    }

    pub fn lookup_detailed(
        &self,
        workspace: &str,
    ) -> Result<Option<ConfigSecret>, LinuxLookupFailure> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| LinuxLookupFailure::Runtime(error.kind()))?;
        runtime.block_on(self.lookup_async(workspace))
    }

    async fn lookup_async(
        &self,
        workspace: &str,
    ) -> Result<Option<ConfigSecret>, LinuxLookupFailure> {
        let mut command = Command::new(&self.executable);
        command
            .arg("lookup")
            .arg("service")
            .arg("linear-cli")
            .arg("account")
            .arg(workspace)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Environment::Replace(environment) = &self.environment {
            command.env_clear().envs(environment);
        }
        let mut child = command
            .spawn()
            .map_err(|error| LinuxLookupFailure::Spawn(error.kind()))?;
        let (stdout, stderr) = match (child.stdout.take(), child.stderr.take()) {
            (Some(stdout), Some(stderr)) => (stdout, stderr),
            _ => {
                let _ = child.start_kill();
                let _ = child.wait().await;
                return Err(LinuxLookupFailure::MissingPipe);
            }
        };
        let started = Instant::now();
        let deadline = started + self.timeout;
        let result = {
            let out_future = read_capped(stdout, MAX_STDOUT);
            let err_future = read_capped(stderr, MAX_STDERR);
            let wait_future = child.wait();
            tokio::pin!(out_future, err_future, wait_future);
            let mut out = None;
            let mut err = None;
            let mut status = None;
            let mut pipe_deadline = deadline;
            loop {
                tokio::select! {
                    bytes = &mut out_future, if out.is_none() => match bytes {
                        Ok(bytes) if bytes.len() <= MAX_STDOUT => out = Some(bytes),
                        Ok(_) => break Err(LinuxLookupFailure::StdoutTooLarge),
                        Err(kind) => break Err(LinuxLookupFailure::ReadStdout(kind)),
                    },
                    bytes = &mut err_future, if err.is_none() => match bytes {
                        Ok(bytes) if bytes.len() <= MAX_STDERR => err = Some(bytes),
                        Ok(_) => break Err(LinuxLookupFailure::StderrTooLarge),
                        Err(kind) => break Err(LinuxLookupFailure::ReadStderr(kind)),
                    },
                    waited = &mut wait_future, if status.is_none() => match waited {
                        Ok(waited) => {
                            status = Some(waited);
                            pipe_deadline = deadline.min(Instant::now() + PIPE_GRACE);
                        }
                        Err(error) => break Err(LinuxLookupFailure::Wait(error.kind())),
                    },
                    () = time::sleep_until(pipe_deadline) => {
                        break Err(if status.is_some() { LinuxLookupFailure::PipeHeldOpen } else { LinuxLookupFailure::Timeout });
                    },
                }
                if out.is_some() && err.is_some() && status.is_some() {
                    break match (out.take(), err.take(), status.take()) {
                        (Some(out), Some(err), Some(status)) => Ok((out, err, status)),
                        _ => Err(LinuxLookupFailure::MissingPipe),
                    };
                }
            }
        };
        let (mut stdout, stderr, status) = match result {
            Ok(result) => result,
            Err(error) => {
                let _ = child.start_kill();
                let _ = child.wait().await;
                return Err(error);
            }
        };
        if status.success() {
            if stdout.starts_with(&[0xef, 0xbb, 0xbf]) {
                stdout.drain(..3);
            }
            let stdout = String::from_utf8(stdout).map_err(|_| LinuxLookupFailure::InvalidUtf8)?;
            return if stdout.is_empty() {
                Ok(None)
            } else {
                Ok(Some(ConfigSecret::new(stdout)))
            };
        }
        let stderr_is_empty = String::from_utf8(stderr)
            .ok()
            .is_some_and(|stderr| stderr.trim_matches(js_whitespace).is_empty());
        if status.code() == Some(1) && stderr_is_empty {
            Ok(None)
        } else {
            Err(LinuxLookupFailure::ExitFailure)
        }
    }
}

async fn read_capped(reader: impl AsyncRead + Unpin, max: usize) -> Result<Vec<u8>, io::ErrorKind> {
    let mut bytes = Vec::new();
    let limit = u64::try_from(max).map_err(|_| io::ErrorKind::InvalidInput)? + 1;
    reader
        .take(limit)
        .read_to_end(&mut bytes)
        .await
        .map_err(|error| error.kind())?;
    Ok(bytes)
}

fn js_whitespace(character: char) -> bool {
    matches!(
        character,
        '\u{0009}'..='\u{000d}'
            | '\u{0020}'
            | '\u{00a0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200a}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202f}'
            | '\u{205f}'
            | '\u{3000}'
            | '\u{feff}'
    )
}

impl KeyringReader for LinuxKeyringReader {
    fn lookup(&self, workspace: &str) -> LookupResult {
        match self.lookup_detailed(workspace) {
            Ok(Some(key)) => LookupResult::Hit(key),
            Ok(None) => LookupResult::Miss,
            Err(error) => LookupResult::Failed(error.category()),
        }
    }
}
