//! Native clap grammar. Each command owns its typed arguments.
use clap::{Command, CommandFactory, Parser, Subcommand, ValueEnum};
pub mod api;
pub mod auth;
pub mod completions;
pub mod config;
pub mod cycle;
pub mod document;
pub mod fish_completion;
pub mod initiative;
pub mod initiative_update;
pub mod issue;
pub mod label;
pub mod markdown;
pub mod milestone;
mod numeric;
pub mod project;
pub mod project_update;
pub mod schema;
pub mod team;
pub mod template;
pub mod user;

#[derive(Debug, Parser)]
#[command(
    name = "linear",
    version,
    about = "Handy linear commands from the command line.",
    long_about = "Handy linear commands from the command line.\n\nEnvironment Variables:\n  LINEAR_DEBUG=1              Show full error details including stack traces\n  LINEAR_IGNORE_ENV_FILE=1    Skip loading .env files"
)]
pub struct Cli {
    #[arg(long, global = true, value_name = "slug", value_parser = nonempty_string, help = "Target workspace (uses credentials)")]
    pub workspace: Option<String>,
    #[command(subcommand)]
    pub command: Option<RootCommand>,
}

#[derive(Debug, Subcommand)]
pub enum RootCommand {
    #[command(name = "auth", about = "Manage Linear authentication")]
    Auth(auth::Auth),
    #[command(name = "issue", about = "Manage Linear issues", visible_aliases = ["i"])]
    Issue(issue::Issue),
    #[command(name = "team", about = "Manage Linear teams", visible_aliases = ["t"])]
    Team(team::Team),
    #[command(name = "user", about = "Manage Linear users", visible_aliases = ["u"])]
    User(user::User),
    #[command(name = "project", about = "Manage Linear projects", visible_aliases = ["p"])]
    Project(project::Project),
    #[command(name = "project-update", about = "Manage project status updates", visible_aliases = ["pu"])]
    ProjectUpdate(project_update::ProjectUpdate),
    #[command(name = "cycle", about = "Manage Linear team cycles", visible_aliases = ["cy"])]
    Cycle(cycle::Cycle),
    #[command(name = "milestone", about = "Manage Linear project milestones", visible_aliases = ["m"])]
    Milestone(milestone::Milestone),
    #[command(name = "initiative", about = "Manage Linear initiatives", visible_aliases = ["init"])]
    Initiative(initiative::Initiative),
    #[command(name = "initiative-update", about = "Manage initiative status updates (timeline posts)", visible_aliases = ["iu"])]
    InitiativeUpdate(initiative_update::InitiativeUpdate),
    #[command(name = "label", about = "Manage Linear issue labels", visible_aliases = ["l"])]
    Label(label::Label),
    #[command(
        name = "template",
        about = "Browse Linear issue, project, and document templates. Apply one with `issue create --template` or `project create --template`."
    )]
    Template(template::Template),
    #[command(name = "document", about = "Manage Linear documents", visible_aliases = ["docs", "doc"])]
    Document(document::Document),
    #[command(
        name = "completions",
        about = "Generate shell completions.",
        long_about = "Generate shell completions.\n\nTo enable shell completions for this program add the following line to your ~/.bashrc or similar:\n\n    source <(linear completions [shell])\n\n    For more information run linear completions [shell] --help\n"
    )]
    Completions(completions::Completions),
    #[command(name = "config", about = "Interactively generate .linear.toml configuration", visible_aliases = ["configure"])]
    Config(config::Config),
    #[command(name = "schema", about = "Print the GraphQL schema to stdout")]
    Schema(schema::Schema),
    #[command(
        name = "api",
        about = "Make a raw GraphQL API request",
        long_about = "Make a raw GraphQL API request\n\nPass the GraphQL document as one quoted argument or on stdin. The api command has no subcommands: a leading query or mutation keyword belongs inside that document."
    )]
    Api(api::Api),
    #[command(
        name = "markdown",
        about = "Linear-flavored Markdown: mentions and collapsible sections",
        long_about = "Linear-flavored Markdown: mentions and collapsible sections\n\nThese rules apply to comment bodies, issue descriptions, document content,\nproject overviews, and status update bodies.\n\nMENTIONS\n\nA resource's plain Linear URL becomes a linked mention. A literal `@name`, an\n`@[Name](id)`, or a Markdown link such as `[Name](url)` does not — it stays\nplain text and notifies nobody. Put the bare URL in the body:\n\nhttps://linear.app/acme/profiles/someuser can you take a look?\n\nRESOLVING PEOPLE\n\nLook the person up in the relevant team first. The team can usually be\ninferred from the issue identifier or the current directory:\n\nlinear team members ENG --json\n\nPaste the selected member's `url` field verbatim. If the intended person is\nnot a member of that team, stop and confirm before searching the whole\nworkspace with `linear user list --json`; mentioning someone outside the team\nis likely accidental.\n\nTo mention an issue, use its URL the same way:\n\nlinear issue url ENG-123\n\nCOLLAPSIBLE SECTIONS\n\nOpen a section with `+++ [title]` and close it with `+++`:\n\n+++ [Server log]\n\nMarkdown content that is initially hidden.\n\n+++\n\nThe square brackets around the title and the closing `+++` are both required."
    )]
    Markdown(markdown::Markdown),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum Sort {
    #[value(name = "manual")]
    Manual,
    #[value(name = "priority")]
    Priority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum AgentSessionStatus {
    #[value(name = "pending")]
    Pending,
    #[value(name = "active")]
    Active,
    #[value(name = "complete")]
    Complete,
    #[value(name = "awaitingInput")]
    AwaitingInput,
    #[value(name = "error")]
    Error,
    #[value(name = "stale")]
    Stale,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum TemplateType {
    #[value(name = "issue")]
    Issue,
    #[value(name = "project")]
    Project,
    #[value(name = "document")]
    Document,
}

pub fn command() -> Command {
    Cli::command()
}

pub fn parse(args: &[std::ffi::OsString]) -> Result<Cli, crate::error::AppError> {
    Cli::try_parse_from(
        std::iter::once(std::ffi::OsString::from("linear")).chain(args.iter().cloned()),
    )
    .map_err(crate::error::AppError::native_parser)
}

pub(crate) fn nonempty_string(value: &str) -> Result<String, String> {
    if value.is_empty() {
        Err("expected a nonempty value".to_owned())
    } else {
        Ok(value.to_owned())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariableAssignment {
    pub key: String,
    pub value: String,
}

pub(crate) fn variable_assignment(value: &str) -> Result<VariableAssignment, String> {
    let (key, value) = value
        .split_once('=')
        .ok_or_else(|| format!("expected key=value, got {value:?}"))?;
    Ok(VariableAssignment {
        key: key.to_owned(),
        value: value.to_owned(),
    })
}
