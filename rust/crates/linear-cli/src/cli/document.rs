use clap::{Args, Subcommand};

#[derive(Debug, Args)]
pub struct Document {
    #[command(subcommand)]
    pub command: Option<DocumentCommand>,
}

#[derive(Debug, Subcommand)]
pub enum DocumentCommand {
    #[command(name = "list", about = "List documents", visible_aliases = ["l"])]
    List(DocumentList),
    #[command(name = "view", about = "View a document's content", visible_aliases = ["v"])]
    View(DocumentView),
    #[command(name = "create", about = "Create a new document", long_about = "Create a new document\n\nLinear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,\nand `[Name](url)` do not. Get a person's URL from the `url` field of\n`linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.\nRun `linear markdown` for collapsible sections and the full reference.", visible_aliases = ["c"])]
    Create(DocumentCreate),
    #[command(name = "update", about = "Update an existing document", long_about = "Update an existing document\n\nLinear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,\nand `[Name](url)` do not. Get a person's URL from the `url` field of\n`linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.\nRun `linear markdown` for collapsible sections and the full reference.", visible_aliases = ["u"])]
    Update(DocumentUpdate),
    #[command(name = "delete", about = "Delete a document (moves to trash)", visible_aliases = ["d"])]
    Delete(DocumentDelete),
    #[command(name = "comment", about = "Manage document comments")]
    Comment(DocumentComment),
}

#[derive(Debug, Args)]
pub struct DocumentList {
    #[arg(long = "project", help = "Filter by project (UUID, slug ID, or name)", value_name = "project", value_parser = super::nonempty_string)]
    pub project: Option<String>,
    #[arg(long = "issue", help = "Filter by issue (identifier like TC-123)", value_name = "issue", value_parser = super::nonempty_string)]
    pub issue: Option<String>,
    #[arg(long = "initiative", help = "Filter by initiative (UUID, slug ID, or name)", value_name = "initiative", value_parser = super::nonempty_string)]
    pub initiative: Option<String>,
    #[arg(long = "team", help = "Filter by team (key, name, or ID); with --cycle, scopes the cycle lookup instead", value_name = "team", value_parser = super::nonempty_string)]
    pub team: Option<String>,
    #[arg(long = "cycle", help = "Filter by cycle: name, number, 'active'/'now', 'next', 'previous', or a relative offset like +1 (team from --team or config)", value_name = "cycle", value_parser = super::nonempty_string)]
    pub cycle: Option<String>,
    #[arg(long = "release", help = "Filter by release (UUID, name, or version)", value_name = "release", value_parser = super::nonempty_string)]
    pub release: Option<String>,
    #[arg(long = "json", help = "Output as JSON")]
    pub json: bool,
    #[arg(long = "limit", help = "Limit results", value_name = "limit", value_parser = super::numeric::positive_u32, default_value = "50")]
    pub limit: std::num::NonZeroU32,
}

#[derive(Debug, Args)]
pub struct DocumentView {
    #[arg(value_name = "id")]
    pub id: String,
    #[arg(long = "raw", help = "Output raw markdown without rendering")]
    pub raw: bool,
    #[arg(long = "web", short = 'w', help = "Open document in browser")]
    pub web: bool,
    #[arg(long = "json", help = "Output full document as JSON")]
    pub json: bool,
    #[arg(
        long = "no-download",
        help = "Keep remote URLs instead of downloading files"
    )]
    pub no_download: bool,
}

#[derive(Debug, Args)]
pub struct DocumentCreate {
    #[arg(long = "title", short = 't', help = "Document title (required)", value_name = "title", value_parser = super::nonempty_string)]
    pub title: Option<String>,
    #[arg(long = "content", short = 'c', help = "Markdown content (inline)", value_name = "content", value_parser = super::nonempty_string)]
    pub content: Option<String>,
    #[arg(long = "content-file", short = 'f', help = "Read content from file", value_name = "path", value_parser = super::nonempty_string)]
    pub content_file: Option<String>,
    #[arg(long = "project", help = "Attach to project (UUID, slug ID, or name)", value_name = "project", value_parser = super::nonempty_string)]
    pub project: Option<String>,
    #[arg(long = "issue", help = "Attach to issue (identifier like TC-123)", value_name = "issue", value_parser = super::nonempty_string)]
    pub issue: Option<String>,
    #[arg(long = "initiative", help = "Attach to initiative (UUID, slug ID, or name)", value_name = "initiative", value_parser = super::nonempty_string)]
    pub initiative: Option<String>,
    #[arg(long = "team", help = "Attach to team (key, name, or ID); with --cycle, scopes the cycle lookup instead", value_name = "team", value_parser = super::nonempty_string)]
    pub team: Option<String>,
    #[arg(long = "cycle", help = "Attach to cycle: name, number, 'active'/'now', 'next', 'previous', or a relative offset like +1 (team from --team or config)", value_name = "cycle", value_parser = super::nonempty_string)]
    pub cycle: Option<String>,
    #[arg(long = "release", help = "Attach to release (UUID, name, or version)", value_name = "release", value_parser = super::nonempty_string)]
    pub release: Option<String>,
    #[arg(long = "icon", help = "Document icon (emoji)", value_name = "icon", value_parser = super::nonempty_string)]
    pub icon: Option<String>,
    #[arg(
        long = "interactive",
        short = 'i',
        help = "Interactive mode with prompts"
    )]
    pub interactive: bool,
}

#[derive(Debug, Args)]
pub struct DocumentUpdate {
    #[arg(value_name = "documentId")]
    pub document_id: String,
    #[arg(long = "title", short = 't', help = "New title for the document", value_name = "title", value_parser = super::nonempty_string)]
    pub title: Option<String>,
    #[arg(long = "content", short = 'c', help = "New markdown content (inline)", value_name = "content", value_parser = super::nonempty_string)]
    pub content: Option<String>,
    #[arg(long = "content-file", short = 'f', help = "Read new content from file", value_name = "path", value_parser = super::nonempty_string)]
    pub content_file: Option<String>,
    #[arg(long = "icon", help = "New icon (emoji)", value_name = "icon", value_parser = super::nonempty_string)]
    pub icon: Option<String>,
    #[arg(long = "project", help = "Re-point to project (UUID, slug ID, or name); replaces the current attachment", value_name = "project", value_parser = super::nonempty_string)]
    pub project: Option<String>,
    #[arg(long = "issue", help = "Re-point to issue (identifier like TC-123); replaces the current attachment", value_name = "issue", value_parser = super::nonempty_string)]
    pub issue: Option<String>,
    #[arg(long = "initiative", help = "Re-point to initiative (UUID, slug ID, or name); replaces the current attachment", value_name = "initiative", value_parser = super::nonempty_string)]
    pub initiative: Option<String>,
    #[arg(long = "team", help = "Re-point to team (key, name, or ID); with --cycle, scopes the cycle lookup instead", value_name = "team", value_parser = super::nonempty_string)]
    pub team: Option<String>,
    #[arg(long = "cycle", help = "Re-point to cycle: name, number, 'active'/'now', 'next', 'previous', or a relative offset like +1 (team from --team or config)", value_name = "cycle", value_parser = super::nonempty_string)]
    pub cycle: Option<String>,
    #[arg(long = "release", help = "Re-point to release (UUID, name, or version); replaces the current attachment", value_name = "release", value_parser = super::nonempty_string)]
    pub release: Option<String>,
    #[arg(
        long = "edit",
        short = 'e',
        help = "Open current content in $EDITOR for editing"
    )]
    pub edit: bool,
    #[arg(
        long = "force",
        help = "Update content even when document comments may lose inline anchors"
    )]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct DocumentDelete {
    #[arg(value_name = "documentId")]
    pub document_id: Option<String>,
    #[arg(long = "yes", short = 'y', help = "Skip confirmation prompt")]
    pub yes: bool,
    #[arg(long = "bulk", help = "Delete multiple documents by slug or ID", value_name = "ids", value_parser = super::nonempty_string, num_args = 0..)]
    pub bulk: Option<Vec<String>>,
    #[arg(long = "bulk-file", help = "Read document slugs/IDs from a file (one per line)", value_name = "file", value_parser = super::nonempty_string)]
    pub bulk_file: Option<String>,
    #[arg(long = "bulk-stdin", help = "Read document slugs/IDs from stdin")]
    pub bulk_stdin: bool,
}

#[derive(Debug, Args)]
pub struct DocumentComment {
    #[command(subcommand)]
    pub command: Option<DocumentCommentCommand>,
}

#[derive(Debug, Subcommand)]
pub enum DocumentCommentCommand {
    #[command(
        name = "add",
        about = "Add a comment or reply to a document (by ID or slug)",
        long_about = "Add a comment or reply to a document (by ID or slug)\n\nLinear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,\nand `[Name](url)` do not. Get a person's URL from the `url` field of\n`linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.\nRun `linear markdown` for collapsible sections and the full reference."
    )]
    Add(DocumentCommentAdd),
    #[command(name = "list", about = "List comments on a document (by ID or slug)")]
    List(DocumentCommentList),
}

#[derive(Debug, Args)]
pub struct DocumentCommentAdd {
    #[arg(value_name = "document")]
    pub document: String,
    #[arg(long = "body", short = 'b', help = "Comment body text", value_name = "text", value_parser = super::nonempty_string)]
    pub body: Option<String>,
    #[arg(long = "body-file", help = "Read comment body from a file (preferred for markdown content)", value_name = "path", value_parser = super::nonempty_string)]
    pub body_file: Option<String>,
    #[arg(long = "parent", short = 'p', visible_aliases = ["reply-to"], help = "Reply to a top-level comment by ID (the reply joins that thread)", value_name = "commentId", value_parser = super::nonempty_string)]
    pub parent: Option<String>,
}

#[derive(Debug, Args)]
pub struct DocumentCommentList {
    #[arg(value_name = "document")]
    pub document: String,
    #[arg(long = "json", short = 'j', help = "Output as JSON")]
    pub json: bool,
}
