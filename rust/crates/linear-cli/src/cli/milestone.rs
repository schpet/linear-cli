use chrono::NaiveDate;
use clap::builder::NonEmptyStringValueParser;
use clap::{ArgGroup, Args, Subcommand};

use crate::graphql::scalars::Float;

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct Milestone {
    #[command(subcommand)]
    pub command: MilestoneCommand,
}

#[derive(Debug, Subcommand)]
pub enum MilestoneCommand {
    /// List a project's milestones
    List(MilestoneList),
    /// Show a milestone and its issues
    #[command(visible_alias = "v")]
    View(MilestoneView),
    /// Create a project milestone
    Create(MilestoneCreate),
    /// Update a project milestone
    Update(MilestoneUpdate),
    /// Delete a project milestone
    Delete(MilestoneDelete),
}

#[derive(Debug, Args)]
pub struct MilestoneList {
    /// Project ID, slug or name
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    pub project: String,
    /// Maximum number of milestones to show (a number or `all`)
    #[arg(long, value_parser = super::limit::parse, default_value = "all")]
    pub limit: super::Limit,
    /// Print JSON
    #[arg(long, short)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct MilestoneView {
    /// Milestone ID, or its name with --project
    pub milestone: String,
    /// List every issue instead of the first 10
    #[arg(long)]
    pub all: bool,
    /// Project (ID, slug or name) to find the milestone name in
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    pub project: Option<String>,
    /// Print JSON
    #[arg(long, short)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct MilestoneCreate {
    /// Project ID, slug or name
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    pub project: String,
    /// Milestone name
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    pub name: String,
    /// Milestone description
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    pub description: Option<String>,
    /// Target date (YYYY-MM-DD)
    #[arg(long, value_name = "DATE", value_parser = super::values::date)]
    pub target_date: Option<NaiveDate>,
}

#[derive(Debug, Args)]
#[command(group(
    ArgGroup::new("changes")
        .required(true)
        .multiple(true)
        .args(["name", "description", "target_date", "sort_order", "project"])
))]
pub struct MilestoneUpdate {
    /// Milestone ID
    pub id: String,
    /// New name
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    pub name: Option<String>,
    /// New description
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    pub description: Option<String>,
    /// New target date (YYYY-MM-DD)
    #[arg(long, value_name = "DATE", value_parser = super::values::date)]
    pub target_date: Option<NaiveDate>,
    /// Position among the project's milestones
    #[arg(long, value_name = "NUMBER", value_parser = super::values::sort_order, allow_negative_numbers = true)]
    pub sort_order: Option<Float>,
    /// Move the milestone to this project (ID, slug or name)
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    pub project: Option<String>,
}

#[derive(Debug, Args)]
pub struct MilestoneDelete {
    /// Milestone ID
    pub id: String,
    #[command(flatten)]
    pub confirm: super::ConfirmArgs,
}
