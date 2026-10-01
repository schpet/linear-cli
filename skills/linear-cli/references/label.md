# label

> Manage Linear issue labels

```
Manage Linear issue labels

Usage: linear label [OPTIONS] [COMMAND]

Commands:
  list    List issue labels
  create  Create a new issue label
  delete  Delete an issue label
  help    Print this message or the help of the given subcommand(s)

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

## list

> List issue labels

```
List issue labels

Usage: linear label list [OPTIONS]

Options:
      --team <team>       Filter by team key, name, or ID (e.g., TC). Shows that team's labels plus workspace labels.
      --workspace <slug>  Target workspace (uses credentials)
      --workspace-only    Show only workspace-level labels (not team-specific)
      --all               Show all labels (both workspace and team)
  -j, --json              Output as JSON
  -h, --help              Print help
```

## create

> Create a new issue label

```
Create a new issue label

Usage: linear label create [OPTIONS]

Options:
  -n, --name <name>                Label name (required)
      --workspace <slug>           Target workspace (uses credentials)
  -c, --color <color>              Color hex code (e.g., #EB5757)
  -d, --description <description>  Label description
  -t, --team <team>                Team key, name, or ID for a team-specific label (omit for workspace label)
  -i, --interactive                Interactive mode (default if no flags provided)
  -h, --help                       Print help
```

## delete

> Delete an issue label

```
Delete an issue label

Usage: linear label delete [OPTIONS] <nameOrId>

Arguments:
  <nameOrId>  

Options:
  -t, --team <team>       Team key, name, or ID to disambiguate labels with the same name
      --workspace <slug>  Target workspace (uses credentials)
  -f, --force             Skip confirmation prompt
  -h, --help              Print help
```
