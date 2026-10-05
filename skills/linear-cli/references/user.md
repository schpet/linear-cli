# user

> List workspace members

## Usage

```
List workspace members

Usage: linear user [OPTIONS] <COMMAND>

Commands:
  list  List the workspace's members
  help  Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

## Subcommands

### list

> List the workspace's members

```
List the workspace's members

Usage: linear user list [OPTIONS]

Options:
  -a, --all
          Include deactivated members

      --limit <LIMIT>
          Maximum number of members to show (a number or `all`)
          
          [default: all]

  -j, --json
          Print JSON; a member's `url` mentions them when pasted into Markdown
          
          This lists the whole workspace. To find someone to mention, prefer `linear team members
          <TEAM>`, and confirm before mentioning someone outside the team.

  -h, --help
          Print help (see a summary with '-h')

Global options:
      --workspace <SLUG>
          Workspace to use, by the name its credential is stored under

      --no-input
          Never prompt; fail instead when a required value is missing
```
