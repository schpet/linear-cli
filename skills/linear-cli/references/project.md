# project

> Manage Linear projects

```
Manage Linear projects

Usage: linear project [OPTIONS] [COMMAND]

Commands:
  list     List projects
  view     View project details [alias: v]
  create   Create a new Linear project
  update   Update a Linear project
  delete   Delete (trash) a Linear project
  comment  Manage project comments
  help     Print this message or the help of the given subcommand(s)

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

## list

> List projects

```
List projects

Usage: linear project list [OPTIONS]

Options:
      --team <team>       Filter by team key, name, or ID
      --workspace <slug>  Target workspace (uses credentials)
      --all-teams         Show projects from all teams
      --status <status>   Filter by status name
  -w, --web               Open in web browser
  -a, --app               Open in Linear.app
  -j, --json              Output as JSON
  -h, --help              Print help
```

## view

> View project details

```
View project details

Usage: linear project view [OPTIONS] [projectId]

Arguments:
  [projectId]  

Options:
  -w, --web               Open in web browser
      --workspace <slug>  Target workspace (uses credentials)
  -a, --app               Open in Linear.app
  -j, --json              Output as JSON
      --no-pager          Disable automatic paging for long output
  -h, --help              Print help
```

## create

> Create a new Linear project

```
Create a new Linear project

Linear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,
and `[Name](url)` do not. Get a person's URL from the `url` field of
`linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.
Run `linear markdown` for collapsible sections and the full reference.

Usage: linear project create [OPTIONS]

Options:
  -n, --name <name>
          Project name (required)

      --workspace <slug>
          Target workspace (uses credentials)

  -d, --description <description>
          Project description (max 255 characters, enforced by Linear's API)

  -f, --description-file <path>
          Read project description from file (still subject to the 255-character API limit)

      --content <markdown>
          Project overview markdown

      --content-file <path>
          Read project overview markdown from a file

  -t, --team <team>
          Team key, name, or ID (required, can be repeated for multiple teams)

  -l, --lead <lead>
          Project lead (username, email, or @me)

  -s, --status <status>
          Project status (planned, started, paused, completed, canceled, backlog)

      --start-date <startDate>
          Start date (YYYY-MM-DD)

      --target-date <targetDate>
          Target completion date (YYYY-MM-DD)

      --priority <priority>
          Project priority (none, urgent, high, medium, low)

      --label <label>
          Project label associated with the project. May be repeated.

      --member <user>
          Project member (username, email, display name, or @me). May be repeated.

      --icon <icon>
          Project icon

      --color <color>
          Project color as a HEX string

      --initiative <initiative>
          Add to initiative immediately (ID, slug, or name)

      --template <template>
          Project template to apply, by name or ID (workspace templates plus those of the project's teams). The template fills in anything you do not pass; explicit flags override it. Applied on create only.

  -i, --interactive
          Interactive mode (default if no flags provided)

  -j, --json
          Output created project as JSON

  -h, --help
          Print help (see a summary with '-h')
```

## update

> Update a Linear project

```
Update a Linear project

Linear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,
and `[Name](url)` do not. Get a person's URL from the `url` field of
`linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.
Run `linear markdown` for collapsible sections and the full reference.

Usage: linear project update [OPTIONS] <projectId>

Arguments:
  <projectId>
          

Options:
  -n, --name <name>
          Project name

      --workspace <slug>
          Target workspace (uses credentials)

  -d, --description <description>
          Project description (max 255 characters, enforced by Linear's API)

  -f, --description-file <path>
          Read project description from file (still subject to the 255-character API limit)

      --content <markdown>
          Project overview markdown

      --content-file <path>
          Read project overview markdown from a file

  -s, --status <status>
          Status (planned, started, paused, completed, canceled, backlog)

  -l, --lead <lead>
          Project lead (username, email, or @me). Use --clear-lead to remove it

      --clear-lead
          Remove the project's lead (cannot be combined with --lead)

      --start-date <startDate>
          Start date (YYYY-MM-DD). Use --clear-start-date to remove it

      --clear-start-date
          Remove the project's start date (cannot be combined with --start-date)

      --target-date <targetDate>
          Target date (YYYY-MM-DD). Use --clear-target-date to remove it

      --clear-target-date
          Remove the project's target date (cannot be combined with --target-date)

  -t, --team <team>
          Team key, name, or ID; replaces the project's entire team set. May be repeated. Use --add-team/--remove-team to change teams incrementally.

      --add-team <team>
          Add a team to the project, keeping its existing teams. May be repeated.

      --remove-team <team>
          Remove a team from the project, keeping its other teams. May be repeated.

      --label <label>
          Project label; replaces the project's entire label set. May be repeated. Use --add-label/--remove-label to change labels incrementally.

      --add-label <label>
          Add a label to the project, keeping its existing labels. May be repeated.

      --remove-label <label>
          Remove a label from the project, keeping its other labels (does not delete the label). May be repeated.

      --initiative <initiative>
          Initiative ID, slug, or name; replaces the project's entire initiative set. May be repeated. Use --add-initiative/--remove-initiative to change initiatives incrementally.

      --add-initiative <initiative>
          Add the project to an initiative, keeping its existing initiatives. May be repeated.

      --remove-initiative <initiative>
          Remove the project from an initiative, keeping its other initiatives (does not delete the initiative). May be repeated.

  -h, --help
          Print help (see a summary with '-h')
```

## delete

> Delete (trash) a Linear project

```
Delete (trash) a Linear project

Usage: linear project delete [OPTIONS] <projectId>

Arguments:
  <projectId>  

Options:
  -f, --force             Skip confirmation prompt
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

## comment

> Manage project comments

```
Manage project comments

Usage: linear project comment [OPTIONS] [COMMAND]

Commands:
  add   Add a comment or reply to a project's discussion (by ID, slug, or name)
  list  List comments on a project (by ID, slug, or name)
  help  Print this message or the help of the given subcommand(s)

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

### add

> Add a comment or reply to a project's discussion (by ID, slug, or name)

```
Add a comment or reply to a project's discussion (by ID, slug, or name)

Linear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,
and `[Name](url)` do not. Get a person's URL from the `url` field of
`linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.
Run `linear markdown` for collapsible sections and the full reference.

Usage: linear project comment add [OPTIONS] <project>

Arguments:
  <project>
          

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

> List comments on a project (by ID, slug, or name)

```
List comments on a project (by ID, slug, or name)

Usage: linear project comment list [OPTIONS] <project>

Arguments:
  <project>  

Options:
  -j, --json              Output as JSON
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```
