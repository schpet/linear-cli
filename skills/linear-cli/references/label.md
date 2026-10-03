# label

> Manage issue labels

## Usage

```
Manage issue labels

Usage: linear label [OPTIONS] <COMMAND>

Commands:
  list    List issue labels
  create  Create an issue label
  delete  Delete an issue label
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

## Subcommands

### list

> List issue labels

```
List issue labels

Usage: linear label list [OPTIONS]

Options:
      --team <TEAM>     Show this team's labels (key, name, or ID) plus workspace labels
      --workspace-only  Show only workspace labels
      --all-teams       Show workspace labels and every team's labels
      --limit <LIMIT>   Maximum number of labels to show (a number or `all`) [default: all]
  -j, --json            Print JSON
  -h, --help            Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### create

> Create an issue label

```
Create an issue label

Usage: linear label create [OPTIONS]

Options:
  -n, --name <NAME>                Label name
  -c, --color <COLOR>              Color, like #EB5757
  -d, --description <DESCRIPTION>  Label description
  -t, --team <TEAM>                Team (key, name, or ID) for a team label; omit for a workspace
                                   label
  -i, --interactive                Also prompt for the optional fields
  -h, --help                       Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### delete

> Delete an issue label

```
Delete an issue label

Usage: linear label delete [OPTIONS] <LABEL>

Arguments:
  <LABEL>  Label name or ID

Options:
  -t, --team <TEAM>  Team (key, name, or ID) whose label to delete, when names repeat
  -y, --yes          Do not ask for confirmation
  -h, --help         Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```
