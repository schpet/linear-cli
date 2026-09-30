use clap::{Args, Subcommand};

#[derive(Debug, Args)]
pub struct ProjectUpdate {
    #[command(subcommand)]
    pub command: Option<ProjectUpdateCommand>,
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
    #[arg(long = "body", help = "Update content (inline)", value_name = "body", value_parser = super::nonempty_string)]
    pub body: Option<String>,
    #[arg(long = "body-file", help = "Read content from file", value_name = "path", value_parser = super::nonempty_string)]
    pub body_file: Option<String>,
    #[arg(long = "health", help = "Project health status (onTrack, atRisk, offTrack)", value_name = "health", value_parser = super::nonempty_string)]
    pub health: Option<String>,
    #[arg(
        long = "interactive",
        short = 'i',
        help = "Interactive mode with prompts"
    )]
    pub interactive: bool,
}

#[derive(Debug, Args)]
pub struct ProjectUpdateList {
    #[arg(value_name = "projectId")]
    pub project_id: String,
    #[arg(long = "json", help = "Output as JSON")]
    pub json: bool,
    #[arg(long = "limit", help = "Limit results", value_name = "limit", value_parser = super::numeric::positive_u32, default_value = "10")]
    pub limit: std::num::NonZeroU32,
}
