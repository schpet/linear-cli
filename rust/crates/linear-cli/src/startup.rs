//! Config-first startup orchestration and read-only credential loading.
use std::fmt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::auth::file::{CredentialFileSource, CredentialReadFailure};
use crate::auth::keyring::KeyringReader;
use crate::auth::{
    CredentialFormatErrorKind, CredentialManifest, CredentialStore, CredentialWarning,
    LookupFailureCategory, LookupReply, LookupResult, credentials_path, hydrate, parse_credentials,
};
use crate::config::{
    ConfigDiagnostic, ConfigParseErrorKind, FileSource, GitRootProbe, ProcessEnvSnapshot,
    RawConfigFile, StartupConfig, StartupError, parse_config_tier, render_diagnostic,
};
use crate::error::{AppError, AppErrorKind};

const DEFAULT_PHASE_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_WORKERS: usize = 8;

pub struct AppStartupConfig {
    pub config: StartupConfig,
    pub credentials: CredentialStore,
    pub credentials_path: Option<PathBuf>,
}

impl fmt::Debug for AppStartupConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AppStartupConfig(<redacted>)")
    }
}

#[derive(Clone, Debug)]
pub enum AppStartupDiagnostic {
    Config(ConfigDiagnostic),
    Credential(CredentialWarning),
}

pub enum AppStartupError {
    Config(StartupError),
    CredentialRead {
        path: PathBuf,
        reason: CredentialReadFailure,
    },
    CredentialParse {
        path: PathBuf,
        reason: ConfigParseErrorKind,
    },
    CredentialFormat {
        path: PathBuf,
        reason: CredentialFormatErrorKind,
    },
    Invariant,
}

impl fmt::Debug for AppStartupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(error) => f.debug_tuple("Config").field(error).finish(),
            Self::CredentialRead { path, reason } => f
                .debug_struct("CredentialRead")
                .field("path", path)
                .field("reason", reason)
                .finish(),
            Self::CredentialParse { path, reason } => f
                .debug_struct("CredentialParse")
                .field("path", path)
                .field("reason", reason)
                .finish(),
            Self::CredentialFormat { path, reason } => f
                .debug_struct("CredentialFormat")
                .field("path", path)
                .field("reason", reason)
                .finish(),
            Self::Invariant => f.write_str("Invariant"),
        }
    }
}

impl AppStartupError {
    pub fn app_error(&self) -> AppError {
        let (path, detail) = match self {
            Self::Config(error) => return error.app_error(),
            Self::CredentialRead { path, reason } => {
                let detail = match reason {
                    CredentialReadFailure::NotRegular => "not a regular file".to_owned(),
                    CredentialReadFailure::TooLarge => "too large".to_owned(),
                    CredentialReadFailure::Io(kind) => format!("read failed: {kind}"),
                };
                (path, detail)
            }
            Self::CredentialParse { path, reason } => {
                let detail = match reason {
                    ConfigParseErrorKind::TooLarge => "too large",
                    ConfigParseErrorKind::InvalidUtf8 => "invalid UTF-8",
                    ConfigParseErrorKind::ByteOrderMark => "byte-order mark",
                    ConfigParseErrorKind::InvalidToml => "invalid TOML",
                    ConfigParseErrorKind::TooDeep => "nesting too deep",
                };
                (path, detail.to_owned())
            }
            Self::CredentialFormat { path, reason } => {
                let detail = match reason {
                    CredentialFormatErrorKind::MixedFormat => "mixed credential formats",
                    CredentialFormatErrorKind::WrongType => "invalid value type",
                    CredentialFormatErrorKind::EmptyWorkspace => "empty workspace name",
                    CredentialFormatErrorKind::TooManyWorkspaces => "too many workspaces",
                };
                (path, detail.to_owned())
            }
            Self::Invariant => {
                return AppError::new(
                    AppErrorKind::Invariant,
                    "credential startup invariant failed",
                );
            }
        };
        AppError::new(
            AppErrorKind::Validation,
            format!("invalid credentials file {}: {detail}", path.display()),
        )
        .with_suggestion("Fix or remove the credentials file, then run `linear auth login`.")
    }
}

pub struct AppStartupReport {
    pub settings: crate::config::DisplaySettings,
    pub diagnostics: Vec<AppStartupDiagnostic>,
    pub result: Result<AppStartupConfig, AppStartupError>,
}

impl fmt::Debug for AppStartupReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppStartupReport")
            .field("settings", &self.settings)
            .field("diagnostics", &self.diagnostics)
            .field("result", &self.result.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

pub fn render_startup_diagnostic(diagnostic: &AppStartupDiagnostic, color: bool) -> String {
    match diagnostic {
        AppStartupDiagnostic::Config(config) => render_diagnostic(config, color),
        AppStartupDiagnostic::Credential(credential) => {
            let message = match credential {
                CredentialWarning::InvalidDefault { workspace } => format!(
                    "Warning: Default workspace \"{workspace}\" is not in the workspaces list. Run `linear auth default <workspace>` to set a valid default."
                ),
                CredentialWarning::LookupMiss { workspace } => format!(
                    "Warning: No keyring entry for workspace \"{workspace}\". Run `linear auth login` to re-authenticate."
                ),
                CredentialWarning::LookupFailed {
                    workspace,
                    category,
                } => {
                    let reason = match category {
                        LookupFailureCategory::Unavailable => "keyring tool unavailable",
                        LookupFailureCategory::Permission => "permission denied",
                        LookupFailureCategory::Other => "lookup failed",
                        LookupFailureCategory::UnsupportedPlatform => "unsupported platform",
                    };
                    format!(
                        "Warning: Failed to read keyring for workspace \"{workspace}\": {reason}"
                    )
                }
            };
            if color {
                format!("\x1b[33m{message}\x1b[39m\n")
            } else {
                format!("{message}\n")
            }
        }
    }
}

pub fn load(
    process: &ProcessEnvSnapshot,
    files: &impl FileSource,
    git: &impl GitRootProbe,
    credential_files: &impl CredentialFileSource,
    keyring: &impl KeyringReader,
) -> AppStartupReport {
    load_with_phase_timeout(
        process,
        files,
        git,
        credential_files,
        keyring,
        DEFAULT_PHASE_TIMEOUT,
    )
}

/// Inject a shorter phase deadline for deterministic fake-backed tests.
#[doc(hidden)]
pub fn load_with_phase_timeout(
    process: &ProcessEnvSnapshot,
    files: &impl FileSource,
    git: &impl GitRootProbe,
    credential_files: &impl CredentialFileSource,
    keyring: &impl KeyringReader,
    phase_timeout: Duration,
) -> AppStartupReport {
    load_with_policy(
        process,
        files,
        git,
        credential_files,
        keyring,
        phase_timeout,
        crate::config::StartupOptionPolicy::Eager,
    )
}
/// Source mine/query defer sort until the resolver pipeline reaches it.
pub fn load_for_issue_reads(
    process: &ProcessEnvSnapshot,
    files: &impl FileSource,
    git: &impl GitRootProbe,
    credential_files: &impl CredentialFileSource,
    keyring: &impl KeyringReader,
) -> AppStartupReport {
    load_with_policy(
        process,
        files,
        git,
        credential_files,
        keyring,
        DEFAULT_PHASE_TIMEOUT,
        crate::config::StartupOptionPolicy::IssueSort,
    )
}
/// Pull-request alone defers all template-option validation until action priority.
pub fn load_for_pull_request(
    process: &ProcessEnvSnapshot,
    files: &impl FileSource,
    git: &impl GitRootProbe,
    credential_files: &impl CredentialFileSource,
    keyring: &impl KeyringReader,
) -> AppStartupReport {
    load_with_policy(
        process,
        files,
        git,
        credential_files,
        keyring,
        DEFAULT_PHASE_TIMEOUT,
        crate::config::StartupOptionPolicy::PullRequestTemplate,
    )
}
fn load_with_policy(
    process: &ProcessEnvSnapshot,
    files: &impl FileSource,
    git: &impl GitRootProbe,
    credential_files: &impl CredentialFileSource,
    keyring: &impl KeyringReader,
    phase_timeout: Duration,
    policy: crate::config::StartupOptionPolicy,
) -> AppStartupReport {
    let config_report = crate::config::load_startup_with_policy(process, files, git, policy);
    let mut diagnostics = config_report
        .diagnostics
        .into_iter()
        .map(AppStartupDiagnostic::Config)
        .collect::<Vec<_>>();
    let config = match config_report.result {
        Ok(config) => config,
        Err(error) => {
            return AppStartupReport {
                settings: config_report.settings,
                diagnostics,
                result: Err(AppStartupError::Config(error)),
            };
        }
    };
    let credentials_path = credentials_path(
        process.inputs.os,
        process.inputs.env("XDG_CONFIG_HOME"),
        process.inputs.env("HOME"),
        process.inputs.env("APPDATA"),
    );
    let manifest = match credentials_path.clone() {
        Some(path) => match credential_files.read_credentials(&path) {
            Ok(Some(bytes)) => {
                let tier = match parse_config_tier(RawConfigFile {
                    path: path.clone(),
                    bytes,
                }) {
                    Ok(tier) => tier,
                    Err(error) => {
                        return AppStartupReport {
                            settings: config_report.settings,
                            diagnostics,
                            result: Err(AppStartupError::CredentialParse {
                                path,
                                reason: error.kind,
                            }),
                        };
                    }
                };
                match parse_credentials(tier) {
                    Ok(manifest) => manifest,
                    Err(error) => {
                        return AppStartupReport {
                            settings: config_report.settings,
                            diagnostics,
                            result: Err(AppStartupError::CredentialFormat {
                                path,
                                reason: error.kind,
                            }),
                        };
                    }
                }
            }
            Ok(None) => CredentialManifest::empty(),
            Err(reason) => {
                return AppStartupReport {
                    settings: config_report.settings,
                    diagnostics,
                    result: Err(AppStartupError::CredentialRead { path, reason }),
                };
            }
        },
        None => CredentialManifest::empty(),
    };
    let replies = match lookup_all(&manifest, keyring, phase_timeout) {
        Ok(replies) => replies,
        Err(error) => {
            return AppStartupReport {
                settings: config_report.settings,
                diagnostics,
                result: Err(error),
            };
        }
    };
    let credentials = match hydrate(manifest, replies) {
        Ok(credentials) => credentials,
        Err(_) => {
            return AppStartupReport {
                settings: config_report.settings,
                diagnostics,
                result: Err(AppStartupError::Invariant),
            };
        }
    };
    diagnostics.extend(
        credentials
            .warnings()
            .iter()
            .cloned()
            .map(AppStartupDiagnostic::Credential),
    );
    AppStartupReport {
        settings: config_report.settings,
        diagnostics,
        result: Ok(AppStartupConfig {
            config,
            credentials,
            credentials_path,
        }),
    }
}

fn lookup_all(
    manifest: &CredentialManifest,
    keyring: &impl KeyringReader,
    phase_timeout: Duration,
) -> Result<Vec<LookupReply>, AppStartupError> {
    let requests = manifest.lookup_requests();
    let next = AtomicUsize::new(0);
    let deadline = Instant::now() + phase_timeout;
    let results = thread::scope(|scope| {
        let mut handles = Vec::new();
        for _ in 0..requests.len().min(MAX_WORKERS) {
            handles.push(scope.spawn(|| {
                let mut answers = Vec::new();
                loop {
                    if Instant::now() >= deadline {
                        break;
                    }
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(workspace) = requests.get(index) else {
                        break;
                    };
                    answers.push((index, keyring.lookup(workspace)));
                }
                answers
            }));
        }
        handles
            .into_iter()
            .map(|handle| handle.join())
            .collect::<Vec<_>>()
    });
    let mut slots = std::iter::repeat_with(|| None)
        .take(requests.len())
        .collect::<Vec<Option<LookupResult>>>();
    for result in results {
        let answers = result.map_err(|_| AppStartupError::Invariant)?;
        for (index, answer) in answers {
            let slot = slots.get_mut(index).ok_or(AppStartupError::Invariant)?;
            if slot.is_some() {
                return Err(AppStartupError::Invariant);
            }
            *slot = Some(answer);
        }
    }
    Ok(requests
        .into_iter()
        .zip(slots)
        .map(|(workspace, result)| LookupReply {
            workspace: workspace.to_owned(),
            result: result.unwrap_or(LookupResult::Failed(LookupFailureCategory::Other)),
        })
        .collect())
}
