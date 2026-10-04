use std::path::PathBuf;

use clap::builder::NonEmptyStringValueParser;
use clap::{Args, Subcommand, ValueHint};

use super::LINEAR_MARKDOWN;

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct Document {
    #[command(subcommand)]
    pub command: DocumentCommand,
}

#[derive(Debug, Subcommand)]
pub enum DocumentCommand {
    /// List documents
    #[command(visible_alias = "l")]
    List(DocumentList),
    /// Show a document
    #[command(visible_alias = "v")]
    View(DocumentView),
    /// Create a document
    #[command(visible_alias = "c", after_long_help = LINEAR_MARKDOWN)]
    Create(DocumentCreate),
    /// Update a document
    #[command(visible_alias = "u", after_long_help = LINEAR_MARKDOWN)]
    Update(DocumentUpdate),
    /// Delete a document (moves it to the trash)
    #[command(visible_alias = "d")]
    Delete(DocumentDelete),
    /// Add and list comments on a document
    Comment(DocumentComment),
}

#[derive(Debug, Args)]
pub struct DocumentList {
    /// Show this project's documents (ID, slug, or name)
    #[arg(long, conflicts_with_all = ["issue", "initiative", "team", "cycle", "release"], value_parser = NonEmptyStringValueParser::new())]
    pub project: Option<String>,
    /// Show this issue's documents (like ENG-123)
    #[arg(long, conflicts_with_all = ["initiative", "team", "cycle", "release"], value_parser = NonEmptyStringValueParser::new())]
    pub issue: Option<String>,
    /// Show this initiative's documents (ID, slug, or name)
    #[arg(long, conflicts_with_all = ["team", "cycle", "release"], value_parser = NonEmptyStringValueParser::new())]
    pub initiative: Option<String>,
    /// Show this team's documents (key, name, or ID); with --cycle, the cycle's team
    #[arg(long, conflicts_with = "release", value_parser = NonEmptyStringValueParser::new())]
    pub team: Option<String>,
    /// Show this cycle's documents: a name, number, `active`, `next`, `previous`, or an offset like +1 or -1
    #[arg(long, conflicts_with = "release", allow_negative_numbers = true, value_parser = NonEmptyStringValueParser::new())]
    pub cycle: Option<String>,
    /// Show this release's documents (ID, name, or version)
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    pub release: Option<String>,
    /// Print JSON
    #[arg(long, short)]
    pub json: bool,
    /// Maximum number of documents to show (a number or `all`)
    #[arg(long, value_parser = super::limit::parse, default_value = "50")]
    pub limit: super::Limit,
}

#[derive(Debug, Args)]
pub struct DocumentView {
    /// Document ID or slug
    #[arg(value_name = "DOCUMENT", value_parser = NonEmptyStringValueParser::new())]
    pub id: String,
    /// Print the Markdown source instead of rendering it
    #[arg(long)]
    pub raw: bool,
    /// Open the document in the browser
    #[arg(long, short)]
    pub web: bool,
    /// Print JSON
    #[arg(long, short)]
    pub json: bool,
    /// Keep remote image and file URLs instead of downloading them
    #[arg(long)]
    pub no_download: bool,
}

#[derive(Debug, Args)]
pub struct DocumentCreate {
    /// Document title
    #[arg(long, short, value_parser = NonEmptyStringValueParser::new())]
    pub title: Option<String>,
    /// Document text, in Markdown
    #[arg(long, short, value_name = "MARKDOWN", value_parser = NonEmptyStringValueParser::new())]
    pub content: Option<String>,
    /// Read the document from a Markdown file
    #[arg(long, short = 'f', value_name = "FILE", value_hint = ValueHint::FilePath)]
    pub content_file: Option<PathBuf>,
    /// Attach the document to a project (ID, slug, or name)
    #[arg(long, conflicts_with_all = ["issue", "initiative", "team", "cycle", "release"], value_parser = NonEmptyStringValueParser::new())]
    pub project: Option<String>,
    /// Attach the document to an issue (like ENG-123)
    #[arg(long, conflicts_with_all = ["initiative", "team", "cycle", "release"], value_parser = NonEmptyStringValueParser::new())]
    pub issue: Option<String>,
    /// Attach the document to an initiative (ID, slug, or name)
    #[arg(long, conflicts_with_all = ["team", "cycle", "release"], value_parser = NonEmptyStringValueParser::new())]
    pub initiative: Option<String>,
    /// Attach the document to a team (key, name, or ID); with --cycle, the cycle's team
    #[arg(long, conflicts_with = "release", value_parser = NonEmptyStringValueParser::new())]
    pub team: Option<String>,
    /// Attach the document to a cycle: a name, number, `active`, `next`, `previous`, or an offset like +1 or -1
    #[arg(long, conflicts_with = "release", allow_negative_numbers = true, value_parser = NonEmptyStringValueParser::new())]
    pub cycle: Option<String>,
    /// Attach the document to a release (ID, name, or version)
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    pub release: Option<String>,
    /// Document icon (an emoji)
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    pub icon: Option<String>,
    /// Also prompt for the optional fields
    #[arg(long, short)]
    pub interactive: bool,
}

#[derive(Debug, Args)]
pub struct DocumentUpdate {
    /// Document ID or slug
    #[arg(value_name = "DOCUMENT", value_parser = NonEmptyStringValueParser::new())]
    pub document_id: String,
    /// New title
    #[arg(long, short, value_parser = NonEmptyStringValueParser::new())]
    pub title: Option<String>,
    /// New text, in Markdown
    #[arg(long, short, value_name = "MARKDOWN", value_parser = NonEmptyStringValueParser::new())]
    pub content: Option<String>,
    /// Read the new text from a Markdown file
    #[arg(long, short = 'f', value_name = "FILE", value_hint = ValueHint::FilePath)]
    pub content_file: Option<PathBuf>,
    /// New icon (an emoji)
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    pub icon: Option<String>,
    /// Move the document to a project (ID, slug, or name)
    #[arg(long, conflicts_with_all = ["issue", "initiative", "team", "cycle", "release"], value_parser = NonEmptyStringValueParser::new())]
    pub project: Option<String>,
    /// Move the document to an issue (like ENG-123)
    #[arg(long, conflicts_with_all = ["initiative", "team", "cycle", "release"], value_parser = NonEmptyStringValueParser::new())]
    pub issue: Option<String>,
    /// Move the document to an initiative (ID, slug, or name)
    #[arg(long, conflicts_with_all = ["team", "cycle", "release"], value_parser = NonEmptyStringValueParser::new())]
    pub initiative: Option<String>,
    /// Move the document to a team (key, name, or ID); with --cycle, the cycle's team
    #[arg(long, conflicts_with = "release", value_parser = NonEmptyStringValueParser::new())]
    pub team: Option<String>,
    /// Move the document to a cycle: a name, number, `active`, `next`, `previous`, or an offset like +1 or -1
    #[arg(long, conflicts_with = "release", allow_negative_numbers = true, value_parser = NonEmptyStringValueParser::new())]
    pub cycle: Option<String>,
    /// Move the document to a release (ID, name, or version)
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    pub release: Option<String>,
    /// Edit the current text in $EDITOR
    #[arg(long, short)]
    pub edit: bool,
    /// Replace the text even if inline comments may lose their anchors
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct DocumentDelete {
    /// Document ID or slug
    #[arg(value_name = "DOCUMENT", value_parser = NonEmptyStringValueParser::new(), conflicts_with_all = ["bulk", "bulk_file", "bulk_stdin"])]
    pub document_id: Option<String>,
    #[command(flatten)]
    pub confirm: super::ConfirmArgs,
    #[command(flatten)]
    pub bulk: super::BulkArgs,
}

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct DocumentComment {
    #[command(subcommand)]
    pub command: DocumentCommentCommand,
}

#[derive(Debug, Subcommand)]
pub enum DocumentCommentCommand {
    /// Comment on a document, or reply to a comment
    #[command(after_long_help = LINEAR_MARKDOWN)]
    Add(DocumentCommentAdd),
    /// List a document's comments
    List(DocumentCommentList),
}

#[derive(Debug, Args)]
pub struct DocumentCommentAdd {
    /// Document ID or slug
    #[arg(value_parser = NonEmptyStringValueParser::new())]
    pub document: String,
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
pub struct DocumentCommentList {
    /// Document ID or slug
    #[arg(value_parser = NonEmptyStringValueParser::new())]
    pub document: String,
    /// Maximum number of comments to show (a number or `all`)
    #[arg(long, value_parser = super::limit::parse, default_value = "all")]
    pub limit: super::Limit,
    /// Print JSON
    #[arg(long, short)]
    pub json: bool,
}
