use chrono::NaiveDate;
use clap::{ArgGroup, Args, Subcommand};

use crate::graphql::operations::number::Float;

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct Milestone {
    #[command(subcommand)]
    pub command: MilestoneCommand,
}

#[derive(Debug, Subcommand)]
pub enum MilestoneCommand {
    #[command(name = "list", about = "List milestones for a project")]
    List(MilestoneList),
    #[command(name = "view", about = "View milestone details. By default lists the first 10 attached issues from the first page of 50; use --all to paginate the full set.", visible_aliases = ["v"])]
    View(MilestoneView),
    #[command(name = "create", about = "Create a new project milestone")]
    Create(MilestoneCreate),
    #[command(name = "update", about = "Update an existing project milestone")]
    Update(MilestoneUpdate),
    #[command(name = "delete", about = "Delete a project milestone")]
    Delete(MilestoneDelete),
}

#[derive(Debug, Args)]
pub struct MilestoneList {
    #[arg(long = "project", help = "Project (UUID, slug ID, or name)", value_name = "project", value_parser = super::nonempty_string)]
    pub project: String,
    #[arg(long = "json", short = 'j', help = "Output as JSON")]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct MilestoneView {
    #[arg(value_name = "milestone")]
    pub milestone: String,
    #[arg(
        long = "all",
        help = "Fetch and list every issue attached to the milestone (paginates the Linear API)."
    )]
    pub all: bool,
    #[arg(long = "project", help = "Project for resolving a milestone name (UUID, slug ID, or name)", value_name = "project", value_parser = super::nonempty_string)]
    pub project: Option<String>,
    #[arg(long = "json", short = 'j', help = "Output as JSON")]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct MilestoneCreate {
    #[arg(long = "project", help = "Project (UUID, slug ID, or name)", value_name = "project", value_parser = super::nonempty_string)]
    pub project: String,
    #[arg(long = "name", help = "Milestone name", value_name = "name", value_parser = super::nonempty_string)]
    pub name: String,
    #[arg(long = "description", help = "Milestone description", value_name = "description", value_parser = super::nonempty_string)]
    pub description: Option<String>,
    #[arg(long = "target-date", help = "Target date (YYYY-MM-DD)", value_name = "date", value_parser = super::values::date)]
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
    #[arg(value_name = "id")]
    pub id: String,
    #[arg(long = "name", help = "Milestone name", value_name = "name", value_parser = super::nonempty_string)]
    pub name: Option<String>,
    #[arg(long = "description", help = "Milestone description", value_name = "description", value_parser = super::nonempty_string)]
    pub description: Option<String>,
    #[arg(long = "target-date", help = "Target date (YYYY-MM-DD)", value_name = "date", value_parser = super::values::date)]
    pub target_date: Option<NaiveDate>,
    #[arg(long = "sort-order", help = "Sort order relative to other milestones", value_name = "value", value_parser = super::values::sort_order, allow_negative_numbers = true)]
    pub sort_order: Option<Float>,
    #[arg(long = "project", help = "Move to a different project (UUID, slug ID, or name)", value_name = "project", value_parser = super::nonempty_string)]
    pub project: Option<String>,
}

#[derive(Debug, Args)]
pub struct MilestoneDelete {
    #[arg(value_name = "id")]
    pub id: String,
    #[arg(long = "force", short = 'f', help = "Skip confirmation prompt")]
    pub force: bool,
}
