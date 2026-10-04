//! The command-line grammar. Each command owns its typed arguments; values are
//! parsed here, at the boundary, so commands receive checked types.
use std::path::PathBuf;

use clap::{Args, Command, CommandFactory, Parser, Subcommand, ValueEnum, ValueHint};

use values::NonBlank;

pub(crate) use limit::Limit;
pub(crate) mod api;
pub(crate) mod auth;
pub(crate) mod completions;
pub(crate) mod config;
pub(crate) mod cycle;
pub(crate) mod document;
pub(crate) mod initiative;
pub(crate) mod initiative_update;
pub(crate) mod issue;
pub(crate) mod label;
mod limit;
pub(crate) mod markdown;
pub(crate) mod milestone;
pub(crate) mod project;
pub(crate) mod project_update;
pub(crate) mod schema;
pub(crate) mod team;
pub(crate) mod template;
pub(crate) mod user;
pub(crate) mod values;

#[cfg(test)]
mod tests;

/// Shown under `--help` for commands that take Markdown text.
const LINEAR_MARKDOWN: &str = "\
Linear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,
and `[Name](url)` do not. Get a person's URL from the `url` field of
`linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.
Run `linear markdown` for collapsible sections and the full reference.";

const ENVIRONMENT: &str = "\
Environment:
  LINEAR_API_KEY            API key to use instead of a stored credential
  LINEAR_TEAM_ID            Default team key
  LINEAR_DEBUG=1            Show the details and causes of errors
  LINEAR_IGNORE_ENV_FILE=1  Do not load .env files

Every .linear.toml setting can also be set with a LINEAR_* variable; run
`linear config` to write one for the current repository.";

/// Work with Linear from the command line
#[derive(Debug, Parser)]
#[command(
    name = "linear",
    version,
    arg_required_else_help = true,
    max_term_width = 100,
    after_long_help = ENVIRONMENT,
)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalArgs,
    #[command(subcommand)]
    pub command: RootCommand,
}

/// Options every command accepts.
#[derive(Debug, Args)]
#[command(next_help_heading = "Global options")]
pub struct GlobalArgs {
    /// Workspace to use, by the name its credential is stored under
    #[arg(
        long,
        global = true,
        value_name = "SLUG",
        value_parser = NonBlank,
        display_order = 1000
    )]
    pub workspace: Option<String>,
    /// Never prompt; fail instead when a required value is missing
    #[arg(long, global = true, alias = "no-interactive", display_order = 1001)]
    pub no_input: bool,
}

#[derive(Debug, Subcommand)]
pub enum RootCommand {
    /// Manage issues
    #[command(alias = "i")]
    Issue(issue::Issue),
    /// Manage projects
    #[command(alias = "p")]
    Project(project::Project),
    /// Post and list project status updates
    #[command(alias = "pu")]
    ProjectUpdate(project_update::ProjectUpdate),
    /// Manage project milestones
    #[command(alias = "m")]
    Milestone(milestone::Milestone),
    /// View team cycles
    #[command(alias = "cy")]
    Cycle(cycle::Cycle),
    /// Manage initiatives
    #[command(alias = "init")]
    Initiative(initiative::Initiative),
    /// Post and list initiative status updates
    #[command(alias = "iu")]
    InitiativeUpdate(initiative_update::InitiativeUpdate),
    /// Manage documents
    #[command(aliases = ["docs", "doc"])]
    Document(document::Document),
    /// Manage issue labels
    #[command(alias = "l")]
    Label(label::Label),
    /// Browse issue, project and document templates
    ///
    /// Apply one with `linear issue create --template` or
    /// `linear project create --template`.
    Template(template::Template),
    /// Manage teams
    #[command(alias = "t")]
    Team(team::Team),
    /// List workspace members
    #[command(alias = "u")]
    User(user::User),
    /// Log in to workspaces and manage their credentials
    Auth(auth::Auth),
    /// Write a .linear.toml for the current repository
    ///
    /// Asks for the settings the flags leave out.
    #[command(alias = "configure")]
    Config(config::Config),
    /// Send a raw GraphQL request to the Linear API
    ///
    /// Pass the GraphQL document as one quoted argument or on stdin. A leading
    /// `query` or `mutation` keyword belongs inside that document.
    Api(api::Api),
    /// Print the Linear GraphQL schema
    Schema(schema::Schema),
    /// Print shell completions
    ///
    /// Load them from your shell's startup file:
    ///
    ///   bash (~/.bashrc):                   source <(linear completions bash)
    ///   zsh (~/.zshrc):                     source <(linear completions zsh)
    ///   fish (~/.config/fish/config.fish):  linear completions fish | source
    ///   elvish (rc.elv):                    eval (linear completions elvish | slurp)
    ///   powershell ($PROFILE):              linear completions powershell | Out-String | Invoke-Expression
    ///
    /// The script asks linear for candidates as you type, so load it at shell
    /// startup rather than saving it to a file.
    #[command(verbatim_doc_comment)]
    Completions(completions::Completions),
    /// Explain Linear-flavored Markdown: mentions and collapsible sections
    #[command(long_about = include_str!("markdown.txt").trim_end())]
    Markdown(markdown::Markdown),
}

/// Skips the confirmation prompt of a destructive command.
#[derive(Debug, Args)]
pub struct ConfirmArgs {
    /// Do not ask for confirmation
    #[arg(long, short = 'y', aliases = ["force", "confirm"], short_alias = 'f')]
    pub yes: bool,
}

/// [`ConfirmArgs`] without the `-f`/`--force` spellings, for commands where
/// those mean something else.
#[derive(Debug, Args)]
pub struct YesArgs {
    /// Do not ask for confirmation
    #[arg(long, short = 'y', alias = "confirm")]
    pub yes: bool,
}

/// Several targets at once instead of the positional argument.
#[derive(Debug, Args)]
pub struct BulkArgs {
    /// Act on several at once instead of one
    #[arg(long, value_name = "IDS", num_args = 0.., value_parser = NonBlank)]
    pub bulk: Option<Vec<String>>,
    /// Read the IDs from a file, one per line
    #[arg(long, value_name = "FILE", value_hint = ValueHint::FilePath)]
    pub bulk_file: Option<PathBuf>,
    /// Read the IDs from stdin, one per line
    #[arg(long)]
    pub bulk_stdin: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum AgentSessionStatus {
    Pending,
    Active,
    Complete,
    #[value(alias = "awaitingInput")]
    AwaitingInput,
    Error,
    Stale,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum TemplateType {
    Issue,
    Project,
    Document,
}

pub fn command() -> Command {
    Cli::command()
}

/// Parses the process arguments, exiting with a usage error (or help) when
/// they do not parse.
pub fn parse() -> Cli {
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    Cli::try_parse_from(&args).unwrap_or_else(|error| {
        let command = command();
        suggest_names(error, &command, &args).exit()
    })
}

/// An unknown subcommand's "similar subcommand" tip names only real
/// subcommands: clap also matches the short aliases, which make poor
/// suggestions (`lst` is not a typo of `l`).
fn suggest_names(
    mut error: clap::Error,
    root: &Command,
    args: &[std::ffi::OsString],
) -> clap::Error {
    use clap::error::{ContextKind, ContextValue};

    let Some(ContextValue::Strings(suggested)) = error.get(ContextKind::SuggestedSubcommand) else {
        return error;
    };
    let parent = args
        .iter()
        .skip(1)
        .filter_map(|arg| arg.to_str())
        .fold(root, |command, arg| {
            command.find_subcommand(arg).unwrap_or(command)
        });
    let names: Vec<String> = suggested
        .iter()
        .filter(|name| {
            parent
                .get_subcommands()
                .any(|subcommand| subcommand.get_name() == name.as_str())
        })
        .cloned()
        .collect();
    if names.is_empty() {
        error.remove(ContextKind::SuggestedSubcommand);
    } else {
        error.insert(
            ContextKind::SuggestedSubcommand,
            ContextValue::Strings(names),
        );
    }
    error
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
