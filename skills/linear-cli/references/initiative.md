# initiative

> Manage initiatives

## Usage

```
Manage initiatives

Usage: linear initiative [OPTIONS] <COMMAND>

Commands:
  list            List initiatives
  view            Show an initiative
  create          Create an initiative
  update          Update an initiative
  archive         Archive an initiative
  unarchive       Restore an archived initiative
  delete          Delete an initiative permanently
  add-project     Add a project to an initiative
  remove-project  Remove a project from an initiative
  comment         Add and list comments on an initiative
  help            Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

## Subcommands

### list

> List initiatives

```
List initiatives

Usage: linear initiative list [OPTIONS]

Options:
  -s, --status <STATUS>  Show only initiatives with this status [default: active] [possible values:
                         planned, active, completed]
      --all-statuses     Show initiatives of every status
  -o, --owner <USER>     Show only initiatives owned by this user: a username, email, name, or @me
  -w, --web              Open the initiatives page in the browser
  -a, --app              Open the initiatives page in the Linear app
      --limit <LIMIT>    Maximum number of initiatives to show (a number or `all`) [default: all]
  -j, --json             Print JSON
      --archived         Include archived initiatives
  -h, --help             Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### view

> Show an initiative

```
Show an initiative

Usage: linear initiative view [OPTIONS] <INITIATIVE>

Arguments:
  <INITIATIVE>  Initiative ID, slug, or name

Options:
  -w, --web   Open the initiative in the browser
  -a, --app   Open the initiative in the Linear app
  -j, --json  Print JSON
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### create

> Create an initiative

```
Create an initiative

Usage: linear initiative create [OPTIONS]

Options:
  -n, --name <NAME>                Initiative name
  -d, --description <DESCRIPTION>  Initiative description
  -s, --status <STATUS>            Initiative status [default: planned] [possible values: planned,
                                   active, completed]
  -o, --owner <USER>               Owner: a username, email, name, or @me
      --target-date <DATE>         Target date (YYYY-MM-DD)
  -c, --color <COLOR>              Color, like #5E6AD2
      --icon <ICON>                Icon name
  -i, --interactive                Also prompt for the optional fields
  -y, --yes                        Do not ask for confirmation
  -h, --help                       Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### update

> Update an initiative

```
Update an initiative

Usage: linear initiative update [OPTIONS] <INITIATIVE>

Arguments:
  <INITIATIVE>  Initiative ID, slug, or name

Options:
  -n, --name <NAME>                New name
  -d, --description <DESCRIPTION>  New description
      --status <STATUS>            New status [possible values: planned, active, completed]
      --owner <USER>               New owner: a username, email, name, or @me
      --target-date <DATE>         New target date (YYYY-MM-DD)
      --color <COLOR>              New color, like #5E6AD2
      --icon <ICON>                New icon name
  -i, --interactive                Prompt for the fields to change
  -y, --yes                        Do not ask for confirmation
  -h, --help                       Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### archive

> Archive an initiative

```
Archive an initiative

Usage: linear initiative archive [OPTIONS] [INITIATIVE]

Arguments:
  [INITIATIVE]  Initiative ID, slug, or name

Options:
  -y, --yes               Do not ask for confirmation
      --bulk [<IDS>...]   Act on several at once instead of one
      --bulk-file <FILE>  Read the IDs from a file, one per line
      --bulk-stdin        Read the IDs from stdin, one per line
  -h, --help              Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### unarchive

> Restore an archived initiative

```
Restore an archived initiative

Usage: linear initiative unarchive [OPTIONS] <INITIATIVE>

Arguments:
  <INITIATIVE>  Initiative ID, slug, or name

Options:
  -y, --yes   Do not ask for confirmation
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### delete

> Delete an initiative permanently

```
Delete an initiative permanently

Usage: linear initiative delete [OPTIONS] [INITIATIVE]

Arguments:
  [INITIATIVE]  Initiative ID, slug, or name

Options:
  -y, --yes               Do not ask for confirmation
      --bulk [<IDS>...]   Act on several at once instead of one
      --bulk-file <FILE>  Read the IDs from a file, one per line
      --bulk-stdin        Read the IDs from stdin, one per line
  -h, --help              Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### add-project

> Add a project to an initiative

```
Add a project to an initiative

Usage: linear initiative add-project [OPTIONS] <INITIATIVE> <PROJECT>

Arguments:
  <INITIATIVE>  Initiative ID, slug, or name
  <PROJECT>     Project ID, slug, or name

Options:
      --sort-order <NUMBER>  Position among the initiative's projects
  -h, --help                 Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### remove-project

> Remove a project from an initiative

```
Remove a project from an initiative

Usage: linear initiative remove-project [OPTIONS] <INITIATIVE> <PROJECT>

Arguments:
  <INITIATIVE>  Initiative ID, slug, or name
  <PROJECT>     Project ID, slug, or name

Options:
  -y, --yes   Do not ask for confirmation
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### comment

> Add and list comments on an initiative

```
Add and list comments on an initiative

Usage: linear initiative comment [OPTIONS] <COMMAND>

Commands:
  add   Comment on an initiative, or reply to a comment
  list  List an initiative's comments
  help  Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

#### comment subcommands

##### add

> Comment on an initiative, or reply to a comment

```
Comment on an initiative, or reply to a comment

Usage: linear initiative comment add [OPTIONS] <INITIATIVE>

Arguments:
  <INITIATIVE>
          Initiative ID, slug, or name

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

> List an initiative's comments

```
List an initiative's comments

Usage: linear initiative comment list [OPTIONS] <INITIATIVE>

Arguments:
  <INITIATIVE>  Initiative ID, slug, or name

Options:
      --limit <LIMIT>  Maximum number of comments to show (a number or `all`) [default: all]
  -j, --json           Print JSON
  -h, --help           Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```
