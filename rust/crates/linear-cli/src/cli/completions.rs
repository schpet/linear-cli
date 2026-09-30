use clap::{Args, Subcommand};

#[derive(Debug, Args)]
pub struct Completions {
    #[command(subcommand)]
    pub command: Option<CompletionsCommand>,
}

#[derive(Debug, Subcommand)]
pub enum CompletionsCommand {
    #[command(
        name = "bash",
        about = "Generate shell completions for bash.",
        long_about = "Generate shell completions for bash.\n\nTo enable bash completions for this program add following line to your ~/.bashrc:\n\n    source <(linear completions bash)"
    )]
    Bash(CompletionsBash),
    #[command(
        name = "fish",
        about = "Generate shell completions for fish.",
        long_about = "Generate shell completions for fish.\n\nTo enable fish completions for this program add following line to your ~/.config/fish/config.fish:\n\n    source (linear completions fish | psub)"
    )]
    Fish(CompletionsFish),
    #[command(
        name = "zsh",
        about = "Generate shell completions for zsh.",
        long_about = "Generate shell completions for zsh.\n\nTo enable zsh completions for this program add following line to your ~/.zshrc:\n\n    source <(linear completions zsh)"
    )]
    Zsh(CompletionsZsh),
    #[command(
        name = "complete",
        about = "Get completions for given action from given command.",
        hide = true
    )]
    Complete(CompletionsComplete),
}

#[derive(Debug, Args)]
pub struct CompletionsBash {
    #[arg(long = "name", short = 'n', help = "The name of the main command.", value_name = "command-name", value_parser = super::nonempty_string)]
    pub name: Option<String>,
}

#[derive(Debug, Args)]
pub struct CompletionsFish {
    #[arg(long = "name", short = 'n', help = "The name of the main command.", value_name = "command-name", value_parser = super::nonempty_string)]
    pub name: Option<String>,
}

#[derive(Debug, Args)]
pub struct CompletionsZsh {
    #[arg(long = "name", short = 'n', help = "The name of the main command.", value_name = "command-name", value_parser = super::nonempty_string)]
    pub name: Option<String>,
}

#[derive(Debug, Args)]
pub struct CompletionsComplete {
    #[arg(value_name = "action")]
    pub action: String,
    #[arg(value_name = "command")]
    pub command: Vec<String>,
    /// Literal words from saved scripts are not command-path words.
    #[arg(last = true, value_name = "literal")]
    pub literals: Vec<String>,
}
