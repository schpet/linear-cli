use clap::{Args, Subcommand};

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct Team {
    #[command(subcommand)]
    pub command: TeamCommand,
}

#[derive(Debug, Subcommand)]
pub enum TeamCommand {
    #[command(name = "create", about = "Create a linear team")]
    Create(TeamCreate),
    #[command(name = "delete", about = "Delete a Linear team")]
    Delete(TeamDelete),
    #[command(name = "list", about = "List teams")]
    List(TeamList),
    #[command(name = "id", about = "Print the configured team id")]
    Id(TeamId),
    #[command(
        name = "autolinks",
        about = "Configure GitHub repository autolinks for Linear issues with this team prefix"
    )]
    Autolinks(TeamAutolinks),
    #[command(
        name = "members",
        about = "List team members (team by key, name, or ID)"
    )]
    Members(TeamMembers),
    #[command(
        name = "states",
        about = "List workflow states for a team (by key, name, or ID)"
    )]
    States(TeamStates),
}

#[derive(Debug, Args)]
pub struct TeamCreate {
    #[arg(long = "name", short = 'n', help = "Name of the team", value_name = "name", value_parser = super::nonempty_string)]
    pub name: Option<String>,
    #[arg(long = "description", short = 'd', help = "Description of the team", value_name = "description", value_parser = super::nonempty_string)]
    pub description: Option<String>,
    #[arg(long = "key", short = 'k', help = "Team key (if not provided, will be generated from name)", value_name = "key", value_parser = super::nonempty_string)]
    pub key: Option<String>,
    #[arg(long = "private", help = "Make the team private")]
    pub private: bool,
    #[arg(long = "no-interactive", help = "Disable interactive prompts")]
    pub no_interactive: bool,
}

#[derive(Debug, Args)]
pub struct TeamDelete {
    #[arg(value_name = "team")]
    pub team: String,
    #[arg(long = "move-issues", help = "Move all issues to another team (key, name, or ID) before deletion", value_name = "targetTeam", value_parser = super::nonempty_string)]
    pub move_issues: Option<String>,
    #[arg(long = "force", short = 'y', help = "Skip confirmation prompt")]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct TeamList {
    #[arg(long = "web", short = 'w', help = "Open in web browser")]
    pub web: bool,
    #[arg(long = "app", short = 'a', help = "Open in Linear.app")]
    pub app: bool,
    #[arg(long = "json", short = 'j', help = "Output as JSON")]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct TeamId {}

#[derive(Debug, Args)]
pub struct TeamAutolinks {}

#[derive(Debug, Args)]
pub struct TeamMembers {
    #[arg(value_name = "team")]
    pub team: Option<String>,
    #[arg(long = "all", short = 'a', help = "Include inactive members")]
    pub all: bool,
    #[arg(
        long = "json",
        short = 'j',
        help = "Output as JSON; a member's url mentions them when pasted into Markdown"
    )]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct TeamStates {
    #[arg(value_name = "team")]
    pub team: Option<String>,
    #[arg(long = "json", short = 'j', help = "Output as JSON")]
    pub json: bool,
}
