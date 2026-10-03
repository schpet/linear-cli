use clap::{Args, Subcommand};

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct User {
    #[command(subcommand)]
    pub command: UserCommand,
}

#[derive(Debug, Subcommand)]
pub enum UserCommand {
    /// List the workspace's members
    List(UserList),
}

#[derive(Debug, Args)]
pub struct UserList {
    /// Include deactivated members
    #[arg(long, short)]
    pub all: bool,
    /// Maximum number of members to show (a number or `all`)
    #[arg(long, value_parser = super::limit::parse, default_value = "all")]
    pub limit: super::Limit,
    /// Print JSON; a member's `url` mentions them when pasted into Markdown
    ///
    /// This lists the whole workspace. To find someone to mention, prefer
    /// `linear team members <TEAM>`, and confirm before mentioning someone
    /// outside the team.
    #[arg(long, short)]
    pub json: bool,
}
