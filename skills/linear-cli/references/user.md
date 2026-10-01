# user

> Manage Linear users

```
Manage Linear users

Usage: linear user [OPTIONS] [COMMAND]

Commands:
  list  List members of the workspace
  help  Print this message or the help of the given subcommand(s)

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

## list

> List members of the workspace

```
List members of the workspace

Usage: linear user list [OPTIONS]

Options:
  -a, --all               Include inactive members
      --workspace <slug>  Target workspace (uses credentials)
  -j, --json              Output as JSON; a member's url mentions them when pasted into Markdown. This searches the whole workspace — prefer `linear team members <TEAM>`, and confirm before mentioning someone outside the team
  -h, --help              Print help
```
