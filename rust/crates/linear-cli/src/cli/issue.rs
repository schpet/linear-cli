use chrono::{DateTime, NaiveDate, Utc};
use clap::builder::NonEmptyStringValueParser;
use clap::{Args, Subcommand, ValueEnum};

use super::LINEAR_MARKDOWN;

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct Issue {
    #[command(subcommand)]
    pub command: IssueCommand,
}

#[derive(Debug, Subcommand)]
pub enum IssueCommand {
    /// List your issues
    #[command(visible_aliases = ["list", "l"])]
    Mine(IssueMine),
    /// Find issues by filters or full-text search
    #[command(visible_alias = "q")]
    Query(IssueQuery),
    /// Show an issue
    #[command(visible_alias = "v")]
    View(IssueView),
    /// Create an issue
    #[command(after_long_help = LINEAR_MARKDOWN)]
    Create(IssueCreate),
    /// Update an issue
    #[command(after_long_help = LINEAR_MARKDOWN)]
    Update(IssueUpdate),
    /// Delete an issue (moves it to the trash)
    #[command(visible_alias = "d")]
    Delete(IssueDelete),
    /// Archive an issue
    ///
    /// Linear archives closed issues on its own, so prefer closing an issue
    /// (`issue update --state`) and letting auto-archive run, or `issue delete`
    /// to trash it. Archived issues drop out of list, query, and search results
    /// unless --include-archived is passed. See
    /// https://linear.app/docs/delete-archive-issues
    Archive(IssueArchive),
    /// Start an issue: switch to its branch and mark it started
    Start(IssueStart),
    /// Print the issue ID of the current branch or jj change
    Id(IssueId),
    /// Print an issue's title
    Title(IssueTitle),
    /// Print an issue's URL
    Url(IssueUrl),
    /// Print an issue's title and a Linear-issue trailer, for commit messages
    Describe(IssueDescribe),
    /// List the commits that reference an issue (jj only)
    Commits(IssueCommits),
    /// Open a GitHub pull request for an issue
    #[command(visible_alias = "pr")]
    PullRequest(IssuePullRequest),
    /// Comment on an issue
    Comment(IssueComment),
    /// Upload a file and attach it to an issue
    ///
    /// The file is listed in the issue's sidebar; images do not render inline.
    /// To show an image in the conversation, use `issue comment add --attach`.
    Attach(IssueAttach),
    /// Link a URL to an issue
    Link(IssueLink),
    /// Manage relations between issues, like blocks and duplicates
    Relation(IssueRelation),
    /// Inspect agent sessions on an issue
    AgentSession(IssueAgentSession),
}

#[derive(Debug, Args)]
pub struct IssueId {}

#[derive(Debug, Args)]
pub struct IssueMine {
    /// Show issues in this state: a type (triage, backlog, unstarted, started,
    /// completed, canceled), name, or ID; repeatable
    #[arg(long, short, value_parser = NonEmptyStringValueParser::new(), default_values = ["unstarted"])]
    pub state: Vec<String>,
    /// Show issues in every state
    #[arg(long, conflicts_with = "state")]
    pub all_states: bool,
    /// Sort order [default: the issue_sort setting, or priority]
    #[arg(long)]
    pub sort: Option<crate::config::IssueSort>,
    /// Show this team's issues (key, name, or ID); defaults to the configured team
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    pub team: Option<String>,
    /// Show only this project's issues (ID, slug, or name)
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    pub project: Option<String>,
    /// Show only issues in projects with this project label
    #[arg(long, value_name = "LABEL", conflicts_with_all = ["project", "milestone"], value_parser = NonEmptyStringValueParser::new())]
    pub project_label: Option<String>,
    /// Show only this cycle's issues: a name, number, `active`, `next`,
    /// `previous`, or an offset like +1
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    pub cycle: Option<String>,
    /// Show only this milestone's issues (ID, or name with --project)
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    pub milestone: Option<String>,
    /// Show only issues with this label; repeat to require several
    #[arg(long, short, value_parser = NonEmptyStringValueParser::new())]
    pub label: Vec<String>,
    /// Maximum number of issues to show (a number or `all`)
    #[arg(long, value_parser = super::limit::parse, default_value = "50")]
    pub limit: super::Limit,
    /// Show only issues created after this date (YYYY-MM-DD or RFC 3339)
    #[arg(long, value_name = "DATE", value_parser = super::values::date_or_datetime)]
    pub created_after: Option<DateTime<Utc>>,
    /// Show only issues updated after this date (YYYY-MM-DD or RFC 3339)
    #[arg(long, value_name = "DATE", value_parser = super::values::date_or_datetime)]
    pub updated_after: Option<DateTime<Utc>>,
    #[arg(long, hide = true, value_parser = NonEmptyStringValueParser::new())]
    pub assignee: Option<String>,
    #[arg(long, short = 'A', hide = true)]
    pub all_assignees: bool,
    #[arg(long, short = 'U', hide = true)]
    pub unassigned: bool,
    /// Open the list in the browser
    #[arg(long, short)]
    pub web: bool,
    /// Open the list in the Linear app
    #[arg(long, short)]
    pub app: bool,
    /// Do not page long output
    #[arg(long)]
    pub no_pager: bool,
}

#[derive(Debug, Args)]
pub struct IssueQuery {
    /// Search issue titles and descriptions for this text
    #[arg(long, value_name = "TEXT", value_parser = NonEmptyStringValueParser::new(), conflicts_with = "milestone")]
    pub search: Option<String>,
    /// Also search comments (with --search)
    #[arg(long, requires = "search")]
    pub search_comments: bool,
    /// Show this team's issues (key, name, or ID); repeatable [default: the configured team]
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    pub team: Vec<String>,
    /// Show every team's issues
    #[arg(long, conflicts_with = "team")]
    pub all_teams: bool,
    /// Show issues in this state: a type (triage, backlog, unstarted, started,
    /// completed, canceled), name, or ID; repeatable
    #[arg(long, short, value_parser = NonEmptyStringValueParser::new())]
    pub state: Vec<String>,
    #[arg(long, conflicts_with = "state", hide = true)]
    pub all_states: bool,
    /// Show only issues assigned to this user (username, email, name, or @me)
    #[arg(long, value_name = "USER", value_parser = NonEmptyStringValueParser::new())]
    pub assignee: Option<String>,
    #[arg(long, short = 'A', conflicts_with_all = ["assignee", "unassigned"], hide = true)]
    pub all_assignees: bool,
    /// Show only unassigned issues
    #[arg(long, short = 'U', conflicts_with = "assignee")]
    pub unassigned: bool,
    /// Sort order, except with --search [default: the issue_sort setting, or priority]
    #[arg(long, conflicts_with = "search")]
    pub sort: Option<crate::config::IssueSort>,
    /// Show only this project's issues (ID, slug, or name)
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    pub project: Option<String>,
    /// Show only issues in projects with this project label
    #[arg(long, value_name = "LABEL", conflicts_with_all = ["project", "milestone"], value_parser = NonEmptyStringValueParser::new())]
    pub project_label: Option<String>,
    /// Show only this cycle's issues: a name, number, `active`, `next`,
    /// `previous`, or an offset like +1
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    pub cycle: Option<String>,
    /// Show only this milestone's issues (ID, or name with --project)
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    pub milestone: Option<String>,
    /// Show only issues with this label; repeat to require several
    #[arg(long, short, value_parser = NonEmptyStringValueParser::new())]
    pub label: Vec<String>,
    /// Maximum number of issues to show (a number or `all`)
    #[arg(long, value_parser = super::limit::parse, default_value = "50")]
    pub limit: super::Limit,
    /// Show only issues created after this date (YYYY-MM-DD or RFC 3339)
    #[arg(long, value_name = "DATE", value_parser = super::values::date_or_datetime)]
    pub created_after: Option<DateTime<Utc>>,
    /// Show only issues updated after this date (YYYY-MM-DD or RFC 3339)
    #[arg(long, value_name = "DATE", value_parser = super::values::date_or_datetime)]
    pub updated_after: Option<DateTime<Utc>>,
    /// Include archived issues
    #[arg(long)]
    pub include_archived: bool,
    /// Print JSON
    #[arg(long, short)]
    pub json: bool,
    /// Do not page long output
    #[arg(long)]
    pub no_pager: bool,
}

#[derive(Debug, Args)]
pub struct IssueTitle {
    /// Issue ID like ENG-123, or a URL; defaults to the current branch's issue
    #[arg(value_name = "ISSUE")]
    pub issue_id: Option<String>,
}

#[derive(Debug, Args)]
pub struct IssueStart {
    /// Issue ID like ENG-123, or a URL; asked for when omitted
    #[arg(value_name = "ISSUE", value_parser = NonEmptyStringValueParser::new())]
    pub issue_id: Option<String>,
    /// Offer issues of every assignee in the picker
    #[arg(long, short = 'A')]
    pub all_assignees: bool,
    /// Offer only unassigned issues in the picker
    #[arg(long, short = 'U')]
    pub unassigned: bool,
    /// Git ref to create the branch from
    #[arg(long, short, value_name = "REF")]
    pub from_ref: Option<String>,
    /// Branch name to use instead of the issue's
    #[arg(long, short)]
    pub branch: Option<String>,
}

#[derive(Debug, Args)]
pub struct IssueView {
    /// Issue ID like ENG-123, or a URL; defaults to the current branch's issue
    #[arg(value_name = "ISSUE")]
    pub issue_id: Option<String>,
    /// Open the issue in the browser
    #[arg(long, short)]
    pub web: bool,
    /// Open the issue in the Linear app
    #[arg(long, short)]
    pub app: bool,
    /// Leave out comments
    #[arg(long)]
    pub no_comments: bool,
    /// Include resolved comment threads
    #[arg(long)]
    pub show_resolved_threads: bool,
    /// Do not page long output
    #[arg(long)]
    pub no_pager: bool,
    /// Print JSON
    #[arg(long, short)]
    pub json: bool,
    /// Keep remote image and file URLs instead of downloading them
    #[arg(long)]
    pub no_download: bool,
}

#[derive(Debug, Args)]
pub struct IssueUrl {
    /// Issue ID like ENG-123, or a URL; defaults to the current branch's issue
    #[arg(value_name = "ISSUE")]
    pub issue_id: Option<String>,
}

#[derive(Debug, Args)]
pub struct IssueDescribe {
    /// Issue ID like ENG-123, or a URL; defaults to the current branch's issue
    #[arg(value_name = "ISSUE")]
    pub issue_id: Option<String>,
    /// Write "References" instead of "Fixes" in the trailer
    #[arg(long, short, visible_alias = "ref")]
    pub references: bool,
}

#[derive(Debug, Args)]
pub struct IssueCommits {
    /// Issue ID like ENG-123, or a URL; defaults to the current change's issue
    #[arg(value_name = "ISSUE")]
    pub issue_id: Option<String>,
}

#[derive(Debug, Args)]
pub struct IssuePullRequest {
    /// Issue ID like ENG-123, or a URL; defaults to the current branch's issue
    #[arg(value_name = "ISSUE")]
    pub issue_id: Option<String>,
    /// Branch to merge into
    #[arg(long, value_name = "BRANCH")]
    pub base: Option<String>,
    /// Open the pull request as a draft
    #[arg(long)]
    pub draft: bool,
    /// Pull request title, after the issue ID [default: the issue title]
    #[arg(long, short)]
    pub title: Option<String>,
    /// Open the pull request in the browser
    #[arg(long)]
    pub web: bool,
    /// Branch that holds the commits
    #[arg(long, value_name = "BRANCH")]
    pub head: Option<String>,
    /// Start the body from this template file; the issue URL is appended
    #[arg(long, short = 'T', value_name = "FILE")]
    pub template: Option<String>,
    /// Ignore the pr_template setting
    #[arg(long)]
    pub no_template: bool,
}

#[derive(Debug, Args)]
pub struct IssueArchive {
    /// Issue ID like ENG-123, or a URL; defaults to the current branch's issue
    #[arg(value_name = "ISSUE", conflicts_with_all = ["bulk", "bulk_file", "bulk_stdin"])]
    pub issue_id: Option<String>,
    #[command(flatten)]
    pub confirm: super::ConfirmArgs,
    #[command(flatten)]
    pub bulk: super::BulkArgs,
}

#[derive(Debug, Args)]
pub struct IssueDelete {
    /// Issue ID like ENG-123, or a URL; defaults to the current branch's issue
    #[arg(value_name = "ISSUE", conflicts_with_all = ["bulk", "bulk_file", "bulk_stdin"])]
    pub issue_id: Option<String>,
    #[command(flatten)]
    pub confirm: super::ConfirmArgs,
    #[command(flatten)]
    pub bulk: super::BulkArgs,
}

#[derive(Debug, Args)]
pub struct IssueCreate {
    /// Issue title
    #[arg(long, short)]
    pub title: Option<String>,
    /// Issue description, in Markdown
    #[arg(long, short)]
    pub description: Option<String>,
    /// Read the description from a Markdown file
    #[arg(long, value_name = "FILE")]
    pub description_file: Option<String>,
    /// Team (key, name, or ID); defaults to the configured team
    #[arg(long)]
    pub team: Option<String>,
    /// Assignee: a username, email, name, or self
    #[arg(long, short, value_name = "USER")]
    pub assignee: Option<String>,
    /// Workflow state, by name or type
    #[arg(long, short)]
    pub state: Option<String>,
    /// Priority, by name or number (0 none, 1 urgent to 4 low)
    #[arg(long, short, ignore_case = true)]
    pub priority: Option<super::values::Priority>,
    /// Estimate, in points
    #[arg(long, value_name = "POINTS", value_parser = super::values::estimate, allow_negative_numbers = true)]
    pub estimate: Option<i32>,
    /// Label; repeat for several labels
    #[arg(long, short)]
    pub label: Vec<String>,
    /// Due date (YYYY-MM-DD)
    #[arg(long, value_name = "DATE", value_parser = super::values::date)]
    pub due_date: Option<NaiveDate>,
    /// Parent issue, like ENG-123
    #[arg(long, value_name = "ISSUE")]
    pub parent: Option<String>,
    /// Project (ID, slug, or name)
    #[arg(long)]
    pub project: Option<String>,
    /// Project milestone (ID, or name with --project)
    #[arg(long)]
    pub milestone: Option<String>,
    /// Cycle: a name, number, `active`, `next`, `previous`, or an offset like
    /// +1 (write --cycle=-1 for a negative offset)
    #[arg(long)]
    pub cycle: Option<String>,
    /// Start from this issue template (name or ID) instead of the team's default
    ///
    /// The team's templates and workspace templates are searched. The template
    /// fills in anything you do not pass: flags override it, --label adds to
    /// its labels, and --description replaces its body. With a template,
    /// --title is optional.
    #[arg(long)]
    pub template: Option<String>,
    /// Do not apply the team's default template
    #[arg(long)]
    pub no_use_default_template: bool,
    /// Start the issue after creating it
    #[arg(long)]
    pub start: bool,
    /// Do not prompt for missing values
    #[arg(long)]
    pub no_interactive: bool,
}

#[derive(Debug, Args)]
pub struct IssueUpdate {
    /// Issue ID like ENG-123, or a URL; defaults to the current branch's issue
    #[arg(value_name = "ISSUE")]
    pub issue_id: Option<String>,
    /// New title
    #[arg(long, short)]
    pub title: Option<String>,
    /// New description, in Markdown
    #[arg(long, short)]
    pub description: Option<String>,
    /// Read the new description from a Markdown file
    #[arg(long, value_name = "FILE")]
    pub description_file: Option<String>,
    /// Move the issue to this team (key, name, or ID)
    #[arg(long)]
    pub team: Option<String>,
    /// Assignee: a username, email, name, or self
    #[arg(long, short, value_name = "USER")]
    pub assignee: Option<String>,
    /// Remove the assignee
    #[arg(long)]
    pub unassign: bool,
    /// Workflow state, by name or type
    #[arg(long, short)]
    pub state: Option<String>,
    /// Priority, by name or number (0 none, 1 urgent to 4 low)
    #[arg(long, short, ignore_case = true)]
    pub priority: Option<super::values::Priority>,
    /// Estimate, in points
    #[arg(long, value_name = "POINTS", value_parser = super::values::estimate, allow_negative_numbers = true)]
    pub estimate: Option<i32>,
    /// Remove the estimate
    #[arg(long)]
    pub clear_estimate: bool,
    /// Set the labels, replacing the current ones; repeatable
    #[arg(long, short)]
    pub label: Vec<String>,
    /// Add a label, keeping the others; repeatable
    #[arg(long, value_name = "LABEL")]
    pub add_label: Vec<String>,
    /// Remove a label, keeping the others; repeatable
    #[arg(long, value_name = "LABEL")]
    pub remove_label: Vec<String>,
    /// Due date (YYYY-MM-DD)
    #[arg(long, value_name = "DATE", value_parser = super::values::date)]
    pub due_date: Option<NaiveDate>,
    /// Remove the due date
    #[arg(long)]
    pub clear_due_date: bool,
    /// Parent issue, like ENG-123
    #[arg(long, value_name = "ISSUE")]
    pub parent: Option<String>,
    /// Remove the parent
    #[arg(long)]
    pub clear_parent: bool,
    /// Project (ID, slug, or name)
    #[arg(long)]
    pub project: Option<String>,
    /// Remove the issue from its project
    #[arg(long)]
    pub clear_project: bool,
    /// Project milestone (ID, or name within --project or the issue's project)
    #[arg(long)]
    pub milestone: Option<String>,
    /// Remove the issue from its milestone
    #[arg(long)]
    pub clear_milestone: bool,
    /// Cycle: a name, number, `active`, `next`, `previous`, or an offset like
    /// +1 (write --cycle=-1 for a negative offset)
    #[arg(long)]
    pub cycle: Option<String>,
    /// Remove the issue from its cycle
    #[arg(long)]
    pub clear_cycle: bool,
}

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct IssueComment {
    #[command(subcommand)]
    pub command: IssueCommentCommand,
}

#[derive(Debug, Subcommand)]
pub enum IssueCommentCommand {
    /// Comment on an issue, or reply to a comment
    ///
    /// Images uploaded with --attach render inline.
    #[command(after_long_help = LINEAR_MARKDOWN)]
    Add(IssueCommentAdd),
    /// List an issue's comments
    List(IssueCommentList),
    /// Edit a comment
    #[command(after_long_help = LINEAR_MARKDOWN)]
    Update(IssueCommentUpdate),
    /// Delete a comment
    Delete(IssueCommentDelete),
}

#[derive(Debug, Args)]
pub struct IssueCommentAdd {
    /// Issue ID like ENG-123, or a URL; defaults to the current branch's issue
    #[arg(value_name = "ISSUE")]
    pub issue_id: Option<String>,
    /// Comment text, in Markdown
    #[arg(long, short, value_name = "TEXT", value_parser = NonEmptyStringValueParser::new())]
    pub body: Option<String>,
    /// Read the comment from a Markdown file
    #[arg(long, value_name = "FILE", value_parser = NonEmptyStringValueParser::new())]
    pub body_file: Option<String>,
    /// Reply to this top-level comment (by ID)
    #[arg(long, short, visible_alias = "reply-to", value_name = "COMMENT", value_parser = NonEmptyStringValueParser::new())]
    pub parent: Option<String>,
    /// ID for the new comment (a UUID you choose)
    #[arg(long, hide = true, value_name = "UUID", value_parser = NonEmptyStringValueParser::new())]
    pub id: Option<String>,
    /// Upload a file and link it in the comment (images render inline); repeatable
    #[arg(long, short, value_name = "FILE", value_parser = NonEmptyStringValueParser::new())]
    pub attach: Vec<String>,
    /// Make uploaded files public instead of visible to workspace members only
    #[arg(long)]
    pub public: bool,
}

#[derive(Debug, Args)]
pub struct IssueCommentDelete {
    /// Comment ID
    #[arg(value_name = "COMMENT")]
    pub comment_id: String,
}

#[derive(Debug, Args)]
pub struct IssueCommentUpdate {
    /// Comment ID
    #[arg(value_name = "COMMENT")]
    pub comment_id: String,
    /// New text, in Markdown
    #[arg(long, short, value_name = "TEXT", value_parser = NonEmptyStringValueParser::new())]
    pub body: Option<String>,
    /// Read the new text from a Markdown file
    #[arg(long, value_name = "FILE", value_parser = NonEmptyStringValueParser::new())]
    pub body_file: Option<String>,
}

#[derive(Debug, Args)]
pub struct IssueCommentList {
    /// Issue ID like ENG-123, or a URL; defaults to the current branch's issue
    #[arg(value_name = "ISSUE")]
    pub issue_id: Option<String>,
    /// Maximum number of comments to show (a number or `all`)
    #[arg(long, value_parser = super::limit::parse, default_value = "all")]
    pub limit: super::Limit,
    /// Print JSON
    #[arg(long, short)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct IssueAttach {
    /// Issue ID like ENG-123, or a URL
    #[arg(value_name = "ISSUE")]
    pub issue_id: String,
    /// File to upload
    #[arg(value_name = "FILE")]
    pub filepath: String,
    /// Attachment title [default: the file name]
    #[arg(long, short, value_parser = NonEmptyStringValueParser::new())]
    pub title: Option<String>,
    /// Also add a comment with this text, linked to the attachment
    #[arg(long, short, value_name = "TEXT", value_parser = NonEmptyStringValueParser::new())]
    pub comment: Option<String>,
    /// Make the upload public instead of visible to workspace members only
    #[arg(long)]
    pub public: bool,
}

#[derive(Debug, Args)]
pub struct IssueLink {
    /// Issue ID like ENG-123; or, alone, the URL to link to the current branch's issue
    #[arg(value_name = "ISSUE|URL")]
    pub url_or_issue_id: String,
    /// URL to link, when the issue is given first
    pub url: Option<String>,
    /// Link title
    #[arg(long, short, value_parser = NonEmptyStringValueParser::new())]
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
    /// Relate two issues
    Add(IssueRelationAdd),
    /// Remove a relation between two issues
    Delete(IssueRelationDelete),
    /// List an issue's relations
    List(IssueRelationList),
}

/// How one issue relates to another.
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
    /// Issue ID like ENG-123, or a URL
    #[arg(value_name = "ISSUE")]
    pub issue_id: String,
    /// How ISSUE relates to RELATED
    #[arg(value_name = "RELATION", ignore_case = true)]
    pub relation_type: RelationType,
    /// The other issue
    #[arg(value_name = "RELATED")]
    pub related_issue_id: String,
}

#[derive(Debug, Args)]
pub struct IssueRelationDelete {
    /// Issue ID like ENG-123, or a URL
    #[arg(value_name = "ISSUE")]
    pub issue_id: String,
    /// How ISSUE relates to RELATED
    #[arg(value_name = "RELATION", ignore_case = true)]
    pub relation_type: RelationType,
    /// The other issue
    #[arg(value_name = "RELATED")]
    pub related_issue_id: String,
}

#[derive(Debug, Args)]
pub struct IssueRelationList {
    /// Issue ID like ENG-123, or a URL; defaults to the current branch's issue
    #[arg(value_name = "ISSUE")]
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
    /// List an issue's agent sessions
    List(IssueAgentSessionList),
    /// Show an agent session and its activity
    #[command(visible_alias = "v")]
    View(IssueAgentSessionView),
}

#[derive(Debug, Args)]
pub struct IssueAgentSessionList {
    /// Issue ID like ENG-123, or a URL; defaults to the current branch's issue
    #[arg(value_name = "ISSUE")]
    pub issue_id: Option<String>,
    /// Maximum number of sessions to show (a number or `all`)
    #[arg(long, value_parser = super::limit::parse, default_value = "all")]
    pub limit: super::Limit,
    /// Print JSON
    #[arg(long, short)]
    pub json: bool,
    /// Show only sessions with this status
    #[arg(long)]
    pub status: Option<super::AgentSessionStatus>,
}

#[derive(Debug, Args)]
pub struct IssueAgentSessionView {
    /// Agent session ID
    #[arg(value_name = "SESSION")]
    pub session_id: String,
    /// Print JSON
    #[arg(long, short)]
    pub json: bool,
}
