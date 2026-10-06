use std::path::PathBuf;

use chrono::{DateTime, NaiveDate, Utc};
use clap::{Args, Subcommand, ValueEnum, ValueHint};

use super::LINEAR_MARKDOWN;
use super::values::{NonBlank, TextSource, UserRef};

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct Issue {
    #[command(subcommand)]
    pub command: IssueCommand,
}

#[derive(Debug, Subcommand)]
pub enum IssueCommand {
    /// List issues, assigned to you by default
    #[command(aliases = ["mine", "l"])]
    List(IssueList),
    /// Find issues by filters or full-text search
    #[command(alias = "q")]
    Query(IssueQuery),
    /// Show an issue
    #[command(alias = "v")]
    View(IssueView),
    /// Create an issue
    #[command(after_long_help = LINEAR_MARKDOWN)]
    Create(IssueCreate),
    /// Update an issue
    #[command(after_long_help = LINEAR_MARKDOWN)]
    Update(IssueUpdate),
    /// Delete an issue (moves it to the trash)
    #[command(alias = "d")]
    Delete(IssueDelete),
    /// Archive an issue
    ///
    /// Linear archives closed issues on its own, so prefer closing an issue
    /// (`issue update --state`) and letting auto-archive run, or `issue delete`
    /// to trash it. Archived issues drop out of list, query, and search results
    /// unless --include-archived is passed. See
    /// https://linear.app/docs/delete-archive-issues
    Archive(IssueArchive),
    /// Start an issue: check out its git branch or jj change, then mark it started
    ///
    /// With git, creates the issue's branch (or switches to it if it exists).
    /// With jj (`vcs = "jj"` in config or LINEAR_VCS=jj), starts a new change,
    /// reusing an empty undescribed one, and describes it with the issue title
    /// and `Linear-issue` trailers; no git branch is created.
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
    #[command(alias = "pr")]
    PullRequest(IssuePullRequest),
    /// Add, list, edit, and delete comments on an issue
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
pub struct IssueList {
    /// Show issues in this state: a type (triage, backlog, unstarted, started,
    /// completed, canceled), name, or ID; repeatable
    #[arg(long, short, value_parser = NonBlank, default_values = ["unstarted"])]
    pub state: Vec<String>,
    /// Show issues in every state
    #[arg(long, conflicts_with = "state")]
    pub all_states: bool,
    /// Sort order [default: the issue_sort setting, or priority]
    #[arg(long)]
    pub sort: Option<crate::config::IssueSort>,
    /// Show this team's issues (key, name, or ID); defaults to the configured team
    #[arg(long, value_parser = NonBlank)]
    pub team: Option<String>,
    #[command(flatten)]
    pub filters: IssueFilters,
    /// Open the list in the browser
    #[arg(long, short)]
    pub web: bool,
    /// Open the list in the Linear app
    #[arg(long, short)]
    pub app: bool,
    /// Do not page long output
    #[arg(long)]
    pub no_pager: bool,
    /// Print JSON
    #[arg(long, short, conflicts_with_all = ["web", "app"])]
    pub json: bool,
}

/// Filters `issue list` and `issue query` share.
#[derive(Debug, Args)]
pub struct IssueFilters {
    /// Show only issues assigned to this user: a username, email, name, or @me
    #[arg(long, value_name = "USER")]
    pub assignee: Option<UserRef>,
    /// Show issues of every assignee
    #[arg(long, short = 'A', conflicts_with_all = ["assignee", "unassigned"])]
    pub all_assignees: bool,
    /// Show only unassigned issues
    #[arg(long, short = 'U', conflicts_with = "assignee")]
    pub unassigned: bool,
    /// Show only this project's issues (ID, slug, or name)
    #[arg(long, value_parser = NonBlank)]
    pub project: Option<String>,
    /// Show only issues in projects with this project label
    #[arg(long, value_name = "LABEL", conflicts_with_all = ["project", "milestone"], value_parser = NonBlank)]
    pub project_label: Option<String>,
    /// Show only this cycle's issues: a name, number, `active`, `next`,
    /// `previous`, or an offset like +1 or -1
    #[arg(long, allow_negative_numbers = true, value_parser = NonBlank)]
    pub cycle: Option<String>,
    /// Show only this milestone's issues (ID, or name with --project)
    #[arg(long, value_parser = NonBlank)]
    pub milestone: Option<String>,
    /// Show only issues with this label; repeat to require several
    #[arg(long, short, value_parser = NonBlank)]
    pub label: Vec<String>,
    /// Show only issues created after this date (YYYY-MM-DD or RFC 3339)
    #[arg(long, value_name = "DATE", value_parser = super::values::date_or_datetime)]
    pub created_after: Option<DateTime<Utc>>,
    /// Show only issues updated after this date (YYYY-MM-DD or RFC 3339)
    #[arg(long, value_name = "DATE", value_parser = super::values::date_or_datetime)]
    pub updated_after: Option<DateTime<Utc>>,
    /// Maximum number of issues to show (a number or `all`)
    #[arg(long, value_parser = super::limit::parse, default_value = "50")]
    pub limit: super::Limit,
}

// Every assignee and every state are what `issue query` shows by default;
// the flags stay accepted but are not offered.
#[derive(Debug, Args)]
#[command(
    mut_arg("all_assignees", |arg| arg.hide(true)),
    mut_arg("all_states", |arg| arg.hide(true)),
)]
pub struct IssueQuery {
    /// Search issue titles and descriptions for this text
    #[arg(long, value_name = "TEXT", value_parser = NonBlank, conflicts_with = "milestone")]
    pub search: Option<String>,
    /// Also search comments (with --search)
    #[arg(long, requires = "search")]
    pub search_comments: bool,
    /// Show this team's issues (key, name, or ID); repeatable [default: the configured team]
    #[arg(long, value_parser = NonBlank)]
    pub team: Vec<String>,
    /// Show every team's issues
    #[arg(long, conflicts_with = "team")]
    pub all_teams: bool,
    /// Show issues in this state: a type (triage, backlog, unstarted, started,
    /// completed, canceled), name, or ID; repeatable
    #[arg(long, short, value_parser = NonBlank)]
    pub state: Vec<String>,
    #[arg(long, conflicts_with = "state")]
    pub all_states: bool,
    /// Sort order, except with --search [default: the issue_sort setting, or priority]
    #[arg(long, conflicts_with = "search")]
    pub sort: Option<crate::config::IssueSort>,
    #[command(flatten)]
    pub filters: IssueFilters,
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
    #[arg(value_name = "ISSUE", value_parser = NonBlank)]
    pub issue_id: Option<String>,
    /// Team to pick from, and the team of a bare issue number (key, name, or
    /// ID); defaults to the configured team
    #[arg(long, value_parser = NonBlank)]
    pub team: Option<String>,
    /// Offer issues of every assignee in the picker
    #[arg(long, short = 'A')]
    pub all_assignees: bool,
    /// Offer only unassigned issues in the picker
    #[arg(long, short = 'U', conflicts_with = "all_assignees")]
    pub unassigned: bool,
    /// Git ref to create the branch from (git only)
    #[arg(long, short, value_name = "REF", value_parser = NonBlank)]
    pub from_ref: Option<String>,
    /// Branch name to use instead of the issue's (git only)
    #[arg(long, short, value_parser = NonBlank)]
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
    #[arg(long, short = 'T', value_name = "FILE", value_hint = ValueHint::FilePath, value_parser = NonBlank)]
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
    #[arg(long, short, value_parser = NonBlank)]
    pub title: Option<String>,
    /// Issue description, in Markdown
    #[arg(long, short)]
    pub description: Option<String>,
    /// Read the description from a Markdown file (- for stdin)
    #[arg(long, value_name = "FILE", value_hint = ValueHint::FilePath)]
    pub description_file: Option<TextSource>,
    /// Team (key, name, or ID); defaults to the configured team
    #[arg(long, value_parser = NonBlank)]
    pub team: Option<String>,
    /// Assignee: a username, email, name, or @me
    #[arg(long, short, value_name = "USER")]
    pub assignee: Option<UserRef>,
    /// Delegate the issue to an agent: its username, email, name, or ID
    ///
    /// The delegate is an agent user working on the issue, separate from
    /// the assignee.
    #[arg(long, value_name = "AGENT")]
    pub delegate: Option<UserRef>,
    /// Workflow state, by name or type
    #[arg(long, short, value_parser = NonBlank)]
    pub state: Option<String>,
    /// Priority, by name or number (0 none, 1 urgent to 4 low)
    #[arg(long, short, ignore_case = true)]
    pub priority: Option<super::values::Priority>,
    /// Estimate, in points
    #[arg(long, value_name = "POINTS", value_parser = super::values::estimate, allow_negative_numbers = true)]
    pub estimate: Option<i32>,
    /// Label; repeat for several labels
    #[arg(long, short, value_parser = NonBlank)]
    pub label: Vec<String>,
    /// Due date (YYYY-MM-DD)
    #[arg(long, value_name = "DATE", value_parser = super::values::date)]
    pub due_date: Option<NaiveDate>,
    /// Parent issue, like ENG-123
    #[arg(long, value_name = "ISSUE", value_parser = NonBlank)]
    pub parent: Option<String>,
    /// Project (ID, slug, or name)
    #[arg(long, value_parser = NonBlank)]
    pub project: Option<String>,
    /// Project milestone (ID, or name with --project)
    #[arg(long, value_parser = NonBlank)]
    pub milestone: Option<String>,
    /// Cycle: a name, number, `active`, `next`, `previous`, or an offset like
    /// +1 or -1
    #[arg(long, allow_negative_numbers = true, value_parser = NonBlank)]
    pub cycle: Option<String>,
    /// Start from this issue template (name or ID) instead of the team's default
    ///
    /// The team's templates and workspace templates are searched. The template
    /// fills in anything you do not pass: flags override it, --label adds to
    /// its labels, and --description replaces its body. With a template,
    /// --title is optional.
    #[arg(long, value_parser = NonBlank)]
    pub template: Option<String>,
    /// Do not apply the team's default template
    #[arg(long)]
    pub no_use_default_template: bool,
    /// Start the issue after creating it
    #[arg(long)]
    pub start: bool,
    /// Ask for every field instead of taking them as flags
    ///
    /// Only --parent and --project can be combined with it.
    #[arg(long, short, conflicts_with_all = [
        "title", "description", "description_file", "team", "assignee", "delegate", "state", "priority",
        "estimate", "label", "due_date", "milestone", "cycle", "template", "start",
    ])]
    pub interactive: bool,
    #[command(flatten)]
    pub confirm: super::ConfirmArgs,
}

#[derive(Debug, Args)]
pub struct IssueUpdate {
    /// Issue ID like ENG-123, or a URL; defaults to the current branch's issue
    #[arg(value_name = "ISSUE")]
    pub issue_id: Option<String>,
    /// New title
    #[arg(long, short, value_parser = NonBlank)]
    pub title: Option<String>,
    /// New description, in Markdown
    #[arg(long, short)]
    pub description: Option<String>,
    /// Read the new description from a Markdown file (- for stdin)
    #[arg(long, value_name = "FILE", value_hint = ValueHint::FilePath)]
    pub description_file: Option<TextSource>,
    /// Move the issue to this team (key, name, or ID)
    #[arg(long, value_parser = NonBlank)]
    pub team: Option<String>,
    /// Assignee: a username, email, name, or @me
    #[arg(long, short, value_name = "USER")]
    pub assignee: Option<UserRef>,
    /// Remove the assignee
    #[arg(long)]
    pub unassign: bool,
    /// Delegate the issue to an agent: its username, email, name, or ID
    ///
    /// The delegate is an agent user working on the issue, separate from
    /// the assignee.
    #[arg(long, value_name = "AGENT")]
    pub delegate: Option<UserRef>,
    /// Remove the delegate
    #[arg(long, alias = "undelegate")]
    pub clear_delegate: bool,
    /// Workflow state, by name or type
    #[arg(long, short, value_parser = NonBlank)]
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
    #[arg(long, short, value_parser = NonBlank)]
    pub label: Vec<String>,
    /// Add a label, keeping the others; repeatable
    #[arg(long, value_name = "LABEL", value_parser = NonBlank)]
    pub add_label: Vec<String>,
    /// Remove a label, keeping the others; repeatable
    #[arg(long, value_name = "LABEL", value_parser = NonBlank)]
    pub remove_label: Vec<String>,
    /// Due date (YYYY-MM-DD)
    #[arg(long, value_name = "DATE", value_parser = super::values::date)]
    pub due_date: Option<NaiveDate>,
    /// Remove the due date
    #[arg(long)]
    pub clear_due_date: bool,
    /// Parent issue, like ENG-123
    #[arg(long, value_name = "ISSUE", value_parser = NonBlank)]
    pub parent: Option<String>,
    /// Remove the parent
    #[arg(long)]
    pub clear_parent: bool,
    /// Project (ID, slug, or name)
    #[arg(long, value_parser = NonBlank)]
    pub project: Option<String>,
    /// Remove the issue from its project
    #[arg(long)]
    pub clear_project: bool,
    /// Project milestone (ID, or name within --project or the issue's project)
    #[arg(long, value_parser = NonBlank)]
    pub milestone: Option<String>,
    /// Remove the issue from its milestone
    #[arg(long)]
    pub clear_milestone: bool,
    /// Cycle: a name, number, `active`, `next`, `previous`, or an offset like
    /// +1 or -1
    #[arg(long, allow_negative_numbers = true, value_parser = NonBlank)]
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
    #[arg(long, short, value_name = "TEXT", value_parser = NonBlank)]
    pub body: Option<String>,
    /// Read the comment from a Markdown file (- for stdin)
    #[arg(long, value_name = "FILE", value_hint = ValueHint::FilePath)]
    pub body_file: Option<TextSource>,
    /// Reply to this top-level comment (by ID)
    #[arg(long, short = 'p', visible_alias = "parent", value_name = "COMMENT", value_parser = NonBlank)]
    pub reply_to: Option<String>,
    #[command(flatten)]
    pub confirm: super::ConfirmArgs,
    /// ID for the new comment (a UUID you choose)
    #[arg(long, hide = true, value_name = "UUID", value_parser = NonBlank)]
    pub id: Option<String>,
    /// Upload a file and link it in the comment (images render inline); repeatable
    #[arg(long, short, value_name = "FILE", value_hint = ValueHint::FilePath)]
    pub attach: Vec<PathBuf>,
    /// Make uploaded files public instead of visible to workspace members only
    #[arg(long)]
    pub public: bool,
}

#[derive(Debug, Args)]
pub struct IssueCommentDelete {
    /// Comment ID
    #[arg(value_name = "COMMENT")]
    pub comment_id: String,
    #[command(flatten)]
    pub confirm: super::ConfirmArgs,
}

#[derive(Debug, Args)]
pub struct IssueCommentUpdate {
    /// Comment ID
    #[arg(value_name = "COMMENT")]
    pub comment_id: String,
    /// New text, in Markdown
    #[arg(long, short, value_name = "TEXT", value_parser = NonBlank)]
    pub body: Option<String>,
    /// Read the new text from a Markdown file (- for stdin)
    #[arg(long, value_name = "FILE", value_hint = ValueHint::FilePath)]
    pub body_file: Option<TextSource>,
    #[command(flatten)]
    pub confirm: super::ConfirmArgs,
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
    /// Do not page long output
    #[arg(long)]
    pub no_pager: bool,
}

#[derive(Debug, Args)]
pub struct IssueAttach {
    /// Issue ID like ENG-123, or a URL
    #[arg(value_name = "ISSUE")]
    pub issue_id: String,
    /// File to upload
    #[arg(value_name = "FILE", value_hint = ValueHint::FilePath)]
    pub filepath: PathBuf,
    /// Attachment title [default: the file name]
    #[arg(long, short, value_parser = NonBlank)]
    pub title: Option<String>,
    /// Also add a comment with this text, linked to the attachment
    #[arg(long, short, value_name = "TEXT", value_parser = NonBlank)]
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
    #[arg(long, short, value_parser = NonBlank)]
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
    #[command(flatten)]
    pub confirm: super::ConfirmArgs,
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
    #[command(alias = "v")]
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
    /// Do not page long output
    #[arg(long)]
    pub no_pager: bool,
}
