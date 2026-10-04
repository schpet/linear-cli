# project

> Manage projects

## Usage

```
Manage projects

Usage: linear project [OPTIONS] <COMMAND>

Commands:
  list     List projects
  view     Show a project
  create   Create a project
  update   Update a project
  delete   Delete a project (moves it to the trash)
  comment  Add and list comments on a project
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

## Subcommands

### list

> List projects

```
List projects

Usage: linear project list [OPTIONS]

Options:
      --team <TEAM>      Show this team's projects (key, name, or ID); defaults to the configured
                         team
      --all-teams        Show every team's projects
      --status <STATUS>  Show only projects with this status [possible values: backlog, planned,
                         started, paused, completed, canceled]
  -w, --web              Open the projects page in the browser
  -a, --app              Open the projects page in the Linear app
      --limit <LIMIT>    Maximum number of projects to show (a number or `all`) [default: all]
  -j, --json             Print JSON
  -h, --help             Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### view

> Show a project

```
Show a project

Usage: linear project view [OPTIONS] [PROJECT]

Arguments:
  [PROJECT]  Project ID, slug, or name; asked for when omitted

Options:
  -w, --web       Open the project in the browser
  -a, --app       Open the project in the Linear app
  -j, --json      Print JSON
      --no-pager  Do not page long output
  -h, --help      Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### create

> Create a project

```
Create a project

Usage: linear project create [OPTIONS]

Options:
  -n, --name <NAME>
          Project name

  -d, --description <DESCRIPTION>
          Short summary, up to 255 characters

  -f, --description-file <FILE>
          Read the summary from a file

      --content <MARKDOWN>
          Project overview, in Markdown

      --content-file <FILE>
          Read the overview from a Markdown file

  -s, --status <STATUS>
          Project status
          
          [possible values: backlog, planned, started, paused, completed, canceled]

  -l, --lead <USER>
          Project lead: a username, email, name, or @me

      --start-date <DATE>
          Start date (YYYY-MM-DD)

      --target-date <DATE>
          Target date (YYYY-MM-DD)

      --priority <PRIORITY>
          Project priority, by name or number (0 none, 1 urgent to 4 low)
          
          [possible values: none, urgent, high, medium, low]

  -t, --team <TEAM>
          Team (key, name, or ID); repeat for several teams

      --label <LABEL>
          Project label; repeat for several labels

      --member <USER>
          Project member: a username, email, name, or @me; repeatable

      --icon <ICON>
          Project icon

      --color <COLOR>
          Color, like #5E6AD2

      --initiative <INITIATIVE>
          Add the project to this initiative (ID, slug, or name)

      --template <TEMPLATE>
          Start from this project template (name or ID)
          
          Workspace templates and those of the project's teams are searched. The template fills in
          anything you do not pass; flags override it.

  -i, --interactive
          Also prompt for the optional fields

  -j, --json
          Print the created project as JSON

  -y, --yes
          Do not ask for confirmation

  -h, --help
          Print help (see a summary with '-h')

Global options:
      --workspace <SLUG>
          Workspace to use, by the name its credential is stored under

      --no-input
          Never prompt; fail instead when a required value is missing

Linear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,
and `[Name](url)` do not. Get a person's URL from the `url` field of
`linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.
Run `linear markdown` for collapsible sections and the full reference.
```

### update

> Update a project

```
Update a project

Usage: linear project update [OPTIONS] <PROJECT>

Arguments:
  <PROJECT>
          Project ID, slug, or name

Options:
  -n, --name <NAME>
          Project name

  -d, --description <DESCRIPTION>
          Short summary, up to 255 characters

  -f, --description-file <FILE>
          Read the summary from a file

      --content <MARKDOWN>
          Project overview, in Markdown

      --content-file <FILE>
          Read the overview from a Markdown file

  -s, --status <STATUS>
          Project status
          
          [possible values: backlog, planned, started, paused, completed, canceled]

  -l, --lead <USER>
          Project lead: a username, email, name, or @me

      --start-date <DATE>
          Start date (YYYY-MM-DD)

      --target-date <DATE>
          Target date (YYYY-MM-DD)

      --priority <PRIORITY>
          Project priority, by name or number (0 none, 1 urgent to 4 low)
          
          [possible values: none, urgent, high, medium, low]

      --clear-lead
          Remove the project's lead

      --clear-start-date
          Remove the project's start date

      --clear-target-date
          Remove the project's target date

  -t, --team <TEAM>
          Set the project's teams (key, name, or ID), replacing the current ones; repeatable

      --add-team <TEAM>
          Add a team to the project; repeatable

      --remove-team <TEAM>
          Remove a team from the project; repeatable

      --label <LABEL>
          Set the project's labels, replacing the current ones; repeatable

      --add-label <LABEL>
          Add a label to the project; repeatable

      --remove-label <LABEL>
          Remove a label from the project (the label itself stays); repeatable

      --initiative <INITIATIVE>
          Set the project's initiatives (ID, slug, or name), replacing the current ones; repeatable

      --add-initiative <INITIATIVE>
          Add the project to an initiative; repeatable

      --remove-initiative <INITIATIVE>
          Remove the project from an initiative (the initiative itself stays); repeatable

  -h, --help
          Print help (see a summary with '-h')

Global options:
      --workspace <SLUG>
          Workspace to use, by the name its credential is stored under

      --no-input
          Never prompt; fail instead when a required value is missing

Linear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,
and `[Name](url)` do not. Get a person's URL from the `url` field of
`linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.
Run `linear markdown` for collapsible sections and the full reference.
```

### delete

> Delete a project (moves it to the trash)

```
Delete a project (moves it to the trash)

Usage: linear project delete [OPTIONS] <PROJECT>

Arguments:
  <PROJECT>  Project ID, slug, or name

Options:
  -y, --yes   Do not ask for confirmation
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### comment

> Add and list comments on a project

```
Add and list comments on a project

Usage: linear project comment [OPTIONS] <COMMAND>

Commands:
  add   Comment on a project, or reply to a comment
  list  List a project's comments
  help  Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

#### comment subcommands

##### add

> Comment on a project, or reply to a comment

```
Comment on a project, or reply to a comment

Usage: linear project comment add [OPTIONS] <PROJECT>

Arguments:
  <PROJECT>
          Project ID, slug, or name

Options:
  -b, --body <TEXT>
          Comment text, in Markdown

      --body-file <FILE>
          Read the comment from a Markdown file

  -p, --reply-to <COMMENT>
          Reply to this top-level comment (by ID)
          
          [alias: --parent]

  -y, --yes
          Do not ask for confirmation

  -h, --help
          Print help (see a summary with '-h')

Global options:
      --workspace <SLUG>
          Workspace to use, by the name its credential is stored under

      --no-input
          Never prompt; fail instead when a required value is missing

Linear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,
and `[Name](url)` do not. Get a person's URL from the `url` field of
`linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.
Run `linear markdown` for collapsible sections and the full reference.
```

##### list

> List a project's comments

```
List a project's comments

Usage: linear project comment list [OPTIONS] <PROJECT>

Arguments:
  <PROJECT>  Project ID, slug, or name

Options:
      --limit <LIMIT>  Maximum number of comments to show (a number or `all`) [default: all]
  -j, --json           Print JSON
  -h, --help           Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```
