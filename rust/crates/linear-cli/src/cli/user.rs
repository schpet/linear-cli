use clap::{Args, Subcommand};

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct User {
    #[command(subcommand)]
    pub command: UserCommand,
}

#[derive(Debug, Subcommand)]
pub enum UserCommand {
    #[command(name = "list", about = "List members of the workspace")]
    List(UserList),
}

#[derive(Debug, Args)]
pub struct UserList {
    #[arg(long = "all", short = 'a', help = "Include inactive members")]
    pub all: bool,
    #[arg(
        long = "json",
        short = 'j',
        help = "Output as JSON; a member's url mentions them when pasted into Markdown. This searches the whole workspace — prefer `linear team members <TEAM>`, and confirm before mentioning someone outside the team"
    )]
    pub json: bool,
}
