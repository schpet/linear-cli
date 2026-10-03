//! Native clap grammar. Each command owns its typed arguments.
use clap::{Command, CommandFactory, Parser, Subcommand, ValueEnum};
pub mod api;
pub mod auth;
pub mod completions;
pub mod config;
pub mod cycle;
pub mod document;
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
pub mod values;

#[derive(Debug, Parser)]
#[command(
    name = "linear",
    version,
    arg_required_else_help = true,
    about = "Handy linear commands from the command line.",
    long_about = "Handy linear commands from the command line.\n\nEnvironment Variables:\n  LINEAR_DEBUG=1              Show full error details including stack traces\n  LINEAR_IGNORE_ENV_FILE=1    Skip loading .env files"
)]
pub struct Cli {
    #[arg(long, global = true, value_name = "slug", value_parser = nonempty_string, help = "Target workspace (uses credentials)")]
    pub workspace: Option<String>,
    #[command(subcommand)]
    pub command: RootCommand,
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
        long_about = "Generate shell completions.\n\nLoad them from your shell's startup file:\n\n  bash (~/.bashrc):                   source <(linear completions bash)\n  zsh (~/.zshrc):                     source <(linear completions zsh)\n  fish (~/.config/fish/config.fish):  linear completions fish | source\n  elvish (rc.elv):                    eval (linear completions elvish | slurp)\n  powershell ($PROFILE):              linear completions powershell | Out-String | Invoke-Expression"
    )]
    Completions(completions::Completions),
    #[command(name = "config", about = "Generate .linear.toml configuration, asking for what the flags leave out", visible_aliases = ["configure"])]
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
        long_about = include_str!("markdown.txt").trim_end()
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
