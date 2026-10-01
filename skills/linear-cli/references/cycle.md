# cycle

> Manage Linear team cycles

```
Manage Linear team cycles

Usage: linear cycle [OPTIONS] [COMMAND]

Commands:
  list  List cycles for a team
  view  View cycle details [alias: v]
  help  Print this message or the help of the given subcommand(s)

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

## list

> List cycles for a team

```
List cycles for a team

Usage: linear cycle list [OPTIONS]

Options:
      --team <team>       Team key, name, or ID (defaults to current team)
      --workspace <slug>  Target workspace (uses credentials)
  -j, --json              Output as JSON
  -h, --help              Print help
```

## view

> View cycle details

```
View cycle details

Usage: linear cycle view [OPTIONS] <cycleRef>

Arguments:
  <cycleRef>  

Options:
      --team <team>       Team key, name, or ID (defaults to current team)
      --workspace <slug>  Target workspace (uses credentials)
  -j, --json              Output as JSON
  -h, --help              Print help
```
