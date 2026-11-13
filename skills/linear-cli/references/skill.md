# skill

> Teach coding agents to use linear

## Usage

```
Teach coding agents to use linear

`linear skill show` prints a guide to this CLI written for coding agents, and `linear skill setup`
installs a small skill that tells an agent to read it. The guide ships in this binary, so upgrading
linear also updates what agents read.

Run `linear skill setup` once per account, and `linear skill status` to see what is installed.

Usage: linear skill [OPTIONS] <COMMAND>

Commands:
  show    Print the guide that teaches an agent to use linear
  setup   Install linear's skill into coding agents
  status  Report where linear's skill is installed
  remove  Remove linear's skill from coding agents
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help (see a summary with '-h')

Global options:
      --workspace <SLUG>
          Workspace to use, by the name its credential is stored under

      --no-input
          Never prompt; fail instead when a required value is missing
```

## Subcommands

### show

> Print the guide that teaches an agent to use linear

```
Print the guide that teaches an agent to use linear

The guide is Markdown and comes from this binary, so it describes the commands this version has. It
needs no login, configuration or network connection. Run `linear COMMAND --help` for a command's
full options.

An agent with the skill installed reads the guide through the skill. An agent without it can run
this command to read the same guide.

Usage: linear skill show [OPTIONS]

Options:
  -h, --help
          Print help (see a summary with '-h')

Global options:
      --workspace <SLUG>
          Workspace to use, by the name its credential is stored under

      --no-input
          Never prompt; fail instead when a required value is missing
```

### setup

> Install linear's skill into coding agents

```
Install linear's skill into coding agents

Installs the skill for Claude Code, Codex, OpenCode and Pi. Codex, OpenCode and Pi all read the
shared ~/.agents/skills directory, which is always written. Claude Code has its own directory, which
is written when Claude Code is set up on this machine. Use --agent to choose.

Run this once per account. Running it again is safe, and updates a skill an older linear installed.

A skill named linear-cli that this command did not install is left alone and reported as a conflict,
and so is an older copy in a directory that would be read instead of the new one. --force moves them
to a backup directory beside the skills directory, never deleting anything, and installs the skill.

Usage: linear skill setup [OPTIONS]

Options:
      --agent <AGENT>
          Use only AGENT (repeatable)
          
          [possible values: claude, codex, opencode, pi]

      --project
          Set up this repository instead of your account
          
          Writes .agents/skills and .claude/skills at the root of the repository, or in the current
          directory outside one. Commit them to share the skill with everyone who works here.

      --dry-run
          Report what would change without changing anything

      --force
          Move a conflicting linear-cli skill to a backup and install over it

  -j, --json
          Print the result as JSON

  -h, --help
          Print help (see a summary with '-h')

Global options:
      --workspace <SLUG>
          Workspace to use, by the name its credential is stored under

      --no-input
          Never prompt; fail instead when a required value is missing
```

### status

> Report where linear's skill is installed

```
Report where linear's skill is installed

Lists each directory an agent reads the skill from and what is there.
Nothing is changed.

  current     installed and up to date
  outdated    installed by an older linear
  missing     not installed
  foreign     a linear-cli skill that `linear skill setup` did not
              install, or one edited since
  unreadable  could not be read

A current skill reads its guide from the linear on PATH each time, so
it does not go out of date when linear is upgraded.

Only the directories agents read by default are checked. Plugins and
directories added in an agent's own configuration are not.

Usage: linear skill status [OPTIONS]

Options:
      --project
          Report on this repository instead of your account

  -j, --json
          Print the report as JSON

  -h, --help
          Print help (see a summary with '-h')

Global options:
      --workspace <SLUG>
          Workspace to use, by the name its credential is stored under

      --no-input
          Never prompt; fail instead when a required value is missing
```

### remove

> Remove linear's skill from coding agents

```
Remove linear's skill from coding agents

Removes the skill from every directory `linear skill setup` installs into, or from those of the
agents named with --agent. Only the file setup installed is removed. A linear-cli skill it did not
install, and any other file beside the skill, is left as it was.

Usage: linear skill remove [OPTIONS]

Options:
      --agent <AGENT>
          Use only AGENT (repeatable)
          
          [possible values: claude, codex, opencode, pi]

      --project
          Remove from this repository instead of your account

      --dry-run
          Report what would change without changing anything

  -j, --json
          Print the result as JSON

  -h, --help
          Print help (see a summary with '-h')

Global options:
      --workspace <SLUG>
          Workspace to use, by the name its credential is stored under

      --no-input
          Never prompt; fail instead when a required value is missing
```
