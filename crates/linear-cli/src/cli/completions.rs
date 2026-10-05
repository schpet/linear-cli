use clap::Args;
use clap_complete::Shell;

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct Completions {
    /// Shell to print completions for
    pub shell: Shell,
    /// Command name to register the completions for, if not `linear`
    #[arg(long, short, value_parser = command_name)]
    pub name: Option<String>,
}

/// A program name that is safe to embed in every shell's completion script.
fn command_name(name: &str) -> Result<String, String> {
    let mut chars = name.chars();
    let valid = chars
        .next()
        .is_some_and(|first| first.is_ascii_alphanumeric() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
    if valid {
        Ok(name.to_owned())
    } else {
        Err(
            "use ASCII letters, digits, '_', '-' or '.', starting with a letter, digit or '_'"
                .to_owned(),
        )
    }
}
