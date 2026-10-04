use std::path::PathBuf;

use chrono::NaiveDate;
use clap::builder::NonEmptyStringValueParser;
use clap::{Args, Subcommand, ValueEnum, ValueHint};

use super::LINEAR_MARKDOWN;
use super::values::{HexColor, UserRef};
use crate::graphql::operations::project::ProjectStatusType;

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct Project {
    #[command(subcommand)]
    pub command: ProjectCommand,
}

#[derive(Debug, Subcommand)]
pub enum ProjectCommand {
    /// List projects
    List(ProjectList),
    /// Show a project
    #[command(visible_alias = "v")]
    View(ProjectView),
    /// Create a project
    #[command(after_long_help = LINEAR_MARKDOWN)]
    Create(ProjectCreate),
    /// Update a project
    #[command(after_long_help = LINEAR_MARKDOWN)]
    Update(ProjectUpdate),
    /// Delete a project (moves it to the trash)
    Delete(ProjectDelete),
    /// Add and list comments on a project
    Comment(ProjectComment),
}

#[derive(Debug, Args)]
pub struct ProjectList {
    /// Show this team's projects (key, name, or ID); defaults to the configured team
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    pub team: Option<String>,
    /// Show every team's projects
    #[arg(long, conflicts_with = "team")]
    pub all_teams: bool,
    /// Show only projects with this status
    #[arg(long, ignore_case = true)]
    pub status: Option<Status>,
    /// Open the projects page in the browser
    #[arg(long, short)]
    pub web: bool,
    /// Open the projects page in the Linear app
    #[arg(long, short)]
    pub app: bool,
    /// Maximum number of projects to show (a number or `all`)
    #[arg(long, value_parser = super::limit::parse, default_value = "all")]
    pub limit: super::Limit,
    /// Print JSON
    #[arg(long, short)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct ProjectView {
    /// Project ID, slug, or name; asked for when omitted
    #[arg(value_name = "PROJECT")]
    pub project_id: Option<String>,
    /// Open the project in the browser
    #[arg(long, short)]
    pub web: bool,
    /// Open the project in the Linear app
    #[arg(long, short)]
    pub app: bool,
    /// Print JSON
    #[arg(long, short)]
    pub json: bool,
    /// Do not page long output
    #[arg(long)]
    pub no_pager: bool,
}

/// The fields `project create` and `project update` share.
#[derive(Debug, Args)]
pub struct ProjectFields {
    /// Project name
    #[arg(long, short, value_parser = NonEmptyStringValueParser::new())]
    pub name: Option<String>,
    /// Short summary, up to 255 characters
    #[arg(long, short, conflicts_with = "description_file")]
    pub description: Option<String>,
    /// Read the summary from a file
    #[arg(long, short = 'f', value_name = "FILE", value_hint = ValueHint::FilePath)]
    pub description_file: Option<PathBuf>,
    /// Project overview, in Markdown
    #[arg(long, value_name = "MARKDOWN", conflicts_with = "content_file")]
    pub content: Option<String>,
    /// Read the overview from a Markdown file
    #[arg(long, value_name = "FILE", value_hint = ValueHint::FilePath)]
    pub content_file: Option<PathBuf>,
    /// Project status
    #[arg(long, short, ignore_case = true)]
    pub status: Option<Status>,
    /// Project lead: a username, email, name, or @me
    #[arg(long, short, value_name = "USER")]
    pub lead: Option<UserRef>,
    /// Start date (YYYY-MM-DD)
    #[arg(long, value_name = "DATE", value_parser = super::values::date)]
    pub start_date: Option<NaiveDate>,
    /// Target date (YYYY-MM-DD)
    #[arg(long, value_name = "DATE", value_parser = super::values::date)]
    pub target_date: Option<NaiveDate>,
    /// Project priority, by name or number (0 none, 1 urgent to 4 low)
    #[arg(long, ignore_case = true)]
    pub priority: Option<super::values::Priority>,
}

/// A project status, by its kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum Status {
    Backlog,
    Planned,
    #[value(aliases = ["in-progress", "in progress"])]
    Started,
    Paused,
    Completed,
    Canceled,
}

impl From<Status> for ProjectStatusType {
    fn from(status: Status) -> Self {
        match status {
            Status::Backlog => Self::Backlog,
            Status::Planned => Self::Planned,
            Status::Started => Self::Started,
            Status::Paused => Self::Paused,
            Status::Completed => Self::Completed,
            Status::Canceled => Self::Canceled,
        }
    }
}

#[derive(Debug, Args)]
pub struct ProjectCreate {
    #[command(flatten)]
    pub fields: ProjectFields,
    /// Team (key, name, or ID); repeat for several teams
    #[arg(long, short, value_parser = NonEmptyStringValueParser::new())]
    pub team: Vec<String>,
    /// Project label; repeat for several labels
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    pub label: Vec<String>,
    /// Project member: a username, email, name, or @me; repeatable
    #[arg(long, value_name = "USER")]
    pub member: Vec<UserRef>,
    /// Project icon
    #[arg(long)]
    pub icon: Option<String>,
    /// Color, like #5E6AD2
    #[arg(long)]
    pub color: Option<HexColor>,
    /// Add the project to this initiative (ID, slug, or name)
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    pub initiative: Option<String>,
    /// Start from this project template (name or ID)
    ///
    /// Workspace templates and those of the project's teams are searched. The
    /// template fills in anything you do not pass; flags override it.
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    pub template: Option<String>,
    /// Also prompt for the optional fields
    #[arg(long, short)]
    pub interactive: bool,
    /// Print the created project as JSON
    #[arg(long, short)]
    pub json: bool,
    #[command(flatten)]
    pub confirm: super::YesArgs,
}

#[derive(Debug, Args)]
pub struct ProjectUpdate {
    /// Project ID, slug, or name
    #[arg(value_name = "PROJECT")]
    pub project_id: String,
    #[command(flatten)]
    pub fields: ProjectFields,
    /// Remove the project's lead
    #[arg(long, conflicts_with = "lead")]
    pub clear_lead: bool,
    /// Remove the project's start date
    #[arg(long, conflicts_with = "start_date")]
    pub clear_start_date: bool,
    /// Remove the project's target date
    #[arg(long, conflicts_with = "target_date")]
    pub clear_target_date: bool,
    /// Set the project's teams (key, name, or ID), replacing the current ones; repeatable
    #[arg(long, short, value_parser = NonEmptyStringValueParser::new(), conflicts_with_all = ["add_team", "remove_team"])]
    pub team: Vec<String>,
    /// Add a team to the project; repeatable
    #[arg(long, value_name = "TEAM", value_parser = NonEmptyStringValueParser::new())]
    pub add_team: Vec<String>,
    /// Remove a team from the project; repeatable
    #[arg(long, value_name = "TEAM", value_parser = NonEmptyStringValueParser::new())]
    pub remove_team: Vec<String>,
    /// Set the project's labels, replacing the current ones; repeatable
    #[arg(long, value_parser = NonEmptyStringValueParser::new(), conflicts_with_all = ["add_label", "remove_label"])]
    pub label: Vec<String>,
    /// Add a label to the project; repeatable
    #[arg(long, value_name = "LABEL", value_parser = NonEmptyStringValueParser::new())]
    pub add_label: Vec<String>,
    /// Remove a label from the project (the label itself stays); repeatable
    #[arg(long, value_name = "LABEL", value_parser = NonEmptyStringValueParser::new())]
    pub remove_label: Vec<String>,
    /// Set the project's initiatives (ID, slug, or name), replacing the current ones; repeatable
    #[arg(long, value_parser = NonEmptyStringValueParser::new(), conflicts_with_all = ["add_initiative", "remove_initiative"])]
    pub initiative: Vec<String>,
    /// Add the project to an initiative; repeatable
    #[arg(long, value_name = "INITIATIVE", value_parser = NonEmptyStringValueParser::new())]
    pub add_initiative: Vec<String>,
    /// Remove the project from an initiative (the initiative itself stays); repeatable
    #[arg(long, value_name = "INITIATIVE", value_parser = NonEmptyStringValueParser::new())]
    pub remove_initiative: Vec<String>,
}

#[derive(Debug, Args)]
pub struct ProjectDelete {
    /// Project ID, slug, or name
    #[arg(value_name = "PROJECT")]
    pub project_id: String,
    #[command(flatten)]
    pub confirm: super::ConfirmArgs,
}

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct ProjectComment {
    #[command(subcommand)]
    pub command: ProjectCommentCommand,
}

#[derive(Debug, Subcommand)]
pub enum ProjectCommentCommand {
    /// Comment on a project, or reply to a comment
    #[command(after_long_help = LINEAR_MARKDOWN)]
    Add(ProjectCommentAdd),
    /// List a project's comments
    List(ProjectCommentList),
}

#[derive(Debug, Args)]
pub struct ProjectCommentAdd {
    /// Project ID, slug, or name
    pub project: String,
    /// Comment text, in Markdown
    #[arg(long, short, value_name = "TEXT", value_parser = NonEmptyStringValueParser::new())]
    pub body: Option<String>,
    /// Read the comment from a Markdown file
    #[arg(long, value_name = "FILE", value_hint = ValueHint::FilePath)]
    pub body_file: Option<PathBuf>,
    /// Reply to this top-level comment (by ID)
    #[arg(long, short = 'p', visible_alias = "parent", value_name = "COMMENT", value_parser = NonEmptyStringValueParser::new())]
    pub reply_to: Option<String>,
    #[command(flatten)]
    pub confirm: super::ConfirmArgs,
}

#[derive(Debug, Args)]
pub struct ProjectCommentList {
    /// Project ID, slug, or name
    pub project: String,
    /// Maximum number of comments to show (a number or `all`)
    #[arg(long, value_parser = super::limit::parse, default_value = "all")]
    pub limit: super::Limit,
    /// Print JSON
    #[arg(long, short)]
    pub json: bool,
}
