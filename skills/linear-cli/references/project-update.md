# project-update

> Manage project status updates

```
Manage project status updates

Usage: linear project-update [OPTIONS] [COMMAND]

Commands:
  create  Create a new status update for a project [alias: c]
  list    List status updates for a project [alias: l]
  help    Print this message or the help of the given subcommand(s)

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

## create

> Create a new status update for a project

```
Create a new status update for a project

Linear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,
and `[Name](url)` do not. Get a person's URL from the `url` field of
`linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.
Run `linear markdown` for collapsible sections and the full reference.

Usage: linear project-update create [OPTIONS] <projectId>

Arguments:
  <projectId>
          

Options:
      --body <body>
          Update content (inline)

      --workspace <slug>
          Target workspace (uses credentials)

      --body-file <path>
          Read content from file

      --health <health>
          Project health status (onTrack, atRisk, offTrack)

  -i, --interactive
          Interactive mode with prompts

  -h, --help
          Print help (see a summary with '-h')
```

## list

> List status updates for a project

```
List status updates for a project

Usage: linear project-update list [OPTIONS] <projectId>

Arguments:
  <projectId>  

Options:
      --json              Output as JSON
      --workspace <slug>  Target workspace (uses credentials)
      --limit <limit>     Limit results [default: 10]
  -h, --help              Print help
```
