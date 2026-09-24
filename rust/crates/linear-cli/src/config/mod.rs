//! Config-file discovery and selected dotenv loading.
//!
//! This module deliberately stops before TOML parsing and startup wiring.
mod discover;
mod dotenv;
mod runtime;
mod source;

pub use discover::{CandidateTier, ConfigCandidate, ConfigPaths, discover_config_paths};
pub use dotenv::{
    ConfigDiagnostic, ConfigFailure, DiagnosticReason, LoadEnvError, SelectedEnv, load_env,
};
pub use runtime::{ProcessEnvError, ProcessEnvSnapshot, RealGitRootProbe};
pub use source::{
    ConfigInputs, FileKind, FileSource, GitIoStage, GitProbeError, GitProbeResult, GitRootProbe,
    OsFamily, RawConfigFile, ReadCandidate, RealFileSource, read_config_candidate,
};
