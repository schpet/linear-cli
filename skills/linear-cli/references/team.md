# team

> Manage Linear teams

```
Manage Linear teams

Usage: linear team [OPTIONS] [COMMAND]

Commands:
  create     Create a linear team
  delete     Delete a Linear team
  list       List teams
  id         Print the configured team id
  autolinks  Configure GitHub repository autolinks for Linear issues with this team prefix
  members    List team members (team by key, name, or ID)
  states     List workflow states for a team (by key, name, or ID)
  help       Print this message or the help of the given subcommand(s)

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

## create

> Create a linear team

```
Create a linear team

Usage: linear team create [OPTIONS]

Options:
  -n, --name <name>                Name of the team
      --workspace <slug>           Target workspace (uses credentials)
  -d, --description <description>  Description of the team
  -k, --key <key>                  Team key (if not provided, will be generated from name)
      --private                    Make the team private
      --no-interactive             Disable interactive prompts
  -h, --help                       Print help
```

## delete

> Delete a Linear team

```
Delete a Linear team

Usage: linear team delete [OPTIONS] <team>

Arguments:
  <team>  

Options:
      --move-issues <targetTeam>  Move all issues to another team (key, name, or ID) before deletion
      --workspace <slug>          Target workspace (uses credentials)
  -y, --force                     Skip confirmation prompt
  -h, --help                      Print help
```

## list

> List teams

```
List teams

Usage: linear team list [OPTIONS]

Options:
  -w, --web               Open in web browser
      --workspace <slug>  Target workspace (uses credentials)
  -a, --app               Open in Linear.app
  -j, --json              Output as JSON
  -h, --help              Print help
```

## id

> Print the configured team id

```
Print the configured team id

Usage: linear team id [OPTIONS]

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

## autolinks

> Configure GitHub repository autolinks for Linear issues with this team prefix

```
Configure GitHub repository autolinks for Linear issues with this team prefix

Usage: linear team autolinks [OPTIONS]

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

## members

> List team members (team by key, name, or ID)

```
List team members (team by key, name, or ID)

Usage: linear team members [OPTIONS] [team]

Arguments:
  [team]  

Options:
  -a, --all               Include inactive members
      --workspace <slug>  Target workspace (uses credentials)
  -j, --json              Output as JSON; a member's url mentions them when pasted into Markdown
  -h, --help              Print help
```

## states

> List workflow states for a team (by key, name, or ID)

```
List workflow states for a team (by key, name, or ID)

Usage: linear team states [OPTIONS] [team]

Arguments:
  [team]  

Options:
  -j, --json              Output as JSON
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```
