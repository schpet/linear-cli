# milestone

> Manage project milestones

## Usage

```
Manage project milestones

Usage: linear milestone [OPTIONS] <COMMAND>

Commands:
  list    List a project's milestones
  view    Show a milestone and its issues [alias: v]
  create  Create a project milestone
  update  Update a project milestone
  delete  Delete a project milestone
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

## Subcommands

### list

> List a project's milestones

```
List a project's milestones

Usage: linear milestone list [OPTIONS] --project <PROJECT>

Options:
      --project <PROJECT>  Project ID, slug, or name
      --limit <LIMIT>      Maximum number of milestones to show (a number or `all`) [default: all]
  -j, --json               Print JSON
  -h, --help               Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### view

> Show a milestone and its issues

```
Show a milestone and its issues

Usage: linear milestone view [OPTIONS] <MILESTONE>

Arguments:
  <MILESTONE>  Milestone ID, or its name with --project

Options:
      --all                List every issue instead of the first 10
      --project <PROJECT>  Project (ID, slug, or name) to find the milestone name in
  -j, --json               Print JSON
  -h, --help               Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### create

> Create a project milestone

```
Create a project milestone

Usage: linear milestone create [OPTIONS] --project <PROJECT> --name <NAME>

Options:
      --project <PROJECT>          Project ID, slug, or name
      --name <NAME>                Milestone name
      --description <DESCRIPTION>  Milestone description
      --target-date <DATE>         Target date (YYYY-MM-DD)
  -h, --help                       Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### update

> Update a project milestone

```
Update a project milestone

Usage: linear milestone update [OPTIONS] <ID>

Arguments:
  <ID>  Milestone ID

Options:
      --name <NAME>                New name
      --description <DESCRIPTION>  New description
      --target-date <DATE>         New target date (YYYY-MM-DD)
      --sort-order <NUMBER>        Position among the project's milestones
      --project <PROJECT>          Move the milestone to this project (ID, slug, or name)
  -h, --help                       Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### delete

> Delete a project milestone

```
Delete a project milestone

Usage: linear milestone delete [OPTIONS] <ID>

Arguments:
  <ID>  Milestone ID

Options:
  -y, --yes   Do not ask for confirmation
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```
