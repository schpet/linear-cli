use clap::{Args, Subcommand};

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct Initiative {
    #[command(subcommand)]
    pub command: InitiativeCommand,
}

#[derive(Debug, Subcommand)]
pub enum InitiativeCommand {
    #[command(name = "list", about = "List initiatives", visible_aliases = ["ls"])]
    List(InitiativeList),
    #[command(name = "view", about = "View initiative details", visible_aliases = ["v"])]
    View(InitiativeView),
    #[command(name = "create", about = "Create a new Linear initiative")]
    Create(InitiativeCreate),
    #[command(name = "archive", about = "Archive a Linear initiative")]
    Archive(InitiativeArchive),
    #[command(name = "update", about = "Update a Linear initiative")]
    Update(InitiativeUpdate),
    #[command(name = "unarchive", about = "Unarchive a Linear initiative")]
    Unarchive(InitiativeUnarchive),
    #[command(name = "delete", about = "Permanently delete a Linear initiative")]
    Delete(InitiativeDelete),
    #[command(name = "add-project", about = "Link a project to an initiative")]
    AddProject(InitiativeAddProject),
    #[command(name = "remove-project", about = "Unlink a project from an initiative")]
    RemoveProject(InitiativeRemoveProject),
    #[command(name = "comment", about = "Manage initiative comments")]
    Comment(InitiativeComment),
}

#[derive(Debug, Args)]
pub struct InitiativeList {
    #[arg(long = "status", short = 's', help = "Filter by status (active, planned, completed)", value_name = "status", value_parser = super::nonempty_string)]
    pub status: Option<String>,
    #[arg(
        long = "all-statuses",
        help = "Show all statuses (default: active only)"
    )]
    pub all_statuses: bool,
    #[arg(long = "owner", short = 'o', help = "Filter by owner (username or email)", value_name = "owner", value_parser = super::nonempty_string)]
    pub owner: Option<String>,
    #[arg(
        long = "web",
        short = 'w',
        help = "Open initiatives page in web browser"
    )]
    pub web: bool,
    #[arg(
        long = "app",
        short = 'a',
        help = "Open initiatives page in Linear.app"
    )]
    pub app: bool,
    #[arg(long = "json", short = 'j', help = "Output as JSON")]
    pub json: bool,
    #[arg(long = "archived", help = "Include archived initiatives")]
    pub archived: bool,
}

#[derive(Debug, Args)]
pub struct InitiativeView {
    #[arg(value_name = "initiativeId", value_parser = super::nonempty_string)]
    pub initiative_id: String,
    #[arg(long = "web", short = 'w', help = "Open in web browser")]
    pub web: bool,
    #[arg(long = "app", short = 'a', help = "Open in Linear.app")]
    pub app: bool,
    #[arg(long = "json", short = 'j', help = "Output as JSON")]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct InitiativeCreate {
    #[arg(long = "name", short = 'n', help = "Initiative name (required)", value_name = "name", value_parser = super::nonempty_string)]
    pub name: Option<String>,
    #[arg(long = "description", short = 'd', help = "Initiative description", value_name = "description", value_parser = super::nonempty_string)]
    pub description: Option<String>,
    #[arg(long = "status", short = 's', help = "Status: planned, active, completed (default: planned)", value_name = "status", value_parser = super::nonempty_string)]
    pub status: Option<String>,
    #[arg(long = "owner", short = 'o', help = "Owner (username, email, or @me for yourself)", value_name = "owner", value_parser = super::nonempty_string)]
    pub owner: Option<String>,
    #[arg(long = "target-date", help = "Target completion date (YYYY-MM-DD)", value_name = "targetDate", value_parser = super::nonempty_string)]
    pub target_date: Option<String>,
    #[arg(long = "color", short = 'c', help = "Color hex code (e.g., #5E6AD2)", value_name = "color", value_parser = super::nonempty_string)]
    pub color: Option<String>,
    #[arg(long = "icon", help = "Icon name", value_name = "icon", value_parser = super::nonempty_string)]
    pub icon: Option<String>,
    #[arg(
        long = "interactive",
        short = 'i',
        help = "Interactive mode (default if no flags provided)"
    )]
    pub interactive: bool,
}

#[derive(Debug, Args)]
pub struct InitiativeArchive {
    #[arg(value_name = "initiativeId", value_parser = super::nonempty_string, conflicts_with_all = ["bulk", "bulk_file", "bulk_stdin"])]
    pub initiative_id: Option<String>,
    #[arg(long = "force", short = 'y', help = "Skip confirmation prompt")]
    pub force: bool,
    #[arg(long = "bulk", help = "Archive multiple initiatives by ID, slug, or name", value_name = "ids", value_parser = super::nonempty_string, num_args = 0..)]
    pub bulk: Option<Vec<String>>,
    #[arg(long = "bulk-file", help = "Read initiative IDs from a file (one per line)", value_name = "file", value_parser = super::nonempty_string)]
    pub bulk_file: Option<String>,
    #[arg(long = "bulk-stdin", help = "Read initiative IDs from stdin")]
    pub bulk_stdin: bool,
}

#[derive(Debug, Args)]
pub struct InitiativeUpdate {
    #[arg(value_name = "initiativeId", value_parser = super::nonempty_string)]
    pub initiative_id: String,
    #[arg(long = "name", short = 'n', help = "New name for the initiative", value_name = "name", value_parser = super::nonempty_string)]
    pub name: Option<String>,
    #[arg(long = "description", short = 'd', help = "New description", value_name = "description", value_parser = super::nonempty_string)]
    pub description: Option<String>,
    #[arg(long = "status", help = "New status (planned, active, completed, paused)", value_name = "status", value_parser = super::nonempty_string)]
    pub status: Option<String>,
    #[arg(long = "owner", help = "New owner (username, email, or @me)", value_name = "owner", value_parser = super::nonempty_string)]
    pub owner: Option<String>,
    #[arg(long = "target-date", help = "Target completion date (YYYY-MM-DD)", value_name = "targetDate", value_parser = super::nonempty_string)]
    pub target_date: Option<String>,
    #[arg(long = "color", help = "Initiative color (hex, e.g., #5E6AD2)", value_name = "color", value_parser = super::nonempty_string)]
    pub color: Option<String>,
    #[arg(long = "icon", help = "Initiative icon name", value_name = "icon", value_parser = super::nonempty_string)]
    pub icon: Option<String>,
    #[arg(
        long = "interactive",
        short = 'i',
        help = "Interactive mode for updates"
    )]
    pub interactive: bool,
}

#[derive(Debug, Args)]
pub struct InitiativeUnarchive {
    #[arg(value_name = "initiativeId", value_parser = super::nonempty_string)]
    pub initiative_id: String,
    #[arg(long = "force", short = 'y', help = "Skip confirmation prompt")]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct InitiativeDelete {
    #[arg(value_name = "initiativeId", value_parser = super::nonempty_string, conflicts_with_all = ["bulk", "bulk_file", "bulk_stdin"])]
    pub initiative_id: Option<String>,
    #[arg(long = "force", short = 'y', help = "Skip confirmation prompt")]
    pub force: bool,
    #[arg(long = "bulk", help = "Delete multiple initiatives by ID, slug, or name", value_name = "ids", value_parser = super::nonempty_string, num_args = 0..)]
    pub bulk: Option<Vec<String>>,
    #[arg(long = "bulk-file", help = "Read initiative IDs from a file (one per line)", value_name = "file", value_parser = super::nonempty_string)]
    pub bulk_file: Option<String>,
    #[arg(long = "bulk-stdin", help = "Read initiative IDs from stdin")]
    pub bulk_stdin: bool,
}

#[derive(Debug, Args)]
pub struct InitiativeAddProject {
    #[arg(value_name = "initiative", value_parser = super::nonempty_string)]
    pub initiative: String,
    #[arg(value_name = "project", value_parser = super::nonempty_string)]
    pub project: String,
    #[arg(long = "sort-order", help = "Sort order within initiative", value_name = "sortOrder", value_parser = super::numeric::finite_decimal, allow_negative_numbers = true)]
    pub sort_order: Option<f64>,
}

#[derive(Debug, Args)]
pub struct InitiativeRemoveProject {
    #[arg(value_name = "initiative", value_parser = super::nonempty_string)]
    pub initiative: String,
    #[arg(value_name = "project", value_parser = super::nonempty_string)]
    pub project: String,
    #[arg(long = "force", short = 'y', help = "Skip confirmation prompt")]
    pub force: bool,
}

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct InitiativeComment {
    #[command(subcommand)]
    pub command: InitiativeCommentCommand,
}

#[derive(Debug, Subcommand)]
pub enum InitiativeCommentCommand {
    #[command(
        name = "add",
        about = "Add a comment or reply to an initiative's discussion (by ID, slug, or name)",
        long_about = "Add a comment or reply to an initiative's discussion (by ID, slug, or name)\n\nLinear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,\nand `[Name](url)` do not. Get a person's URL from the `url` field of\n`linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.\nRun `linear markdown` for collapsible sections and the full reference."
    )]
    Add(InitiativeCommentAdd),
    #[command(
        name = "list",
        about = "List comments on an initiative (by ID, slug, or name)"
    )]
    List(InitiativeCommentList),
}

#[derive(Debug, Args)]
pub struct InitiativeCommentAdd {
    #[arg(value_name = "initiative", value_parser = super::nonempty_string)]
    pub initiative: String,
    #[arg(long = "body", short = 'b', help = "Comment body text", value_name = "text", value_parser = super::nonempty_string)]
    pub body: Option<String>,
    #[arg(long = "body-file", help = "Read comment body from a file (preferred for markdown content)", value_name = "path", value_parser = super::nonempty_string)]
    pub body_file: Option<String>,
    #[arg(long = "parent", short = 'p', visible_aliases = ["reply-to"], help = "Reply to a top-level comment by ID (the reply joins that thread)", value_name = "commentId", value_parser = super::nonempty_string)]
    pub parent: Option<String>,
}

#[derive(Debug, Args)]
pub struct InitiativeCommentList {
    #[arg(value_name = "initiative", value_parser = super::nonempty_string)]
    pub initiative: String,
    #[arg(long = "json", short = 'j', help = "Output as JSON")]
    pub json: bool,
}
