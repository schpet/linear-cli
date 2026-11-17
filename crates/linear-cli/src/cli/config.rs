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

/// Offline inspection exposes only template preferences, never scalar settings or credentials.
#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
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
            Self::List { defaults, .. } | Self::Get { defaults, .. } => *defaults,
        }
    }
}
