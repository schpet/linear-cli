use clap::Args;

use super::values::NonBlank;

#[derive(Debug, Args)]
pub struct Config {
    /// Default team (key, name, or ID); asked for when omitted
    #[arg(long, value_parser = NonBlank)]
    pub team: Option<String>,
    /// Default issue sort order; asked for when omitted
    #[arg(long)]
    pub sort: Option<crate::config::IssueSort>,
}
