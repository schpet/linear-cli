use clap::{Args, Subcommand};

use super::values::NonBlank;

#[derive(Debug, Args)]
#[command(subcommand_negates_reqs = true, args_conflicts_with_subcommands = true)]
pub struct Config {
    #[command(subcommand)]
    pub command: Option<ConfigCommand>,
    /// Default team (key, name, or ID); asked for when omitted
    #[arg(long, value_parser = NonBlank)]
    pub team: Option<String>,
    /// Default issue sort order; asked for when omitted
    #[arg(long)]
    pub sort: Option<crate::config::IssueSort>,
}

/// Offline configuration checks and template preference inspection.
#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Check loaded configuration locally and report ignored settings
    ///
    /// Validates environment, .env, selected global/project config and template
    /// overlays without login, API requests or file changes. Ignored settings
    /// are informational and safe to leave in place. Invalid active settings
    /// or malformed config fail with exit 1; harmless leftovers exit 0.
    Check,
    /// Print effective template preferences as TOML (including empty command hooks)
    List {
        /// Ignore all configuration and overlays; print compiled defaults and empty hooks
        #[arg(long)]
        defaults: bool,
        /// Add TOML comments naming the source of each value
        #[arg(long)]
        show_origin: bool,
    },
    /// Print the raw body of a templates.NAME or template-aliases.NAME preference
    #[command(after_long_help = concat!(
        "Template variables and function signatures: ",
        env!("CARGO_PKG_REPOSITORY"), "/blob/main/docs/templating.md#templates-and-data",
    ))]
    Get {
        /// Preference key, including its templates. or template-aliases. prefix
        #[arg(value_name = "KEY")]
        key: String,
        /// Ignore all configuration and overlays
        #[arg(long)]
        defaults: bool,
    },
}

impl ConfigCommand {
    pub fn defaults(&self) -> bool {
        match self {
            Self::Check => false,
            Self::List { defaults, .. } | Self::Get { defaults, .. } => *defaults,
        }
    }
}
