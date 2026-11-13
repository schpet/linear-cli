use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;

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
    ///
    /// An agent with the skill installed reads the guide through the skill. An
    /// agent without it can run this command to read the same guide.
    // Installed skills run exactly `linear skill show`. Keep it working with
    // no arguments for as long as any of them may be installed.
    Show(SkillShow),
    /// Install linear's skill into coding agents
    ///
    /// Installs the skill for Claude Code, Codex, OpenCode and Pi. Codex,
    /// OpenCode and Pi all read the shared ~/.agents/skills directory, which
    /// is always written. Claude Code has its own directory, which is written
    /// when Claude Code is set up on this machine. Use --agent to choose.
    ///
    /// Run this once per account. Running it again is safe, and updates a
    /// skill an older linear installed.
    ///
    /// A skill named linear-cli that this command did not install is left
    /// alone and reported as a conflict, and so is an older copy in a
    /// directory that would be read instead of the new one. --force moves
    /// them to a backup directory beside the skills directory, never deleting
    /// anything, and installs the skill.
    Setup(SkillSetup),
    /// Report where linear's skill is installed
    ///
    /// Lists each directory an agent reads the skill from and what is there.
    /// Nothing is changed.
    ///
    ///   current     installed and up to date
    ///   outdated    installed by an older linear
    ///   missing     not installed
    ///   foreign     a linear-cli skill that `linear skill setup` did not
    ///               install, or one edited since
    ///   unreadable  could not be read
    ///
    /// A current skill reads its guide from the linear on PATH each time, so
    /// it does not go out of date when linear is upgraded.
    ///
    /// Only the directories agents read by default are checked. Plugins and
    /// directories added in an agent's own configuration are not.
    #[command(verbatim_doc_comment)]
    Status(SkillStatus),
    /// Remove linear's skill from coding agents
    ///
    /// Removes the skill from every directory `linear skill setup` installs
    /// into, or from those of the agents named with --agent. Only the file
    /// setup installed is removed. A linear-cli skill it did not install, and
    /// any other file beside the skill, is left as it was.
    Remove(SkillRemove),
}

#[derive(Debug, Args)]
pub struct SkillShow {}

#[derive(Debug, Args)]
pub struct SkillSetup {
    /// Use only AGENT (repeatable)
    #[arg(long, value_name = "AGENT", value_enum)]
    pub agent: Vec<Agent>,
    /// Set up this repository instead of your account
    ///
    /// Writes .agents/skills and .claude/skills at the root of the
    /// repository, or in the current directory outside one. Commit them to
    /// share the skill with everyone who works here.
    #[arg(long)]
    pub project: bool,
    /// Report what would change without changing anything
    #[arg(long)]
    pub dry_run: bool,
    /// Move a conflicting linear-cli skill to a backup and install over it
    #[arg(long)]
    pub force: bool,
    /// Print the result as JSON
    #[arg(long, short = 'j')]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct SkillStatus {
    /// Report on this repository instead of your account
    #[arg(long)]
    pub project: bool,
    /// Print the report as JSON
    #[arg(long, short = 'j')]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct SkillRemove {
    /// Use only AGENT (repeatable)
    #[arg(long, value_name = "AGENT", value_enum)]
    pub agent: Vec<Agent>,
    /// Remove from this repository instead of your account
    #[arg(long)]
    pub project: bool,
    /// Report what would change without changing anything
    #[arg(long)]
    pub dry_run: bool,
    /// Print the result as JSON
    #[arg(long, short = 'j')]
    pub json: bool,
}

/// A coding agent that loads skills.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Agent {
    Claude,
    Codex,
    Opencode,
    Pi,
}

impl Agent {
    pub const ALL: [Self; 4] = [Self::Claude, Self::Codex, Self::Opencode, Self::Pi];

    /// The agent's name as its own documentation writes it.
    pub fn title(self) -> &'static str {
        match self {
            Self::Claude => "Claude Code",
            Self::Codex => "Codex",
            Self::Opencode => "OpenCode",
            Self::Pi => "Pi",
        }
    }
}
