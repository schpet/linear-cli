use clap::Args;

#[derive(Debug, Args)]
pub struct Config {
    #[arg(
        long = "team",
        help = "Default team key, name, or ID (asked for when omitted)",
        value_name = "team",
        value_parser = super::nonempty_string
    )]
    pub team: Option<String>,
    #[arg(
        long = "sort",
        help = "Default issue sort order (asked for when omitted)",
        value_name = "sort"
    )]
    pub sort: Option<crate::config::IssueSort>,
}
