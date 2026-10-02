use clap::{Args, Subcommand};

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct Auth {
    #[command(subcommand)]
    pub command: AuthCommand,
}

#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    #[command(name = "login", about = "Add a workspace credential")]
    Login(AuthLogin),
    #[command(name = "logout", about = "Remove a workspace credential")]
    Logout(AuthLogout),
    #[command(name = "list", about = "List configured workspaces")]
    List(AuthList),
    #[command(name = "default", about = "Set the default workspace")]
    Default(AuthDefault),
    #[command(name = "token", about = "Print the configured API token")]
    Token(AuthToken),
    #[command(
        name = "whoami",
        about = "Print information about the authenticated user"
    )]
    Whoami(AuthWhoami),
    #[command(
        name = "migrate",
        about = "Migrate plaintext credentials to system keyring"
    )]
    Migrate(AuthMigrate),
}

#[derive(Debug, Args)]
pub struct AuthLogin {
    #[arg(long = "key", short = 'k', help = "API key (prompted if not provided)", value_name = "key", value_parser = super::nonempty_string)]
    pub key: Option<String>,
    #[arg(
        long = "plaintext",
        help = "Store API key in credentials file instead of system keyring"
    )]
    pub plaintext: bool,
}

#[derive(Debug, Args)]
pub struct AuthLogout {
    #[arg(value_name = "workspace")]
    pub workspace_name: Option<String>,
    #[arg(long = "force", short = 'f', help = "Skip confirmation prompt")]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct AuthList {}

#[derive(Debug, Args)]
pub struct AuthDefault {
    #[arg(value_name = "workspace")]
    pub workspace_name: Option<String>,
}

#[derive(Debug, Args)]
pub struct AuthToken {}

#[derive(Debug, Args)]
pub struct AuthWhoami {}

#[derive(Debug, Args)]
pub struct AuthMigrate {}
