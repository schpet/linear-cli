use clap::{Args, Subcommand};

#[derive(Debug, Args)]
pub struct Project {
    #[command(subcommand)]
    pub command: Option<ProjectCommand>,
}

#[derive(Debug, Subcommand)]
pub enum ProjectCommand {
    #[command(name = "list", about = "List projects")]
    List(ProjectList),
    #[command(name = "view", about = "View project details", visible_aliases = ["v"])]
    View(ProjectView),
    #[command(
        name = "create",
        about = "Create a new Linear project",
        long_about = "Create a new Linear project\n\nLinear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,\nand `[Name](url)` do not. Get a person's URL from the `url` field of\n`linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.\nRun `linear markdown` for collapsible sections and the full reference."
    )]
    Create(ProjectCreate),
    #[command(
        name = "update",
        about = "Update a Linear project",
        long_about = "Update a Linear project\n\nLinear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,\nand `[Name](url)` do not. Get a person's URL from the `url` field of\n`linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.\nRun `linear markdown` for collapsible sections and the full reference."
    )]
    Update(ProjectUpdate),
    #[command(name = "delete", about = "Delete (trash) a Linear project")]
    Delete(ProjectDelete),
    #[command(name = "comment", about = "Manage project comments")]
    Comment(ProjectComment),
}

#[derive(Debug, Args)]
pub struct ProjectList {
    #[arg(long = "team", help = "Filter by team key, name, or ID", value_name = "team", value_parser = super::nonempty_string)]
    pub team: Option<String>,
    #[arg(long = "all-teams", help = "Show projects from all teams")]
    pub all_teams: bool,
    #[arg(long = "status", help = "Filter by status name", value_name = "status", value_parser = super::nonempty_string)]
    pub status: Option<String>,
    #[arg(long = "web", short = 'w', help = "Open in web browser")]
    pub web: bool,
    #[arg(long = "app", short = 'a', help = "Open in Linear.app")]
    pub app: bool,
    #[arg(long = "json", short = 'j', help = "Output as JSON")]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct ProjectView {
    #[arg(value_name = "projectId")]
    pub project_id: Option<String>,
    #[arg(long = "web", short = 'w', help = "Open in web browser")]
    pub web: bool,
    #[arg(long = "app", short = 'a', help = "Open in Linear.app")]
    pub app: bool,
    #[arg(long = "json", short = 'j', help = "Output as JSON")]
    pub json: bool,
    #[arg(long = "no-pager", help = "Disable automatic paging for long output")]
    pub no_pager: bool,
}

#[derive(Debug, Args)]
pub struct ProjectCreate {
    #[arg(long = "name", short = 'n', help = "Project name (required)", value_name = "name", value_parser = super::nonempty_string)]
    pub name: Option<String>,
    #[arg(
        long = "description",
        short = 'd',
        help = "Project description (max 255 characters, enforced by Linear's API)",
        value_name = "description"
    )]
    pub description: Option<String>,
    #[arg(long = "description-file", short = 'f', help = "Read project description from file (still subject to the 255-character API limit)", value_name = "path", value_parser = super::nonempty_string)]
    pub description_file: Option<String>,
    #[arg(
        long = "content",
        help = "Project overview markdown",
        value_name = "markdown"
    )]
    pub content: Option<String>,
    #[arg(long = "content-file", help = "Read project overview markdown from a file", value_name = "path", value_parser = super::nonempty_string)]
    pub content_file: Option<String>,
    #[arg(long = "team", short = 't', help = "Team key, name, or ID (required, can be repeated for multiple teams)", value_name = "team", value_parser = super::nonempty_string)]
    pub team: Vec<String>,
    #[arg(long = "lead", short = 'l', help = "Project lead (username, email, or @me)", value_name = "lead", value_parser = super::nonempty_string)]
    pub lead: Option<String>,
    #[arg(long = "status", short = 's', help = "Project status (planned, started, paused, completed, canceled, backlog)", value_name = "status", value_parser = super::nonempty_string)]
    pub status: Option<String>,
    #[arg(long = "start-date", help = "Start date (YYYY-MM-DD)", value_name = "startDate", value_parser = super::nonempty_string)]
    pub start_date: Option<String>,
    #[arg(long = "target-date", help = "Target completion date (YYYY-MM-DD)", value_name = "targetDate", value_parser = super::nonempty_string)]
    pub target_date: Option<String>,
    #[arg(long = "priority", help = "Project priority (none, urgent, high, medium, low)", value_name = "priority", value_parser = super::nonempty_string)]
    pub priority: Option<String>,
    #[arg(long = "label", help = "Project label associated with the project. May be repeated.", value_name = "label", value_parser = super::nonempty_string)]
    pub label: Vec<String>,
    #[arg(long = "member", help = "Project member (username, email, display name, or @me). May be repeated.", value_name = "user", value_parser = super::nonempty_string)]
    pub member: Vec<String>,
    #[arg(long = "icon", help = "Project icon", value_name = "icon")]
    pub icon: Option<String>,
    #[arg(
        long = "color",
        help = "Project color as a HEX string",
        value_name = "color"
    )]
    pub color: Option<String>,
    #[arg(long = "initiative", help = "Add to initiative immediately (ID, slug, or name)", value_name = "initiative", value_parser = super::nonempty_string)]
    pub initiative: Option<String>,
    #[arg(long = "template", help = "Project template to apply, by name or ID (workspace templates plus those of the project's teams). The template fills in anything you do not pass; explicit flags override it. Applied on create only.", value_name = "template", value_parser = super::nonempty_string)]
    pub template: Option<String>,
    #[arg(
        long = "interactive",
        short = 'i',
        help = "Interactive mode (default if no flags provided)"
    )]
    pub interactive: bool,
    #[arg(long = "json", short = 'j', help = "Output created project as JSON")]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct ProjectUpdate {
    #[arg(value_name = "projectId")]
    pub project_id: String,
    #[arg(long = "name", short = 'n', help = "Project name", value_name = "name", value_parser = super::nonempty_string)]
    pub name: Option<String>,
    #[arg(
        long = "description",
        short = 'd',
        help = "Project description (max 255 characters, enforced by Linear's API)",
        value_name = "description"
    )]
    pub description: Option<String>,
    #[arg(long = "description-file", short = 'f', help = "Read project description from file (still subject to the 255-character API limit)", value_name = "path", value_parser = super::nonempty_string)]
    pub description_file: Option<String>,
    #[arg(
        long = "content",
        help = "Project overview markdown",
        value_name = "markdown"
    )]
    pub content: Option<String>,
    #[arg(long = "content-file", help = "Read project overview markdown from a file", value_name = "path", value_parser = super::nonempty_string)]
    pub content_file: Option<String>,
    #[arg(long = "status", short = 's', help = "Status (planned, started, paused, completed, canceled, backlog)", value_name = "status", value_parser = super::nonempty_string)]
    pub status: Option<String>,
    #[arg(long = "lead", short = 'l', help = "Project lead (username, email, or @me). Use --clear-lead to remove it", value_name = "lead", value_parser = super::nonempty_string)]
    pub lead: Option<String>,
    #[arg(
        long = "clear-lead",
        help = "Remove the project's lead (cannot be combined with --lead)"
    )]
    pub clear_lead: bool,
    #[arg(long = "start-date", help = "Start date (YYYY-MM-DD). Use --clear-start-date to remove it", value_name = "startDate", value_parser = super::nonempty_string)]
    pub start_date: Option<String>,
    #[arg(
        long = "clear-start-date",
        help = "Remove the project's start date (cannot be combined with --start-date)"
    )]
    pub clear_start_date: bool,
    #[arg(long = "target-date", help = "Target date (YYYY-MM-DD). Use --clear-target-date to remove it", value_name = "targetDate", value_parser = super::nonempty_string)]
    pub target_date: Option<String>,
    #[arg(
        long = "clear-target-date",
        help = "Remove the project's target date (cannot be combined with --target-date)"
    )]
    pub clear_target_date: bool,
    #[arg(long = "team", short = 't', help = "Team key, name, or ID; replaces the project's entire team set. May be repeated. Use --add-team/--remove-team to change teams incrementally.", value_name = "team", value_parser = super::nonempty_string)]
    pub team: Vec<String>,
    #[arg(long = "add-team", help = "Add a team to the project, keeping its existing teams. May be repeated.", value_name = "team", value_parser = super::nonempty_string)]
    pub add_team: Vec<String>,
    #[arg(long = "remove-team", help = "Remove a team from the project, keeping its other teams. May be repeated.", value_name = "team", value_parser = super::nonempty_string)]
    pub remove_team: Vec<String>,
    #[arg(long = "label", help = "Project label; replaces the project's entire label set. May be repeated. Use --add-label/--remove-label to change labels incrementally.", value_name = "label", value_parser = super::nonempty_string)]
    pub label: Vec<String>,
    #[arg(long = "add-label", help = "Add a label to the project, keeping its existing labels. May be repeated.", value_name = "label", value_parser = super::nonempty_string)]
    pub add_label: Vec<String>,
    #[arg(long = "remove-label", help = "Remove a label from the project, keeping its other labels (does not delete the label). May be repeated.", value_name = "label", value_parser = super::nonempty_string)]
    pub remove_label: Vec<String>,
    #[arg(long = "initiative", help = "Initiative ID, slug, or name; replaces the project's entire initiative set. May be repeated. Use --add-initiative/--remove-initiative to change initiatives incrementally.", value_name = "initiative", value_parser = super::nonempty_string)]
    pub initiative: Vec<String>,
    #[arg(long = "add-initiative", help = "Add the project to an initiative, keeping its existing initiatives. May be repeated.", value_name = "initiative", value_parser = super::nonempty_string)]
    pub add_initiative: Vec<String>,
    #[arg(long = "remove-initiative", help = "Remove the project from an initiative, keeping its other initiatives (does not delete the initiative). May be repeated.", value_name = "initiative", value_parser = super::nonempty_string)]
    pub remove_initiative: Vec<String>,
}

#[derive(Debug, Args)]
pub struct ProjectDelete {
    #[arg(value_name = "projectId")]
    pub project_id: String,
    #[arg(long = "force", short = 'f', help = "Skip confirmation prompt")]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct ProjectComment {
    #[command(subcommand)]
    pub command: Option<ProjectCommentCommand>,
}

#[derive(Debug, Subcommand)]
pub enum ProjectCommentCommand {
    #[command(
        name = "add",
        about = "Add a comment or reply to a project's discussion (by ID, slug, or name)",
        long_about = "Add a comment or reply to a project's discussion (by ID, slug, or name)\n\nLinear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,\nand `[Name](url)` do not. Get a person's URL from the `url` field of\n`linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.\nRun `linear markdown` for collapsible sections and the full reference."
    )]
    Add(ProjectCommentAdd),
    #[command(
        name = "list",
        about = "List comments on a project (by ID, slug, or name)"
    )]
    List(ProjectCommentList),
}

#[derive(Debug, Args)]
pub struct ProjectCommentAdd {
    #[arg(value_name = "project")]
    pub project: String,
    #[arg(long = "body", short = 'b', help = "Comment body text", value_name = "text", value_parser = super::nonempty_string)]
    pub body: Option<String>,
    #[arg(long = "body-file", help = "Read comment body from a file (preferred for markdown content)", value_name = "path", value_parser = super::nonempty_string)]
    pub body_file: Option<String>,
    #[arg(long = "parent", short = 'p', visible_aliases = ["reply-to"], help = "Reply to a top-level comment by ID (the reply joins that thread)", value_name = "commentId", value_parser = super::nonempty_string)]
    pub parent: Option<String>,
}

#[derive(Debug, Args)]
pub struct ProjectCommentList {
    #[arg(value_name = "project")]
    pub project: String,
    #[arg(long = "json", short = 'j', help = "Output as JSON")]
    pub json: bool,
}
