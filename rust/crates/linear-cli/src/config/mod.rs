//! Config-file discovery and selected dotenv loading.
//!
//! This module deliberately stops before TOML parsing and startup wiring.
mod discover;
mod dotenv;
mod source;

pub use discover::{CandidateTier, ConfigCandidate, ConfigPaths, discover_config_paths};
pub use dotenv::{ConfigDiagnostic, ConfigFailure, DiagnosticReason, SelectedEnv, load_env};
pub use source::{
    ConfigInputs, FileKind, FileSource, GitProbeResult, GitRootProbe, OsFamily, RawConfigFile,
    ReadCandidate, RealFileSource, read_config_candidate,
};
