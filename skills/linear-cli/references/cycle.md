# cycle

> View team cycles

## Usage

```
View team cycles

Usage: linear cycle [OPTIONS] <COMMAND>

Commands:
  list  List a team's cycles
  view  Show a cycle and its issues [alias: v]
  help  Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

## Subcommands

### list

> List a team's cycles

```
List a team's cycles

Usage: linear cycle list [OPTIONS]

Options:
      --team <TEAM>    Team key, name, or ID; defaults to the configured team
      --limit <LIMIT>  Maximum number of cycles to show, newest first (a number or `all`) [default:
                       all]
  -j, --json           Print JSON
  -h, --help           Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### view

> Show a cycle and its issues

```
Show a cycle and its issues

Usage: linear cycle view [OPTIONS] <CYCLE>

Arguments:
  <CYCLE>  Cycle name, number, `active`, `next`, `previous`, or an offset like +1 or -1

Options:
      --team <TEAM>  Team key, name, or ID; defaults to the configured team
  -j, --json         Print JSON
  -h, --help         Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```
