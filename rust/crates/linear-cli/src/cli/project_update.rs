use clap::{Args, Subcommand, ValueEnum};

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct ProjectUpdate {
    #[command(subcommand)]
    pub command: ProjectUpdateCommand,
}

#[derive(Debug, Subcommand)]
pub enum ProjectUpdateCommand {
    #[command(name = "create", about = "Create a new status update for a project", long_about = "Create a new status update for a project\n\nLinear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,\nand `[Name](url)` do not. Get a person's URL from the `url` field of\n`linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.\nRun `linear markdown` for collapsible sections and the full reference.", visible_aliases = ["c"])]
    Create(ProjectUpdateCreate),
    #[command(name = "list", about = "List status updates for a project", visible_aliases = ["l"])]
    List(ProjectUpdateList),
}

#[derive(Debug, Args)]
pub struct ProjectUpdateCreate {
    #[arg(value_name = "projectId")]
    pub project_id: String,
    #[command(flatten)]
    pub update: StatusUpdateArgs,
}

/// The content of a new project or initiative status update.
#[derive(Debug, Args)]
pub struct StatusUpdateArgs {
    #[arg(long = "body", help = "Update content (markdown)", value_name = "body", value_parser = super::nonempty_string, conflicts_with = "body_file")]
    pub body: Option<String>,
    #[arg(long = "body-file", help = "Read content from file", value_name = "path", value_parser = super::nonempty_string)]
    pub body_file: Option<String>,
    #[arg(long = "health", help = "Health status", value_name = "health")]
    pub health: Option<Health>,
    #[arg(
        long = "interactive",
        short = 'i',
        help = "Interactive mode with prompts"
    )]
    pub interactive: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum Health {
    #[value(name = "onTrack")]
    OnTrack,
    #[value(name = "atRisk")]
    AtRisk,
    #[value(name = "offTrack")]
    OffTrack,
}

#[derive(Debug, Args)]
pub struct ProjectUpdateList {
    #[arg(value_name = "projectId")]
    pub project_id: String,
    #[arg(long = "json", help = "Output as JSON")]
    pub json: bool,
    #[arg(long = "limit", help = "Limit results", value_name = "limit", value_parser = clap::value_parser!(i32).range(1..), default_value = "10")]
    pub limit: i32,
}
