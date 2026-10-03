use clap::builder::NonEmptyStringValueParser;
use clap::{Args, Subcommand};

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct Auth {
    #[command(subcommand)]
    pub command: AuthCommand,
}

#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    /// Log in to a workspace with an API key
    Login(AuthLogin),
    /// Remove a workspace's credential
    Logout(AuthLogout),
    /// List the workspaces you are logged in to
    List(AuthList),
    /// Set the workspace used when none is named
    Default(AuthDefault),
    /// Print the API key in use
    Token(AuthToken),
    /// Show who you are logged in as
    Whoami(AuthWhoami),
    /// Move API keys from the credentials file to the system keyring
    Migrate(AuthMigrate),
}

#[derive(Debug, Args)]
pub struct AuthLogin {
    /// API key; asked for, or read from stdin when it is piped
    #[arg(long, short, value_parser = NonEmptyStringValueParser::new())]
    pub key: Option<String>,
    /// Store the API key in the credentials file instead of the system keyring
    #[arg(long)]
    pub plaintext: bool,
}

#[derive(Debug, Args)]
pub struct AuthLogout {
    /// Workspace to log out of; asked for when several are stored
    #[arg(value_name = "WORKSPACE")]
    pub workspace_name: Option<String>,
    /// Do not ask for confirmation
    #[arg(long, short)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct AuthList {}

#[derive(Debug, Args)]
pub struct AuthDefault {
    /// Workspace to make the default; asked for when omitted
    #[arg(value_name = "WORKSPACE")]
    pub workspace_name: Option<String>,
}

#[derive(Debug, Args)]
pub struct AuthToken {}

#[derive(Debug, Args)]
pub struct AuthWhoami {}

#[derive(Debug, Args)]
pub struct AuthMigrate {}
