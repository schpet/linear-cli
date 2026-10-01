# initiative-update

> Manage initiative status updates (timeline posts)

```
Manage initiative status updates (timeline posts)

Usage: linear initiative-update [OPTIONS] [COMMAND]

Commands:
  create  Create a new status update for an initiative [alias: c]
  list    List status updates for an initiative [aliases: l, ls]
  help    Print this message or the help of the given subcommand(s)

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

## create

> Create a new status update for an initiative

```
Create a new status update for an initiative

Linear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,
and `[Name](url)` do not. Get a person's URL from the `url` field of
`linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.
Run `linear markdown` for collapsible sections and the full reference.

Usage: linear initiative-update create [OPTIONS] <initiativeId>

Arguments:
  <initiativeId>
          

Options:
      --body <body>
          Update content (markdown)

      --workspace <slug>
          Target workspace (uses credentials)

      --body-file <path>
          Read content from file

      --health <health>
          Health status (onTrack, atRisk, offTrack)

  -i, --interactive
          Interactive mode with prompts

  -h, --help
          Print help (see a summary with '-h')
```

## list

> List status updates for an initiative

```
List status updates for an initiative

Usage: linear initiative-update list [OPTIONS] <initiativeId>

Arguments:
  <initiativeId>  

Options:
  -j, --json              Output as JSON
      --workspace <slug>  Target workspace (uses credentials)
      --limit <limit>     Limit results [default: 10]
  -h, --help              Print help
```
