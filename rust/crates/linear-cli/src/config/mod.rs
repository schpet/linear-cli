//! Config files, `.env`, environment variables and transport settings.
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
pub use parse::{ConfigParseError, ConfigParseErrorKind, ConfigTier, parse_config_tier};
pub use runtime::{ProcessEnvError, ProcessEnvSnapshot};
/// Crate-internal lexical path normalization shared with credential discovery.
pub(crate) use source::lexical as lexical_config_path;
pub use source::{
    ConfigInputs, FileKind, FileSource, MAX_CONFIG_BYTES, OsFamily, RawConfigFile, ReadCandidate,
    RealFileSource, read_config_candidate, repo_root,
};
pub use startup::{
    ChildEnvOverlay, DisplaySettings, StartupConfig, StartupReport, load_startup, render_diagnostic,
};
pub use transport::TransportEnvInputs;

#[cfg(test)]
mod test_support;
