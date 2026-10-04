use chrono::NaiveDate;
use clap::builder::NonEmptyStringValueParser;
use clap::{Args, Subcommand, ValueHint};

use super::LINEAR_MARKDOWN;
use super::values::{HexColor, InitiativeStatus, TextSource, UserRef};
use crate::graphql::scalars::Float;

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct Initiative {
    #[command(subcommand)]
    pub command: InitiativeCommand,
}

#[derive(Debug, Subcommand)]
pub enum InitiativeCommand {
    /// List initiatives
    #[command(alias = "ls")]
    List(InitiativeList),
    /// Show an initiative
    #[command(alias = "v")]
    View(InitiativeView),
    /// Create an initiative
    Create(InitiativeCreate),
    /// Update an initiative
    Update(InitiativeUpdate),
    /// Archive an initiative
    Archive(InitiativeArchive),
    /// Restore an archived initiative
    Unarchive(InitiativeUnarchive),
    /// Delete an initiative permanently
    Delete(InitiativeDelete),
    /// Add a project to an initiative
    AddProject(InitiativeAddProject),
    /// Remove a project from an initiative
    RemoveProject(InitiativeRemoveProject),
    /// Add and list comments on an initiative
    Comment(InitiativeComment),
}

#[derive(Debug, Args)]
pub struct InitiativeList {
    /// Show only initiatives with this status [default: active]
    #[arg(long, short, ignore_case = true, conflicts_with = "all_statuses")]
    pub status: Option<InitiativeStatus>,
    /// Show initiatives of every status
    #[arg(long)]
    pub all_statuses: bool,
    /// Show only initiatives owned by this user: a username, email, name, or @me
    #[arg(long, short, value_name = "USER")]
    pub owner: Option<UserRef>,
    /// Open the initiatives page in the browser
    #[arg(long, short)]
    pub web: bool,
    /// Open the initiatives page in the Linear app
    #[arg(long, short)]
    pub app: bool,
    /// Maximum number of initiatives to show (a number or `all`)
    #[arg(long, value_parser = super::limit::parse, default_value = "all")]
    pub limit: super::Limit,
    /// Print JSON
    #[arg(long, short)]
    pub json: bool,
    /// Include archived initiatives
    #[arg(long)]
    pub archived: bool,
}

#[derive(Debug, Args)]
pub struct InitiativeView {
    /// Initiative ID, slug, or name
    #[arg(value_name = "INITIATIVE", value_parser = NonEmptyStringValueParser::new())]
    pub initiative_id: String,
    /// Open the initiative in the browser
    #[arg(long, short)]
    pub web: bool,
    /// Open the initiative in the Linear app
    #[arg(long, short)]
    pub app: bool,
    /// Print JSON
    #[arg(long, short)]
    pub json: bool,
    /// Do not page long output
    #[arg(long)]
    pub no_pager: bool,
}

#[derive(Debug, Args)]
pub struct InitiativeCreate {
    /// Initiative name
    #[arg(long, short, value_parser = NonEmptyStringValueParser::new())]
    pub name: Option<String>,
    /// Initiative description
    #[arg(long, short, value_parser = NonEmptyStringValueParser::new())]
    pub description: Option<String>,
    /// Initiative status [default: planned]
    #[arg(long, short, ignore_case = true)]
    pub status: Option<InitiativeStatus>,
    /// Owner: a username, email, name, or @me
    #[arg(long, short, value_name = "USER")]
    pub owner: Option<UserRef>,
    /// Target date (YYYY-MM-DD)
    #[arg(long, value_name = "DATE", value_parser = super::values::date)]
    pub target_date: Option<NaiveDate>,
    /// Color, like #5E6AD2
    #[arg(long, short)]
    pub color: Option<HexColor>,
    /// Icon name
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    pub icon: Option<String>,
    /// Also prompt for the optional fields
    #[arg(long, short)]
    pub interactive: bool,
    #[command(flatten)]
    pub confirm: super::ConfirmArgs,
}

#[derive(Debug, Args)]
pub struct InitiativeArchive {
    /// Initiative ID, slug, or name
    #[arg(value_name = "INITIATIVE", value_parser = NonEmptyStringValueParser::new(), conflicts_with_all = ["bulk", "bulk_file", "bulk_stdin"])]
    pub initiative_id: Option<String>,
    #[command(flatten)]
    pub confirm: super::ConfirmArgs,
    #[command(flatten)]
    pub bulk: super::BulkArgs,
}

#[derive(Debug, Args)]
pub struct InitiativeUpdate {
    /// Initiative ID, slug, or name
    #[arg(value_name = "INITIATIVE", value_parser = NonEmptyStringValueParser::new())]
    pub initiative_id: String,
    /// New name
    #[arg(long, short, value_parser = NonEmptyStringValueParser::new())]
    pub name: Option<String>,
    /// New description
    #[arg(long, short, value_parser = NonEmptyStringValueParser::new())]
    pub description: Option<String>,
    /// New status
    #[arg(long, ignore_case = true)]
    pub status: Option<InitiativeStatus>,
    /// New owner: a username, email, name, or @me
    #[arg(long, value_name = "USER")]
    pub owner: Option<UserRef>,
    /// New target date (YYYY-MM-DD)
    #[arg(long, value_name = "DATE", value_parser = super::values::date)]
    pub target_date: Option<NaiveDate>,
    /// New color, like #5E6AD2
    #[arg(long)]
    pub color: Option<HexColor>,
    /// New icon name
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    pub icon: Option<String>,
    /// Prompt for the fields to change
    #[arg(long, short)]
    pub interactive: bool,
    #[command(flatten)]
    pub confirm: super::ConfirmArgs,
}

#[derive(Debug, Args)]
pub struct InitiativeUnarchive {
    /// Initiative ID, slug, or name
    #[arg(value_name = "INITIATIVE", value_parser = NonEmptyStringValueParser::new())]
    pub initiative_id: String,
    #[command(flatten)]
    pub confirm: super::ConfirmArgs,
}

#[derive(Debug, Args)]
pub struct InitiativeDelete {
    /// Initiative ID, slug, or name
    #[arg(value_name = "INITIATIVE", value_parser = NonEmptyStringValueParser::new(), conflicts_with_all = ["bulk", "bulk_file", "bulk_stdin"])]
    pub initiative_id: Option<String>,
    #[command(flatten)]
    pub confirm: super::ConfirmArgs,
    #[command(flatten)]
    pub bulk: super::BulkArgs,
}

#[derive(Debug, Args)]
pub struct InitiativeAddProject {
    /// Initiative ID, slug, or name
    #[arg(value_parser = NonEmptyStringValueParser::new())]
    pub initiative: String,
    /// Project ID, slug, or name
    #[arg(value_parser = NonEmptyStringValueParser::new())]
    pub project: String,
    /// Position among the initiative's projects
    #[arg(long, value_name = "NUMBER", value_parser = super::values::sort_order, allow_negative_numbers = true)]
    pub sort_order: Option<Float>,
}

#[derive(Debug, Args)]
pub struct InitiativeRemoveProject {
    /// Initiative ID, slug, or name
    #[arg(value_parser = NonEmptyStringValueParser::new())]
    pub initiative: String,
    /// Project ID, slug, or name
    #[arg(value_parser = NonEmptyStringValueParser::new())]
    pub project: String,
    #[command(flatten)]
    pub confirm: super::ConfirmArgs,
}

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct InitiativeComment {
    #[command(subcommand)]
    pub command: InitiativeCommentCommand,
}

#[derive(Debug, Subcommand)]
pub enum InitiativeCommentCommand {
    /// Comment on an initiative, or reply to a comment
    #[command(after_long_help = LINEAR_MARKDOWN)]
    Add(InitiativeCommentAdd),
    /// List an initiative's comments
    List(InitiativeCommentList),
}

#[derive(Debug, Args)]
pub struct InitiativeCommentAdd {
    /// Initiative ID, slug, or name
    #[arg(value_parser = NonEmptyStringValueParser::new())]
    pub initiative: String,
    /// Comment text, in Markdown
    #[arg(long, short, value_name = "TEXT", value_parser = NonEmptyStringValueParser::new())]
    pub body: Option<String>,
    /// Read the comment from a Markdown file (- for stdin)
    #[arg(long, value_name = "FILE", value_hint = ValueHint::FilePath)]
    pub body_file: Option<TextSource>,
    /// Reply to this top-level comment (by ID)
    #[arg(long, short = 'p', visible_alias = "parent", value_name = "COMMENT", value_parser = NonEmptyStringValueParser::new())]
    pub reply_to: Option<String>,
    #[command(flatten)]
    pub confirm: super::ConfirmArgs,
}

#[derive(Debug, Args)]
pub struct InitiativeCommentList {
    /// Initiative ID, slug, or name
    #[arg(value_parser = NonEmptyStringValueParser::new())]
    pub initiative: String,
    /// Maximum number of comments to show (a number or `all`)
    #[arg(long, value_parser = super::limit::parse, default_value = "all")]
    pub limit: super::Limit,
    /// Print JSON
    #[arg(long, short)]
    pub json: bool,
    /// Do not page long output
    #[arg(long)]
    pub no_pager: bool,
}
