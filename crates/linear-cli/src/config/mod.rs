//! Config files, `.env`, environment variables and network settings.
mod discover;
mod dotenv;
mod network;
mod options;
mod parse;
mod runtime;
mod source;
mod startup;

pub use network::NetworkEnv;
pub use options::{
    AssignSelf, ConfigOptions, ConfigSecret, IssueSort, OptionSource, PrTemplateCli, Vcs,
};
pub use parse::{ConfigTier, parse_config_tier};
pub use runtime::ProcessEnvSnapshot;
/// Crate-internal lexical path normalization shared with credential discovery.
pub(crate) use source::lexical as lexical_config_path;
pub use source::{MAX_CONFIG_BYTES, OsFamily, RawConfigFile, RealFileSource, repo_root};
pub use startup::{
    ChildEnvOverlay, DisplaySettings, StartupConfig, load_startup, render_diagnostic,
};

#[cfg(test)]
mod test_support;
#[cfg(test)]
pub(crate) use test_support::fixture_path;

#[cfg(test)]
pub(crate) use dotenv::{ConfigDiagnostic, DiagnosticReason, SelectedEnv};
#[cfg(test)]
pub(crate) use options::OptionInputs;
#[cfg(test)]
pub(crate) use source::{ConfigInputs, FileKind, FileSource};
