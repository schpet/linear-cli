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
    /// Print the startup reminder used by an agent's SessionStart hook
    #[command(hide = true)]
    SessionStart(SkillSessionStart),
    /// Print the guide that teaches an agent to use linear
    ///
    /// The guide is Markdown and comes from this binary, so it describes the
    /// commands this version has, with global and project template preferences.
    /// It needs no login or network. Use --builtin to bypass all configuration.
    /// Name a command path for its inherited guidance and full options.
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
    /// Also adds SessionStart reminders for Claude Code (settings.json) and
    /// Codex (hooks.json), preserving other settings and hooks. Codex's account
    /// hook follows CODEX_HOME and is installed by default when that directory
    /// exists, or explicitly with --agent codex. Review it in Codex's /hooks menu.
    /// These reminders point to the skill even when its catalog description is omitted.
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
    /// Also reports Claude Code and Codex SessionStart reminders.
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
    /// Also removes unedited Claude Code and Codex SessionStart reminders,
    /// preserving all other settings and hooks.
    Remove(SkillRemove),
}

#[derive(Debug, Args)]
pub struct SkillSessionStart {
    /// Agent receiving the startup reminder
    #[arg(long, value_enum, default_value = "claude")]
    pub agent: HookAgent,
}

/// Agents with a supported SessionStart hook integration.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum HookAgent {
    Claude,
    Codex,
}

impl HookAgent {
    pub const ALL: [Self; 2] = [Self::Claude, Self::Codex];

    pub fn agent(self) -> Agent {
        match self {
            Self::Claude => Agent::Claude,
            Self::Codex => Agent::Codex,
        }
    }
}

#[derive(Debug, Args)]
pub struct SkillShow {
    /// Command path, accepting the same aliases as the CLI (e.g. issue comment add)
    #[arg(value_name = "COMMAND")]
    pub command: Vec<String>,
    /// Print shipped guidance, ignoring all configuration and template overlays
    #[arg(long)]
    pub builtin: bool,
}

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
    /// Claude Code's reminder is written to .claude/settings.json there.
    /// Codex project hooks are skipped; set up its account reminder instead.
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
