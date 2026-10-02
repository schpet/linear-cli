//! Keyring access through the platform's command-line tool: `secret-tool` on
//! Linux and `/usr/bin/security` on macOS. Going through `security` keeps
//! existing keychain items readable without a new access prompt, because
//! their access lists already trust that tool.
use std::ffi::OsString;
use std::io;
use std::process::{Output, Stdio};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

use super::{KeyringReader, ReaderFlavor};
use crate::auth::mutation::MutationFailure;
use crate::auth::{LookupFailureCategory, LookupResult};
use crate::config::{ChildEnvOverlay, ConfigSecret};
use crate::error::{AppError, AppErrorKind};
use crate::text::{js_space, js_trim};

/// How long one keyring command may run before it is killed.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// How long output may keep arriving after the tool exits. A pipe stays open
/// past that only when another process inherited it, and that process never
/// writes the tool's output.
const PIPE_GRACE: Duration = Duration::from_millis(500);

/// `security` exits with this status when no matching item exists.
const MAC_NOT_FOUND: i32 = 44;

#[derive(Debug)]
enum RunError {
    Spawn(io::Error),
    Io(io::Error),
    Timeout,
}

/// Runs a keyring command to completion, feeding `input` on stdin.
///
/// The child is killed if it outlives `timeout` or the caller is dropped.
async fn run(
    command: &mut Command,
    input: Option<&[u8]>,
    timeout: Duration,
) -> Result<Output, RunError> {
    command
        .kill_on_drop(true)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(RunError::Spawn)?;
    let stdin = child.stdin.take();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let exchange = async {
        if let (Some(input), Some(mut stdin)) = (input, stdin) {
            stdin.write_all(input).await.map_err(RunError::Io)?;
        }
        let mut stdout_bytes = Vec::new();
        let mut stderr_bytes = Vec::new();
        let status = {
            let reads = async {
                tokio::try_join!(
                    read_into(stdout, &mut stdout_bytes),
                    read_into(stderr, &mut stderr_bytes)
                )
            };
            tokio::pin!(reads);
            let mut drained = false;
            let status = tokio::select! {
                status = child.wait() => status,
                output = &mut reads => {
                    output.map_err(RunError::Io)?;
                    drained = true;
                    child.wait().await
                }
            }
            .map_err(RunError::Io)?;
            if !drained && let Ok(output) = tokio::time::timeout(PIPE_GRACE, reads).await {
                output.map_err(RunError::Io)?;
            }
            status
        };
        Ok(Output {
            status,
            stdout: stdout_bytes,
            stderr: stderr_bytes,
        })
    };
    match tokio::time::timeout(timeout, exchange).await {
        Ok(result) => result,
        Err(_elapsed) => {
            // Kill and reap so no zombie outlives the lookup.
            let _ = child.start_kill();
            let _ = child.wait().await;
            Err(RunError::Timeout)
        }
    }
}

/// Appends everything read from `pipe` to `bytes`, which keeps whatever
/// arrived if this future is dropped early.
async fn read_into(pipe: Option<impl AsyncRead + Unpin>, bytes: &mut Vec<u8>) -> io::Result<()> {
    let Some(mut pipe) = pipe else {
        return Ok(());
    };
    let mut chunk = [0_u8; 4096];
    loop {
        let count = pipe.read(&mut chunk).await?;
        if count == 0 {
            return Ok(());
        }
        bytes.extend_from_slice(chunk.get(..count).unwrap_or_default());
    }
}

/// Why a keyring lookup failed. Never carries the tool's output, which may
/// include the secret.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessLookupFailure {
    Runtime(io::ErrorKind),
    Spawn(io::ErrorKind),
    Io(io::ErrorKind),
    Timeout,
    InvalidUtf8,
    ExitFailure,
}

impl ProcessLookupFailure {
    fn category(self) -> LookupFailureCategory {
        match self {
            Self::Spawn(io::ErrorKind::NotFound) => LookupFailureCategory::Unavailable,
            Self::Spawn(io::ErrorKind::PermissionDenied) => LookupFailureCategory::Permission,
            Self::Runtime(_)
            | Self::Spawn(_)
            | Self::Io(_)
            | Self::Timeout
            | Self::InvalidUtf8
            | Self::ExitFailure => LookupFailureCategory::Other,
        }
    }
}

/// Reads API keys with the platform's keyring tool.
pub struct ProcessKeyringReader {
    flavor: ReaderFlavor,
    executable: OsString,
    timeout: Duration,
}

impl ProcessKeyringReader {
    pub fn new(flavor: ReaderFlavor) -> Self {
        Self::with_executable(flavor, OsString::from(flavor.executable()))
    }

    /// Runs `executable` instead of the platform tool, with the same
    /// arguments.
    pub fn with_executable(flavor: ReaderFlavor, executable: OsString) -> Self {
        Self {
            flavor,
            executable,
            timeout: DEFAULT_TIMEOUT,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// `Ok(None)` when no key is stored for the workspace.
    pub fn lookup_detailed(
        &self,
        workspace: &str,
    ) -> Result<Option<ConfigSecret>, ProcessLookupFailure> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| ProcessLookupFailure::Runtime(error.kind()))?;
        let mut command = Command::new(&self.executable);
        command.args(self.flavor.lookup_arguments(workspace));
        let output = runtime
            .block_on(run(&mut command, None, self.timeout))
            .map_err(|error| match error {
                RunError::Spawn(error) => ProcessLookupFailure::Spawn(error.kind()),
                RunError::Io(error) => ProcessLookupFailure::Io(error.kind()),
                RunError::Timeout => ProcessLookupFailure::Timeout,
            })?;
        if output.status.success() {
            let stdout =
                String::from_utf8(output.stdout).map_err(|_| ProcessLookupFailure::InvalidUtf8)?;
            let key = match self.flavor {
                // `security -w` prints the password followed by a newline.
                ReaderFlavor::MacSecurity => js_trim(&stdout).to_owned(),
                ReaderFlavor::SecretTool => stdout,
            };
            return Ok((!key.is_empty()).then(|| ConfigSecret::new(key)));
        }
        let missing = match self.flavor {
            ReaderFlavor::MacSecurity => output.status.code() == Some(MAC_NOT_FOUND),
            // secret-tool exits 1 without a message when nothing matches.
            ReaderFlavor::SecretTool => {
                output.status.code() == Some(1)
                    && String::from_utf8(output.stderr)
                        .is_ok_and(|stderr| stderr.trim_matches(js_space).is_empty())
            }
        };
        if missing {
            Ok(None)
        } else {
            Err(ProcessLookupFailure::ExitFailure)
        }
    }
}

impl KeyringReader for ProcessKeyringReader {
    fn lookup(&self, workspace: &str) -> LookupResult {
        match self.lookup_detailed(workspace) {
            Ok(Some(key)) => LookupResult::Hit(key),
            Ok(None) => LookupResult::Miss,
            Err(error) => LookupResult::Failed(error.category()),
        }
    }
}

/// Stores and deletes API keys with the platform's keyring tool. The child
/// sees the `.env` overlay, like every other subprocess.
pub struct ProcessMutationBackend {
    pub overlay: ChildEnvOverlay,
}

impl ProcessMutationBackend {
    #[cfg(target_os = "linux")]
    const FLAVOR: ReaderFlavor = ReaderFlavor::SecretTool;
    #[cfg(not(target_os = "linux"))]
    const FLAVOR: ReaderFlavor = ReaderFlavor::MacSecurity;

    fn tool_name() -> &'static str {
        match Self::FLAVOR {
            ReaderFlavor::SecretTool => "secret-tool",
            ReaderFlavor::MacSecurity => "security",
        }
    }

    async fn run(&self, args: &[String], input: Option<&[u8]>) -> Result<Output, MutationFailure> {
        if args.iter().any(|arg| arg.contains('\0')) {
            return Err(MutationFailure::Typed(AppError::new(
                AppErrorKind::Validation,
                "Keyring arguments cannot contain a NUL character",
            )));
        }
        let executable = Self::FLAVOR.executable();
        let mut command = Command::new(executable);
        command.args(args).envs(self.overlay.iter());
        run(&mut command, input, DEFAULT_TIMEOUT)
            .await
            .map_err(|error| match error {
                RunError::Spawn(error) => MutationFailure::Ordinary(match Self::FLAVOR {
                    ReaderFlavor::SecretTool => format!(
                        "Could not run secret-tool. Install libsecret (e.g. apt install libsecret-tools, pacman -S libsecret).\nAlternatively, set the LINEAR_API_KEY environment variable.\n  ({error})"
                    ),
                    ReaderFlavor::MacSecurity => format!(
                        "Could not run {executable}. Is this a macOS system?\n  ({error})"
                    ),
                }),
                RunError::Io(error) => MutationFailure::Ordinary(format!(
                    "{} failed: {error}",
                    Self::tool_name()
                )),
                RunError::Timeout => MutationFailure::Ordinary(format!(
                    "{} did not finish within {} seconds",
                    Self::tool_name(),
                    DEFAULT_TIMEOUT.as_secs()
                )),
            })
    }

    /// Fails unless the tool exited with one of `accepted`.
    fn check(output: &Output, action: &str, accepted: &[i32]) -> Result<(), MutationFailure> {
        let code = output.status.code();
        if code.is_some_and(|code| accepted.contains(&code)) {
            return Ok(());
        }
        let status = code.map_or_else(|| output.status.to_string(), |code| format!("exit {code}"));
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(MutationFailure::Ordinary(format!(
            "{} {action} failed ({status}): {}",
            Self::tool_name(),
            js_trim(&stderr)
        )))
    }
}

impl crate::auth::mutation::CredentialMutationBackend for ProcessMutationBackend {
    async fn available(&self) -> bool {
        match Self::FLAVOR {
            ReaderFlavor::SecretTool => self.run(&[], None).await.is_ok(),
            ReaderFlavor::MacSecurity => true,
        }
    }

    async fn store(&self, workspace: &str, secret: &ConfigSecret) -> Result<(), MutationFailure> {
        let args = Self::FLAVOR.store_arguments(workspace, secret);
        // secret-tool reads the secret from stdin; `security` takes it as an
        // argument.
        let input = match Self::FLAVOR {
            ReaderFlavor::SecretTool => Some(secret.expose().as_bytes()),
            ReaderFlavor::MacSecurity => None,
        };
        let output = self.run(&args, input).await?;
        Self::check(&output, Self::FLAVOR.store_action(), &[0])
    }

    async fn delete(&self, workspace: &str) -> Result<(), MutationFailure> {
        let args = Self::FLAVOR.delete_arguments(workspace);
        let output = self.run(&args, None).await?;
        let accepted: &[i32] = match Self::FLAVOR {
            ReaderFlavor::SecretTool => &[0],
            ReaderFlavor::MacSecurity => &[0, MAC_NOT_FOUND],
        };
        Self::check(&output, Self::FLAVOR.delete_action(), accepted)
    }
}
