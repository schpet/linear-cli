use clap::{Args, Subcommand, ValueEnum};

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct Issue {
    #[command(subcommand)]
    pub command: IssueCommand,
}

#[derive(Debug, Subcommand)]
pub enum IssueCommand {
    #[command(name = "id", about = "Print the issue based on the current git branch")]
    Id(IssueId),
    #[command(name = "mine", about = "List your issues", visible_aliases = ["list", "l"])]
    Mine(IssueMine),
    #[command(name = "query", about = "Query issues with structured filters", visible_aliases = ["q"])]
    Query(IssueQuery),
    #[command(name = "title", about = "Print the issue title")]
    Title(IssueTitle),
    #[command(name = "start", about = "Start working on an issue")]
    Start(IssueStart),
    #[command(name = "view", about = "View issue details (default) or open in browser/app", visible_aliases = ["v"])]
    View(IssueView),
    #[command(name = "url", about = "Print the issue URL")]
    Url(IssueUrl),
    #[command(
        name = "describe",
        about = "Print the issue title and Linear-issue trailer"
    )]
    Describe(IssueDescribe),
    #[command(
        name = "commits",
        about = "Show all commits for a Linear issue (jj only)"
    )]
    Commits(IssueCommits),
    #[command(name = "pull-request", about = "Create a GitHub pull request with issue details", visible_aliases = ["pr"])]
    PullRequest(IssuePullRequest),
    #[command(
        name = "archive",
        about = "Archive an issue",
        long_about = "Archive an issue\n\nLinear archives closed issues on its own, and its docs say \"archiving happens automatically with no option to manually archive items\". Prefer closing (issue update --state) and letting auto-archive run, or issue delete to trash. This command calls the issueArchive mutation, which the Linear app and its official MCP server do not expose; archived issues drop out of list, query, and search results unless --include-archived is passed. See https://linear.app/docs/delete-archive-issues"
    )]
    Archive(IssueArchive),
    #[command(name = "delete", about = "Delete an issue", visible_aliases = ["d"])]
    Delete(IssueDelete),
    #[command(
        name = "create",
        about = "Create a linear issue",
        long_about = "Create a linear issue\n\nLinear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,\nand `[Name](url)` do not. Get a person's URL from the `url` field of\n`linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.\nRun `linear markdown` for collapsible sections and the full reference."
    )]
    Create(IssueCreate),
    #[command(
        name = "update",
        about = "Update a linear issue",
        long_about = "Update a linear issue\n\nLinear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,\nand `[Name](url)` do not. Get a person's URL from the `url` field of\n`linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.\nRun `linear markdown` for collapsible sections and the full reference."
    )]
    Update(IssueUpdate),
    #[command(name = "comment", about = "Manage issue comments")]
    Comment(IssueComment),
    #[command(
        name = "attach",
        about = "Create a sidebar link attachment on an issue (images do not render inline)"
    )]
    Attach(IssueAttach),
    #[command(name = "link", about = "Link a URL to an issue")]
    Link(IssueLink),
    #[command(name = "relation", about = "Manage issue relations (dependencies)")]
    Relation(IssueRelation),
    #[command(name = "agent-session", about = "Manage agent sessions for an issue")]
    AgentSession(IssueAgentSession),
}

#[derive(Debug, Args)]
pub struct IssueId {}

#[derive(Debug, Args)]
pub struct IssueMine {
    #[arg(long = "state", short = 's', help = "Filter by workflow state type (triage, backlog, unstarted, started, completed, canceled), name, or ID (can be repeated for multiple states)", value_name = "state", value_parser = super::nonempty_string, default_values = ["unstarted"])]
    pub state: Vec<String>,
    #[arg(long = "all-states", help = "Show issues from all states")]
    pub all_states: bool,
    #[arg(
        long = "sort",
        help = "Sort order (default: priority, can also be set via LINEAR_ISSUE_SORT)",
        value_name = "sort"
    )]
    pub sort: Option<super::Sort>,
    #[arg(long = "team", help = "Team key, name, or ID to list issues for (if not your default team)", value_name = "team", value_parser = super::nonempty_string)]
    pub team: Option<String>,
    #[arg(long = "project", help = "Filter by project (UUID, slug ID, or name)", value_name = "project", value_parser = super::nonempty_string)]
    pub project: Option<String>,
    #[arg(long = "project-label", help = "Filter by project label name (shows issues from all projects with this label)", value_name = "projectLabel", value_parser = super::nonempty_string)]
    pub project_label: Option<String>,
    #[arg(long = "cycle", help = "Filter by cycle name, number, 'active'/'now', 'next', 'previous', or a relative offset like +1", value_name = "cycle", value_parser = super::nonempty_string)]
    pub cycle: Option<String>,
    #[arg(long = "milestone", help = "Filter by project milestone (UUID, or name when --project is set)", value_name = "milestone", value_parser = super::nonempty_string)]
    pub milestone: Option<String>,
    #[arg(long = "label", short = 'l', help = "Filter by label name (can be repeated for multiple labels)", value_name = "label", value_parser = super::nonempty_string)]
    pub label: Vec<String>,
    #[arg(long = "limit", help = "Maximum number of issues to fetch (default: 50, use 0 for unlimited)", value_name = "limit", value_parser = super::numeric::issue_limit, default_value = "50")]
    pub limit: super::numeric::IssueLimit,
    #[arg(long = "created-after", help = "Filter issues created after this date (ISO 8601 or YYYY-MM-DD)", value_name = "date", value_parser = super::nonempty_string)]
    pub created_after: Option<String>,
    #[arg(long = "updated-after", help = "Filter issues updated after this date (ISO 8601 or YYYY-MM-DD)", value_name = "date", value_parser = super::nonempty_string)]
    pub updated_after: Option<String>,
    #[arg(long = "assignee", help = "Removed: use `issue query --assignee` instead", hide = true, value_name = "assignee", value_parser = super::nonempty_string)]
    pub assignee: Option<String>,
    #[arg(
        long = "all-assignees",
        short = 'A',
        help = "Removed: use `issue query --all-assignees` instead",
        hide = true
    )]
    pub all_assignees: bool,
    #[arg(
        long = "unassigned",
        short = 'U',
        help = "Removed: use `issue query --unassigned` instead",
        hide = true
    )]
    pub unassigned: bool,
    #[arg(long = "web", short = 'w', help = "Open in web browser")]
    pub web: bool,
    #[arg(long = "app", short = 'a', help = "Open in Linear.app")]
    pub app: bool,
    #[arg(long = "no-pager", help = "Disable automatic paging for long output")]
    pub no_pager: bool,
}

#[derive(Debug, Args)]
pub struct IssueQuery {
    #[arg(long = "search", help = "Full-text search term", value_name = "term", value_parser = super::nonempty_string)]
    pub search: Option<String>,
    #[arg(
        long = "search-comments",
        help = "Also search inside issue comments (requires --search)"
    )]
    pub search_comments: bool,
    #[arg(long = "team", help = "Filter by team key, name, or ID (can be repeated for multiple teams)", value_name = "team", value_parser = super::nonempty_string)]
    pub team: Vec<String>,
    #[arg(long = "all-teams", help = "Query across all teams")]
    pub all_teams: bool,
    #[arg(long = "state", short = 's', help = "Filter by workflow state type (triage, backlog, unstarted, started, completed, canceled), name, or ID (can be repeated for multiple states)", value_name = "state", value_parser = super::nonempty_string)]
    pub state: Vec<String>,
    #[arg(
        long = "all-states",
        help = "Show issues from all states (this is the default)"
    )]
    pub all_states: bool,
    #[arg(long = "assignee", help = "Filter by assignee (username)", value_name = "assignee", value_parser = super::nonempty_string)]
    pub assignee: Option<String>,
    #[arg(
        long = "all-assignees",
        short = 'A',
        help = "Show issues for all assignees (this is the default)"
    )]
    pub all_assignees: bool,
    #[arg(long = "unassigned", short = 'U', help = "Show only unassigned issues")]
    pub unassigned: bool,
    #[arg(
        long = "sort",
        help = "Sort order: manual or priority (default: priority, not available with --search)",
        value_name = "sort"
    )]
    pub sort: Option<super::Sort>,
    #[arg(long = "project", help = "Filter by project (UUID, slug ID, or name)", value_name = "project", value_parser = super::nonempty_string)]
    pub project: Option<String>,
    #[arg(long = "project-label", help = "Filter by project label name (shows issues from all projects with this label)", value_name = "projectLabel", value_parser = super::nonempty_string)]
    pub project_label: Option<String>,
    #[arg(long = "cycle", help = "Filter by cycle name, number, 'active'/'now', 'next', 'previous', or a relative offset like +1", value_name = "cycle", value_parser = super::nonempty_string)]
    pub cycle: Option<String>,
    #[arg(long = "milestone", help = "Filter by project milestone (UUID, or name when --project is set)", value_name = "milestone", value_parser = super::nonempty_string)]
    pub milestone: Option<String>,
    #[arg(long = "label", short = 'l', help = "Filter by label name (can be repeated for multiple labels)", value_name = "label", value_parser = super::nonempty_string)]
    pub label: Vec<String>,
    #[arg(long = "limit", help = "Maximum number of issues to fetch (default: 50, use 0 for unlimited)", value_name = "limit", value_parser = super::numeric::issue_limit, default_value = "50")]
    pub limit: super::numeric::IssueLimit,
    #[arg(long = "created-after", help = "Filter issues created after this date (ISO 8601 or YYYY-MM-DD)", value_name = "date", value_parser = super::nonempty_string)]
    pub created_after: Option<String>,
    #[arg(long = "updated-after", help = "Filter issues updated after this date (ISO 8601 or YYYY-MM-DD)", value_name = "date", value_parser = super::nonempty_string)]
    pub updated_after: Option<String>,
    #[arg(long = "include-archived", help = "Include archived issues")]
    pub include_archived: bool,
    #[arg(long = "json", short = 'j', help = "Output results as JSON")]
    pub json: bool,
    #[arg(long = "no-pager", help = "Disable automatic paging for long output")]
    pub no_pager: bool,
}

#[derive(Debug, Args)]
pub struct IssueTitle {
    #[arg(value_name = "issueId")]
    pub issue_id: Option<String>,
}

#[derive(Debug, Args)]
pub struct IssueStart {
    #[arg(value_name = "issueId")]
    pub issue_id: Option<String>,
    #[arg(
        long = "all-assignees",
        short = 'A',
        help = "Show issues for all assignees"
    )]
    pub all_assignees: bool,
    #[arg(long = "unassigned", short = 'U', help = "Show only unassigned issues")]
    pub unassigned: bool,
    #[arg(
        long = "from-ref",
        short = 'f',
        help = "Git ref to create new branch from",
        value_name = "fromRef"
    )]
    pub from_ref: Option<String>,
    #[arg(
        long = "branch",
        short = 'b',
        help = "Custom branch name to use instead of the issue identifier",
        value_name = "branch"
    )]
    pub branch: Option<String>,
}

#[derive(Debug, Args)]
pub struct IssueView {
    #[arg(value_name = "issueId")]
    pub issue_id: Option<String>,
    #[arg(long = "web", short = 'w', help = "Open in web browser")]
    pub web: bool,
    #[arg(long = "app", short = 'a', help = "Open in Linear.app")]
    pub app: bool,
    #[arg(long = "no-comments", help = "Exclude comments from the output")]
    pub no_comments: bool,
    #[arg(
        long = "show-resolved-threads",
        help = "Include resolved comment threads in the output"
    )]
    pub show_resolved_threads: bool,
    #[arg(long = "no-pager", help = "Disable automatic paging for long output")]
    pub no_pager: bool,
    #[arg(long = "json", short = 'j', help = "Output issue data as JSON")]
    pub json: bool,
    #[arg(
        long = "no-download",
        help = "Keep remote URLs instead of downloading files"
    )]
    pub no_download: bool,
}

#[derive(Debug, Args)]
pub struct IssueUrl {
    #[arg(value_name = "issueId")]
    pub issue_id: Option<String>,
}

#[derive(Debug, Args)]
pub struct IssueDescribe {
    #[arg(value_name = "issueId")]
    pub issue_id: Option<String>,
    #[arg(long = "references", short = 'r', visible_aliases = ["ref"], help = "Use 'References' instead of 'Fixes' for the Linear issue link")]
    pub references: bool,
}

#[derive(Debug, Args)]
pub struct IssueCommits {
    #[arg(value_name = "issueId")]
    pub issue_id: Option<String>,
}

#[derive(Debug, Args)]
pub struct IssuePullRequest {
    #[arg(value_name = "issueId")]
    pub issue_id: Option<String>,
    #[arg(
        long = "base",
        help = "The branch into which you want your code merged",
        value_name = "branch"
    )]
    pub base: Option<String>,
    #[arg(long = "draft", help = "Create the pull request as a draft")]
    pub draft: bool,
    #[arg(
        long = "title",
        short = 't',
        help = "Optional title for the pull request (Linear issue ID will be prefixed)",
        value_name = "title"
    )]
    pub title: Option<String>,
    #[arg(
        long = "web",
        help = "Open the pull request in the browser after creating it"
    )]
    pub web: bool,
    #[arg(
        long = "head",
        help = "The branch that contains commits for your pull request",
        value_name = "branch"
    )]
    pub head: Option<String>,
    #[arg(
        long = "template",
        short = 'T',
        help = "Start the pull request body from this template file (the Linear issue URL is appended)",
        value_name = "file"
    )]
    pub template: Option<String>,
    #[arg(
        long = "no-template",
        help = "Ignore the pr_template config option for this pull request"
    )]
    pub no_template: bool,
}

#[derive(Debug, Args)]
pub struct IssueArchive {
    #[arg(value_name = "issueId")]
    pub issue_id: Option<String>,
    #[arg(long = "confirm", short = 'y', help = "Skip confirmation prompt")]
    pub confirm: bool,
    #[arg(long = "bulk", help = "Archive multiple issues by identifier (e.g., TC-123 TC-124)", value_name = "ids", num_args = 0.., value_parser = super::nonempty_string)]
    pub bulk: Option<Vec<String>>,
    #[arg(long = "bulk-file", help = "Read issue identifiers from a file (one per line)", value_name = "file", value_parser = super::nonempty_string)]
    pub bulk_file: Option<String>,
    #[arg(long = "bulk-stdin", help = "Read issue identifiers from stdin")]
    pub bulk_stdin: bool,
}

#[derive(Debug, Args)]
pub struct IssueDelete {
    #[arg(value_name = "issueId")]
    pub issue_id: Option<String>,
    #[arg(long = "confirm", short = 'y', help = "Skip confirmation prompt")]
    pub confirm: bool,
    #[arg(long = "bulk", help = "Delete multiple issues by identifier (e.g., TC-123 TC-124)", value_name = "ids", num_args = 0.., value_parser = super::nonempty_string)]
    pub bulk: Option<Vec<String>>,
    #[arg(long = "bulk-file", help = "Read issue identifiers from a file (one per line)", value_name = "file", value_parser = super::nonempty_string)]
    pub bulk_file: Option<String>,
    #[arg(long = "bulk-stdin", help = "Read issue identifiers from stdin")]
    pub bulk_stdin: bool,
}

#[derive(Debug, Args)]
pub struct IssueCreate {
    #[arg(long = "start", help = "Start the issue after creation")]
    pub start: bool,
    #[arg(
        long = "assignee",
        short = 'a',
        help = "Assign the issue to 'self' or someone (by username or name)",
        value_name = "assignee"
    )]
    pub assignee: Option<String>,
    #[arg(
        long = "due-date",
        help = "Due date of the issue",
        value_name = "dueDate"
    )]
    pub due_date: Option<String>,
    #[arg(
        long = "parent",
        help = "Parent issue (if any) as a team_number code",
        value_name = "parent"
    )]
    pub parent: Option<String>,
    #[arg(long = "priority", short = 'p', help = "Priority of the issue (1-4, descending priority)", value_name = "priority", value_parser = super::numeric::finite_decimal, allow_negative_numbers = true)]
    pub priority: Option<f64>,
    #[arg(long = "estimate", help = "Points estimate of the issue", value_name = "estimate", value_parser = super::numeric::finite_decimal, allow_negative_numbers = true)]
    pub estimate: Option<f64>,
    #[arg(
        long = "description",
        short = 'd',
        help = "Description of the issue",
        value_name = "description"
    )]
    pub description: Option<String>,
    #[arg(
        long = "description-file",
        help = "Read description from a file (preferred for markdown content)",
        value_name = "path"
    )]
    pub description_file: Option<String>,
    #[arg(
        long = "label",
        short = 'l',
        help = "Issue label associated with the issue. May be repeated.",
        value_name = "label"
    )]
    pub label: Vec<String>,
    #[arg(
        long = "team",
        help = "Team (key, name, or ID) for the issue, if not your default team",
        value_name = "team"
    )]
    pub team: Option<String>,
    #[arg(
        long = "project",
        help = "Project for the issue (UUID, slug ID, or name)",
        value_name = "project"
    )]
    pub project: Option<String>,
    #[arg(
        long = "state",
        short = 's',
        help = "Workflow state for the issue (by name or type)",
        value_name = "state"
    )]
    pub state: Option<String>,
    #[arg(
        long = "milestone",
        help = "Project milestone (UUID, or name when --project is set)",
        value_name = "milestone"
    )]
    pub milestone: Option<String>,
    #[arg(
        long = "cycle",
        help = "Cycle name, number, 'active'/'now', 'next', 'previous', or a relative offset like +1 (use --cycle=-1 for negatives)",
        value_name = "cycle"
    )]
    pub cycle: Option<String>,
    #[arg(
        long = "no-use-default-template",
        help = "Do not use default template for the issue"
    )]
    pub no_use_default_template: bool,
    #[arg(
        long = "template",
        help = "Issue template to apply, by name or ID (the team's templates plus workspace ones). Takes the place of the team's default template. The template fills in anything you do not pass: explicit flags override it, --label merges with the template's labels, and --description replaces the template body (omit it to keep the body). Makes --title optional.",
        value_name = "template"
    )]
    pub template: Option<String>,
    #[arg(long = "no-interactive", help = "Disable interactive prompts")]
    pub no_interactive: bool,
    #[arg(
        long = "title",
        short = 't',
        help = "Title of the issue",
        value_name = "title"
    )]
    pub title: Option<String>,
}

#[derive(Debug, Args)]
pub struct IssueUpdate {
    #[arg(value_name = "issueId")]
    pub issue_id: Option<String>,
    #[arg(
        long = "assignee",
        short = 'a',
        help = "Assign the issue to 'self' or someone (by username or name)",
        value_name = "assignee"
    )]
    pub assignee: Option<String>,
    #[arg(
        long = "unassign",
        help = "Clear the issue's assignee (cannot be combined with --assignee)"
    )]
    pub unassign: bool,
    #[arg(
        long = "due-date",
        help = "Due date of the issue. Use --clear-due-date to remove it",
        value_name = "dueDate"
    )]
    pub due_date: Option<String>,
    #[arg(
        long = "clear-due-date",
        help = "Remove the issue's due date (cannot be combined with --due-date)"
    )]
    pub clear_due_date: bool,
    #[arg(
        long = "parent",
        help = "Parent issue (if any) as a team_number code. Use --clear-parent to remove it",
        value_name = "parent"
    )]
    pub parent: Option<String>,
    #[arg(
        long = "clear-parent",
        help = "Remove the issue's parent (cannot be combined with --parent)"
    )]
    pub clear_parent: bool,
    #[arg(long = "priority", short = 'p', help = "Priority of the issue (1-4, descending priority)", value_name = "priority", value_parser = super::numeric::finite_decimal, allow_negative_numbers = true)]
    pub priority: Option<f64>,
    #[arg(long = "estimate", help = "Points estimate of the issue. Use --clear-estimate to remove it", value_name = "estimate", value_parser = super::numeric::finite_decimal, allow_negative_numbers = true)]
    pub estimate: Option<f64>,
    #[arg(
        long = "clear-estimate",
        help = "Remove the issue's estimate (cannot be combined with --estimate)"
    )]
    pub clear_estimate: bool,
    #[arg(
        long = "description",
        short = 'd',
        help = "Description of the issue",
        value_name = "description"
    )]
    pub description: Option<String>,
    #[arg(
        long = "description-file",
        help = "Read description from a file (preferred for markdown content)",
        value_name = "path"
    )]
    pub description_file: Option<String>,
    #[arg(
        long = "label",
        short = 'l',
        help = "Issue label associated with the issue; replaces the issue's entire label set. May be repeated. Use --add-label/--remove-label to change labels incrementally.",
        value_name = "label"
    )]
    pub label: Vec<String>,
    #[arg(
        long = "add-label",
        help = "Add a label to the issue, keeping its existing labels. May be repeated.",
        value_name = "label"
    )]
    pub add_label: Vec<String>,
    #[arg(
        long = "remove-label",
        help = "Remove a label from the issue, keeping its other labels (does not delete the label from the team). May be repeated.",
        value_name = "label"
    )]
    pub remove_label: Vec<String>,
    #[arg(
        long = "team",
        help = "Team (key, name, or ID) to move the issue to",
        value_name = "team"
    )]
    pub team: Option<String>,
    #[arg(
        long = "project",
        help = "Project to assign the issue to (UUID, slug ID, or name). Use --clear-project to remove it",
        value_name = "project"
    )]
    pub project: Option<String>,
    #[arg(
        long = "clear-project",
        help = "Remove the issue from its project (cannot be combined with --project or --milestone)"
    )]
    pub clear_project: bool,
    #[arg(
        long = "state",
        short = 's',
        help = "Workflow state for the issue (by name or type)",
        value_name = "state"
    )]
    pub state: Option<String>,
    #[arg(
        long = "milestone",
        help = "Project milestone (UUID, or name when --project is set or the issue already has a project). Use --clear-milestone to remove it",
        value_name = "milestone"
    )]
    pub milestone: Option<String>,
    #[arg(
        long = "clear-milestone",
        help = "Remove the issue from its project milestone (cannot be combined with --milestone)"
    )]
    pub clear_milestone: bool,
    #[arg(
        long = "cycle",
        help = "Cycle name, number, 'active'/'now', 'next', 'previous', or a relative offset like +1 (use --cycle=-1 for negatives). Use --clear-cycle to remove the issue from its cycle",
        value_name = "cycle"
    )]
    pub cycle: Option<String>,
    #[arg(long = "clear-cycle", help = "Remove the issue from its cycle")]
    pub clear_cycle: bool,
    #[arg(
        long = "title",
        short = 't',
        help = "Title of the issue",
        value_name = "title"
    )]
    pub title: Option<String>,
}

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct IssueComment {
    #[command(subcommand)]
    pub command: IssueCommentCommand,
}

#[derive(Debug, Subcommand)]
pub enum IssueCommentCommand {
    #[command(
        name = "add",
        about = "Add a comment or reply; images uploaded with --attach render inline",
        long_about = "Add a comment or reply; images uploaded with --attach render inline\n\nLinear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,\nand `[Name](url)` do not. Get a person's URL from the `url` field of\n`linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.\nRun `linear markdown` for collapsible sections and the full reference."
    )]
    Add(IssueCommentAdd),
    #[command(name = "delete", about = "Delete a comment")]
    Delete(IssueCommentDelete),
    #[command(
        name = "update",
        about = "Update an existing comment",
        long_about = "Update an existing comment\n\nLinear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,\nand `[Name](url)` do not. Get a person's URL from the `url` field of\n`linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.\nRun `linear markdown` for collapsible sections and the full reference."
    )]
    Update(IssueCommentUpdate),
    #[command(name = "list", about = "List comments for an issue")]
    List(IssueCommentList),
}

#[derive(Debug, Args)]
pub struct IssueCommentAdd {
    #[arg(value_name = "issueId")]
    pub issue_id: Option<String>,
    #[arg(long = "body", short = 'b', help = "Comment body text", value_name = "text", value_parser = super::nonempty_string)]
    pub body: Option<String>,
    #[arg(long = "body-file", help = "Read comment body from a file (preferred for markdown content)", value_name = "path", value_parser = super::nonempty_string)]
    pub body_file: Option<String>,
    #[arg(long = "parent", short = 'p', visible_aliases = ["reply-to"], help = "Reply to a top-level comment by ID (the reply joins that thread)", value_name = "commentId", value_parser = super::nonempty_string)]
    pub parent: Option<String>,
    #[arg(long = "id", help = "Caller-supplied UUID for the new comment", hide = true, value_name = "uuid", value_parser = super::nonempty_string)]
    pub id: Option<String>,
    #[arg(long = "attach", short = 'a', help = "Upload a file and add its Markdown link to the comment (images render inline; repeatable)", value_name = "filepath", value_parser = super::nonempty_string)]
    pub attach: Vec<String>,
    #[arg(
        long = "public",
        help = "Upload attached images to a public, unauthenticated URL (default: private, workspace-members only)"
    )]
    pub public: bool,
}

#[derive(Debug, Args)]
pub struct IssueCommentDelete {
    #[arg(value_name = "commentId")]
    pub comment_id: String,
}

#[derive(Debug, Args)]
pub struct IssueCommentUpdate {
    #[arg(value_name = "commentId")]
    pub comment_id: String,
    #[arg(long = "body", short = 'b', help = "New comment body text", value_name = "text", value_parser = super::nonempty_string)]
    pub body: Option<String>,
    #[arg(long = "body-file", help = "Read comment body from a file (preferred for markdown content)", value_name = "path", value_parser = super::nonempty_string)]
    pub body_file: Option<String>,
}

#[derive(Debug, Args)]
pub struct IssueCommentList {
    #[arg(value_name = "issueId")]
    pub issue_id: Option<String>,
    #[arg(long = "json", short = 'j', help = "Output as JSON")]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct IssueAttach {
    #[arg(value_name = "issueId")]
    pub issue_id: String,
    #[arg(value_name = "filepath")]
    pub filepath: String,
    #[arg(long = "title", short = 't', help = "Custom title for the attachment", value_name = "title", value_parser = super::nonempty_string)]
    pub title: Option<String>,
    #[arg(long = "comment", short = 'c', help = "Create a linked comment with this body; the file remains a sidebar attachment", value_name = "body", value_parser = super::nonempty_string)]
    pub comment: Option<String>,
    #[arg(
        long = "public",
        help = "Upload images to a public, unauthenticated URL (default: private, workspace-members only)"
    )]
    pub public: bool,
}

#[derive(Debug, Args)]
pub struct IssueLink {
    #[arg(value_name = "urlOrIssueId")]
    pub url_or_issue_id: String,
    #[arg(value_name = "url")]
    pub url: Option<String>,
    #[arg(long = "title", short = 't', help = "Custom title for the link", value_name = "title", value_parser = super::nonempty_string)]
    pub title: Option<String>,
}

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct IssueRelation {
    #[command(subcommand)]
    pub command: IssueRelationCommand,
}

#[derive(Debug, Subcommand)]
pub enum IssueRelationCommand {
    #[command(name = "add", about = "Add a relation between two issues")]
    Add(IssueRelationAdd),
    #[command(name = "delete", about = "Delete a relation between two issues")]
    Delete(IssueRelationDelete),
    #[command(name = "list", about = "List relations for an issue")]
    List(IssueRelationList),
}

/// The CLI's four accepted spellings; the API direction is modeled separately.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum RelationType {
    Blocks,
    BlockedBy,
    Related,
    Duplicate,
}

impl RelationType {
    pub fn spelling(self) -> &'static str {
        match self {
            Self::Blocks => "blocks",
            Self::BlockedBy => "blocked-by",
            Self::Related => "related",
            Self::Duplicate => "duplicate",
        }
    }
}

#[derive(Debug, Args)]
pub struct IssueRelationAdd {
    #[arg(value_name = "issueId")]
    pub issue_id: String,
    #[arg(value_name = "relationType", value_enum, ignore_case = true)]
    pub relation_type: RelationType,
    #[arg(value_name = "relatedIssueId")]
    pub related_issue_id: String,
}

#[derive(Debug, Args)]
pub struct IssueRelationDelete {
    #[arg(value_name = "issueId")]
    pub issue_id: String,
    #[arg(value_name = "relationType", value_enum, ignore_case = true)]
    pub relation_type: RelationType,
    #[arg(value_name = "relatedIssueId")]
    pub related_issue_id: String,
}

#[derive(Debug, Args)]
pub struct IssueRelationList {
    #[arg(value_name = "issueId")]
    pub issue_id: Option<String>,
}

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct IssueAgentSession {
    #[command(subcommand)]
    pub command: IssueAgentSessionCommand,
}

#[derive(Debug, Subcommand)]
pub enum IssueAgentSessionCommand {
    #[command(name = "list", about = "List agent sessions for an issue")]
    List(IssueAgentSessionList),
    #[command(name = "view", about = "View agent session details", visible_aliases = ["v"])]
    View(IssueAgentSessionView),
}

#[derive(Debug, Args)]
pub struct IssueAgentSessionList {
    #[arg(value_name = "issueId")]
    pub issue_id: Option<String>,
    #[arg(long = "json", short = 'j', help = "Output as JSON")]
    pub json: bool,
    #[arg(
        long = "status",
        help = "Filter by session status",
        value_name = "status"
    )]
    pub status: Option<super::AgentSessionStatus>,
}

#[derive(Debug, Args)]
pub struct IssueAgentSessionView {
    #[arg(value_name = "sessionId")]
    pub session_id: String,
    #[arg(long = "json", short = 'j', help = "Output as JSON")]
    pub json: bool,
}
