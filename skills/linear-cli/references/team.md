# team

> Manage teams

## Usage

```
Manage teams

Usage: linear team [OPTIONS] <COMMAND>

Commands:
  list       List teams
  create     Create a team
  delete     Delete a team
  members    List a team's members
  states     List a team's workflow states
  id         Print the configured team key
  autolinks  Link the configured team's issue IDs in the current GitHub repository
  help       Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

## Subcommands

### list

> List teams

```
List teams

Usage: linear team list [OPTIONS]

Options:
  -w, --web            Open the teams page in the browser
  -a, --app            Open the teams page in the Linear app
      --limit <LIMIT>  Maximum number of teams to show (a number or `all`) [default: all]
  -j, --json           Print JSON
  -h, --help           Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### create

> Create a team

```
Create a team

Usage: linear team create [OPTIONS]

Options:
  -n, --name <NAME>                Team name
  -d, --description <DESCRIPTION>  Team description
  -k, --key <KEY>                  Team key, like ENG; derived from the name when omitted
      --private                    Make the team private
  -i, --interactive                Also prompt for the optional fields
  -h, --help                       Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### delete

> Delete a team

```
Delete a team

Usage: linear team delete [OPTIONS] <TEAM>

Arguments:
  <TEAM>  Team key, name, or ID

Options:
      --move-issues <TEAM>  Move the team's issues to this team (key, name, or ID) first
  -y, --yes                 Do not ask for confirmation
  -h, --help                Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### members

> List a team's members

```
List a team's members

Usage: linear team members [OPTIONS] [TEAM]

Arguments:
  [TEAM]  Team key, name, or ID; defaults to the configured team

Options:
  -a, --all            Include deactivated members
      --limit <LIMIT>  Maximum number of members to show (a number or `all`) [default: all]
  -j, --json           Print JSON; a member's `url` mentions them when pasted into Markdown
  -h, --help           Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### states

> List a team's workflow states

```
List a team's workflow states

Usage: linear team states [OPTIONS] [TEAM]

Arguments:
  [TEAM]  Team key, name, or ID; defaults to the configured team

Options:
      --limit <LIMIT>  Maximum number of states to show (a number or `all`) [default: all]
  -j, --json           Print JSON
  -h, --help           Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### id

> Print the configured team key

```
Print the configured team key

Usage: linear team id [OPTIONS]

Options:
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### autolinks

> Link the configured team's issue IDs in the current GitHub repository

```
Link the configured team's issue IDs in the current GitHub repository

Adds a GitHub autolink so that references like ENG-123 in commits, issues and pull requests link to
Linear. Needs the `gh` CLI.

Usage: linear team autolinks [OPTIONS]

Options:
  -h, --help
          Print help (see a summary with '-h')

Global options:
      --workspace <SLUG>
          Workspace to use, by the name its credential is stored under

      --no-input
          Never prompt; fail instead when a required value is missing
```
