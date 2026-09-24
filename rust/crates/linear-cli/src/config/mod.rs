//! Config-file discovery, selected dotenv loading, and owned TOML parsing.
//!
//! This module stops before startup wiring.
mod discover;
mod dotenv;
mod options;
mod parse;
mod runtime;
mod source;

pub use discover::{CandidateTier, ConfigCandidate, ConfigPaths, discover_config_paths};
pub use dotenv::{
    ConfigDiagnostic, ConfigFailure, DiagnosticReason, LoadEnvError, SelectedEnv, load_env,
};
pub use options::{
    AssignSelf, ConfigOptionError, ConfigOptions, ConfigSecret, EndpointSource, IssueSort,
    OptionErrorReason, OptionInputs, OptionKey, OptionSource, PrTemplateCli, PrTemplatePath,
    Resolved, ResolvedEndpoint, Vcs,
};
pub use parse::{
    ConfigParseError, ConfigParseErrorKind, ConfigTier, ConfigValue, parse_config_tier,
};
pub use runtime::{ProcessEnvError, ProcessEnvSnapshot, RealGitRootProbe};
pub use source::{
    ConfigInputs, FileKind, FileSource, GitIoStage, GitProbeError, GitProbeResult, GitRootProbe,
    OsFamily, RawConfigFile, ReadCandidate, RealFileSource, read_config_candidate,
};
