use clap::{Args, Subcommand};

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct Label {
    #[command(subcommand)]
    pub command: LabelCommand,
}

#[derive(Debug, Subcommand)]
pub enum LabelCommand {
    #[command(name = "list", about = "List issue labels")]
    List(LabelList),
    #[command(name = "create", about = "Create a new issue label")]
    Create(LabelCreate),
    #[command(name = "delete", about = "Delete an issue label")]
    Delete(LabelDelete),
}

#[derive(Debug, Args)]
pub struct LabelList {
    #[arg(long = "team", help = "Filter by team key, name, or ID (e.g., TC). Shows that team's labels plus workspace labels.", value_name = "team", value_parser = super::nonempty_string)]
    pub team: Option<String>,
    #[arg(
        long = "workspace-only",
        help = "Show only workspace-level labels (not team-specific)",
        conflicts_with_all = ["team", "all"]
    )]
    pub workspace_only: bool,
    #[arg(
        long = "all",
        help = "Show all labels (both workspace and team)",
        conflicts_with = "team"
    )]
    pub all: bool,
    #[arg(long = "limit", help = "Maximum number of labels to show (a number or `all`)", value_name = "limit", value_parser = super::limit::parse, default_value = "all")]
    pub limit: super::Limit,
    #[arg(long = "json", short = 'j', help = "Output as JSON")]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct LabelCreate {
    #[arg(long = "name", short = 'n', help = "Label name (required)", value_name = "name", value_parser = super::nonempty_string)]
    pub name: Option<String>,
    #[arg(long = "color", short = 'c', help = "Color hex code (e.g., #EB5757)", value_name = "color", value_parser = super::values::hex_color)]
    pub color: Option<String>,
    #[arg(long = "description", short = 'd', help = "Label description", value_name = "description", value_parser = super::nonempty_string)]
    pub description: Option<String>,
    #[arg(long = "team", short = 't', help = "Team key, name, or ID for a team-specific label (omit for workspace label)", value_name = "team", value_parser = super::nonempty_string)]
    pub team: Option<String>,
    #[arg(
        long = "interactive",
        short = 'i',
        help = "Interactive mode (default if no flags provided)"
    )]
    pub interactive: bool,
}

#[derive(Debug, Args)]
pub struct LabelDelete {
    #[arg(value_name = "nameOrId")]
    pub name_or_id: String,
    #[arg(long = "team", short = 't', help = "Team key, name, or ID to disambiguate labels with the same name", value_name = "team", value_parser = super::nonempty_string)]
    pub team: Option<String>,
    #[arg(long = "force", short = 'f', help = "Skip confirmation prompt")]
    pub force: bool,
}
