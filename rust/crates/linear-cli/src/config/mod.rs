//! Typed config and credential boundaries for startup and command actions.
mod discover;
mod dotenv;
mod options;
mod parse;
mod runtime;
mod source;
mod startup;
mod transport;

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
/// Crate-internal lexical path normalization shared with credential discovery.
pub(crate) use source::lexical as lexical_config_path;
pub use source::{
    ConfigInputs, FileKind, FileSource, GitIoStage, GitProbeError, GitProbeResult, GitRootProbe,
    OsFamily, RawConfigFile, ReadCandidate, RealFileSource, read_config_candidate,
};
pub use startup::{
    ChildEnvOverlay, DisplaySettings, NoColor, StartupConfig, StartupError, StartupReport,
    load_startup, render_diagnostic,
};
pub use transport::{TransportEnvError, TransportEnvInputs};
