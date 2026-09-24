//! Config-file discovery, selected dotenv loading, and owned TOML parsing.
//!
//! This module stops before startup wiring and typed option selection.
mod discover;
mod dotenv;
mod parse;
mod runtime;
mod source;

pub use discover::{CandidateTier, ConfigCandidate, ConfigPaths, discover_config_paths};
pub use dotenv::{
    ConfigDiagnostic, ConfigFailure, DiagnosticReason, LoadEnvError, SelectedEnv, load_env,
};
pub use parse::{
    ConfigParseError, ConfigParseErrorKind, ConfigTier, ConfigValue, parse_config_tier,
};
pub use runtime::{ProcessEnvError, ProcessEnvSnapshot, RealGitRootProbe};
pub use source::{
    ConfigInputs, FileKind, FileSource, GitIoStage, GitProbeError, GitProbeResult, GitRootProbe,
    OsFamily, RawConfigFile, ReadCandidate, RealFileSource, read_config_candidate,
};
