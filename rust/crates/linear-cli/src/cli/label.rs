use clap::builder::NonEmptyStringValueParser;
use clap::{Args, Subcommand};

use super::values::HexColor;

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct Label {
    #[command(subcommand)]
    pub command: LabelCommand,
}

#[derive(Debug, Subcommand)]
pub enum LabelCommand {
    /// List issue labels
    List(LabelList),
    /// Create an issue label
    Create(LabelCreate),
    /// Delete an issue label
    Delete(LabelDelete),
}

#[derive(Debug, Args)]
pub struct LabelList {
    /// Show this team's labels (key, name, or ID) plus workspace labels
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    pub team: Option<String>,
    /// Show only workspace labels
    #[arg(long, conflicts_with_all = ["team", "all_teams"])]
    pub workspace_only: bool,
    /// Show workspace labels and every team's labels
    #[arg(long, alias = "all", conflicts_with = "team")]
    pub all_teams: bool,
    /// Maximum number of labels to show (a number or `all`)
    #[arg(long, value_parser = super::limit::parse, default_value = "all")]
    pub limit: super::Limit,
    /// Print JSON
    #[arg(long, short)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct LabelCreate {
    /// Label name
    #[arg(long, short, value_parser = NonEmptyStringValueParser::new())]
    pub name: Option<String>,
    /// Color, like #EB5757
    #[arg(long, short)]
    pub color: Option<HexColor>,
    /// Label description
    #[arg(long, short, value_parser = NonEmptyStringValueParser::new())]
    pub description: Option<String>,
    /// Team (key, name, or ID) for a team label; omit for a workspace label
    #[arg(long, short, value_parser = NonEmptyStringValueParser::new())]
    pub team: Option<String>,
    /// Also prompt for the optional fields
    #[arg(long, short)]
    pub interactive: bool,
}

#[derive(Debug, Args)]
pub struct LabelDelete {
    /// Label name or ID
    #[arg(value_name = "LABEL")]
    pub name_or_id: String,
    /// Team (key, name, or ID) whose label to delete, when names repeat
    #[arg(long, short, value_parser = NonEmptyStringValueParser::new())]
    pub team: Option<String>,
    #[command(flatten)]
    pub confirm: super::ConfirmArgs,
}
