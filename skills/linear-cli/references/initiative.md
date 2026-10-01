# initiative

> Manage Linear initiatives

```
Manage Linear initiatives

Usage: linear initiative [OPTIONS] [COMMAND]

Commands:
  list            List initiatives [alias: ls]
  view            View initiative details [alias: v]
  create          Create a new Linear initiative
  archive         Archive a Linear initiative
  update          Update a Linear initiative
  unarchive       Unarchive a Linear initiative
  delete          Permanently delete a Linear initiative
  add-project     Link a project to an initiative
  remove-project  Unlink a project from an initiative
  comment         Manage initiative comments
  help            Print this message or the help of the given subcommand(s)

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

## list

> List initiatives

```
List initiatives

Usage: linear initiative list [OPTIONS]

Options:
  -s, --status <status>   Filter by status (active, planned, completed)
      --workspace <slug>  Target workspace (uses credentials)
      --all-statuses      Show all statuses (default: active only)
  -o, --owner <owner>     Filter by owner (username or email)
  -w, --web               Open initiatives page in web browser
  -a, --app               Open initiatives page in Linear.app
  -j, --json              Output as JSON
      --archived          Include archived initiatives
  -h, --help              Print help
```

## view

> View initiative details

```
View initiative details

Usage: linear initiative view [OPTIONS] <initiativeId>

Arguments:
  <initiativeId>  

Options:
  -w, --web               Open in web browser
      --workspace <slug>  Target workspace (uses credentials)
  -a, --app               Open in Linear.app
  -j, --json              Output as JSON
  -h, --help              Print help
```

## create

> Create a new Linear initiative

```
Create a new Linear initiative

Usage: linear initiative create [OPTIONS]

Options:
  -n, --name <name>                Initiative name (required)
      --workspace <slug>           Target workspace (uses credentials)
  -d, --description <description>  Initiative description
  -s, --status <status>            Status: planned, active, completed (default: planned)
  -o, --owner <owner>              Owner (username, email, or @me for yourself)
      --target-date <targetDate>   Target completion date (YYYY-MM-DD)
  -c, --color <color>              Color hex code (e.g., #5E6AD2)
      --icon <icon>                Icon name
  -i, --interactive                Interactive mode (default if no flags provided)
  -h, --help                       Print help
```

## archive

> Archive a Linear initiative

```
Archive a Linear initiative

Usage: linear initiative archive [OPTIONS] [initiativeId]

Arguments:
  [initiativeId]  

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -y, --force             Skip confirmation prompt
      --bulk [<ids>...]   Archive multiple initiatives by ID, slug, or name
      --bulk-file <file>  Read initiative IDs from a file (one per line)
      --bulk-stdin        Read initiative IDs from stdin
  -h, --help              Print help
```

## update

> Update a Linear initiative

```
Update a Linear initiative

Usage: linear initiative update [OPTIONS] <initiativeId>

Arguments:
  <initiativeId>  

Options:
  -n, --name <name>                New name for the initiative
      --workspace <slug>           Target workspace (uses credentials)
  -d, --description <description>  New description
      --status <status>            New status (planned, active, completed, paused)
      --owner <owner>              New owner (username, email, or @me)
      --target-date <targetDate>   Target completion date (YYYY-MM-DD)
      --color <color>              Initiative color (hex, e.g., #5E6AD2)
      --icon <icon>                Initiative icon name
  -i, --interactive                Interactive mode for updates
  -h, --help                       Print help
```

## unarchive

> Unarchive a Linear initiative

```
Unarchive a Linear initiative

Usage: linear initiative unarchive [OPTIONS] <initiativeId>

Arguments:
  <initiativeId>  

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -y, --force             Skip confirmation prompt
  -h, --help              Print help
```

## delete

> Permanently delete a Linear initiative

```
Permanently delete a Linear initiative

Usage: linear initiative delete [OPTIONS] [initiativeId]

Arguments:
  [initiativeId]  

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -y, --force             Skip confirmation prompt
      --bulk [<ids>...]   Delete multiple initiatives by ID, slug, or name
      --bulk-file <file>  Read initiative IDs from a file (one per line)
      --bulk-stdin        Read initiative IDs from stdin
  -h, --help              Print help
```

## add-project

> Link a project to an initiative

```
Link a project to an initiative

Usage: linear initiative add-project [OPTIONS] <initiative> <project>

Arguments:
  <initiative>  
  <project>     

Options:
      --sort-order <sortOrder>  Sort order within initiative
      --workspace <slug>        Target workspace (uses credentials)
  -h, --help                    Print help
```

## remove-project

> Unlink a project from an initiative

```
Unlink a project from an initiative

Usage: linear initiative remove-project [OPTIONS] <initiative> <project>

Arguments:
  <initiative>  
  <project>     

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -y, --force             Skip confirmation prompt
  -h, --help              Print help
```

## comment

> Manage initiative comments

```
Manage initiative comments

Usage: linear initiative comment [OPTIONS] [COMMAND]

Commands:
  add   Add a comment or reply to an initiative's discussion (by ID, slug, or name)
  list  List comments on an initiative (by ID, slug, or name)
  help  Print this message or the help of the given subcommand(s)

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

### add

> Add a comment or reply to an initiative's discussion (by ID, slug, or name)

```
Add a comment or reply to an initiative's discussion (by ID, slug, or name)

Linear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,
and `[Name](url)` do not. Get a person's URL from the `url` field of
`linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.
Run `linear markdown` for collapsible sections and the full reference.

Usage: linear initiative comment add [OPTIONS] <initiative>

Arguments:
  <initiative>
          

Options:
  -b, --body <text>
          Comment body text

      --workspace <slug>
          Target workspace (uses credentials)

      --body-file <path>
          Read comment body from a file (preferred for markdown content)

  -p, --parent <commentId>
          Reply to a top-level comment by ID (the reply joins that thread)
          
          [alias: --reply-to]

  -h, --help
          Print help (see a summary with '-h')
```

### list

> List comments on an initiative (by ID, slug, or name)

```
List comments on an initiative (by ID, slug, or name)

Usage: linear initiative comment list [OPTIONS] <initiative>

Arguments:
  <initiative>  

Options:
  -j, --json              Output as JSON
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```
