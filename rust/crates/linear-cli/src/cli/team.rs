use clap::builder::NonEmptyStringValueParser;
use clap::{Args, Subcommand};

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct Team {
    #[command(subcommand)]
    pub command: TeamCommand,
}

#[derive(Debug, Subcommand)]
pub enum TeamCommand {
    /// List teams
    List(TeamList),
    /// Create a team
    Create(TeamCreate),
    /// Delete a team
    Delete(TeamDelete),
    /// List a team's members
    Members(TeamMembers),
    /// List a team's workflow states
    States(TeamStates),
    /// Print the configured team key
    Id(TeamId),
    /// Link the configured team's issue IDs in the current GitHub repository
    ///
    /// Adds a GitHub autolink so that references like ENG-123 in commits,
    /// issues and pull requests link to Linear. Needs the `gh` CLI.
    Autolinks(TeamAutolinks),
}

#[derive(Debug, Args)]
pub struct TeamCreate {
    /// Team name
    #[arg(long, short, value_parser = NonEmptyStringValueParser::new())]
    pub name: Option<String>,
    /// Team description
    #[arg(long, short, value_parser = NonEmptyStringValueParser::new())]
    pub description: Option<String>,
    /// Team key, like ENG; derived from the name when omitted
    #[arg(long, short, value_parser = NonEmptyStringValueParser::new())]
    pub key: Option<String>,
    /// Make the team private
    #[arg(long)]
    pub private: bool,
    /// Also prompt for the optional fields
    #[arg(long, short)]
    pub interactive: bool,
}

#[derive(Debug, Args)]
pub struct TeamDelete {
    /// Team key, name, or ID
    #[arg(value_parser = NonEmptyStringValueParser::new())]
    pub team: String,
    /// Move the team's issues to this team (key, name, or ID) first
    #[arg(long, value_name = "TEAM", value_parser = NonEmptyStringValueParser::new())]
    pub move_issues: Option<String>,
    #[command(flatten)]
    pub confirm: super::ConfirmArgs,
}

#[derive(Debug, Args)]
pub struct TeamList {
    /// Open the teams page in the browser
    #[arg(long, short, conflicts_with_all = ["app", "json"])]
    pub web: bool,
    /// Open the teams page in the Linear app
    #[arg(long, short, conflicts_with = "json")]
    pub app: bool,
    /// Maximum number of teams to show (a number or `all`)
    #[arg(long, value_parser = super::limit::parse, default_value = "all")]
    pub limit: super::Limit,
    /// Print JSON
    #[arg(long, short)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct TeamId {}

#[derive(Debug, Args)]
pub struct TeamAutolinks {}

#[derive(Debug, Args)]
pub struct TeamMembers {
    /// Team key, name, or ID; defaults to the configured team
    #[arg(value_parser = NonEmptyStringValueParser::new())]
    pub team: Option<String>,
    /// Include deactivated members
    #[arg(long, short)]
    pub all: bool,
    /// Maximum number of members to show (a number or `all`)
    #[arg(long, value_parser = super::limit::parse, default_value = "all")]
    pub limit: super::Limit,
    /// Print JSON; a member's `url` mentions them when pasted into Markdown
    #[arg(long, short)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct TeamStates {
    /// Team key, name, or ID; defaults to the configured team
    #[arg(value_parser = NonEmptyStringValueParser::new())]
    pub team: Option<String>,
    /// Maximum number of states to show (a number or `all`)
    #[arg(long, value_parser = super::limit::parse, default_value = "all")]
    pub limit: super::Limit,
    /// Print JSON
    #[arg(long, short)]
    pub json: bool,
}
