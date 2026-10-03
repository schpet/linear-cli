use clap::builder::NonEmptyStringValueParser;
use clap::{Args, Subcommand, ValueEnum};

use super::LINEAR_MARKDOWN;

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct ProjectUpdate {
    #[command(subcommand)]
    pub command: ProjectUpdateCommand,
}

#[derive(Debug, Subcommand)]
pub enum ProjectUpdateCommand {
    /// Post a status update on a project
    #[command(visible_alias = "c", after_long_help = LINEAR_MARKDOWN)]
    Create(ProjectUpdateCreate),
    /// List a project's status updates
    #[command(visible_alias = "l")]
    List(ProjectUpdateList),
}

#[derive(Debug, Args)]
pub struct ProjectUpdateCreate {
    /// Project ID, slug, or name
    #[arg(value_name = "PROJECT")]
    pub project_id: String,
    #[command(flatten)]
    pub update: StatusUpdateArgs,
}

/// The content of a new project or initiative status update.
#[derive(Debug, Args)]
pub struct StatusUpdateArgs {
    /// Update text, in Markdown
    #[arg(long, value_name = "TEXT", value_parser = NonEmptyStringValueParser::new(), conflicts_with = "body_file")]
    pub body: Option<String>,
    /// Read the update from a Markdown file
    #[arg(long, value_name = "FILE", value_parser = NonEmptyStringValueParser::new())]
    pub body_file: Option<String>,
    /// How the work is going
    #[arg(long)]
    pub health: Option<Health>,
    /// Also prompt for the optional fields
    #[arg(long, short)]
    pub interactive: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum Health {
    #[value(alias = "onTrack")]
    OnTrack,
    #[value(alias = "atRisk")]
    AtRisk,
    #[value(alias = "offTrack")]
    OffTrack,
}

#[derive(Debug, Args)]
pub struct ProjectUpdateList {
    /// Project ID, slug, or name
    #[arg(value_name = "PROJECT")]
    pub project_id: String,
    /// Print JSON
    #[arg(long, short)]
    pub json: bool,
    /// Maximum number of updates to show, newest first (a number or `all`)
    #[arg(long, value_parser = super::limit::parse, default_value = "10")]
    pub limit: super::Limit,
}
