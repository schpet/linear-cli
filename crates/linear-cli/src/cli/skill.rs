use clap::{Args, Subcommand};

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct Skill {
    #[command(subcommand)]
    pub command: SkillCommand,
}

#[derive(Debug, Subcommand)]
pub enum SkillCommand {
    /// Print the guide that teaches an agent to use linear
    ///
    /// The guide is Markdown and comes from this binary, so it describes the
    /// commands this version has. It needs no login, configuration or network
    /// connection. Run `linear COMMAND --help` for a command's full options.
    // Installed skills run exactly `linear skill show`. Keep it working with
    // no arguments for as long as any of them may be installed.
    Show(SkillShow),
}

#[derive(Debug, Args)]
pub struct SkillShow {}
