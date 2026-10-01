# milestone

> Manage Linear project milestones

```
Manage Linear project milestones

Usage: linear milestone [OPTIONS] [COMMAND]

Commands:
  list    List milestones for a project
  view    View milestone details. By default lists the first 10 attached issues from the first page of 50; use --all to paginate the full set. [alias: v]
  create  Create a new project milestone
  update  Update an existing project milestone
  delete  Delete a project milestone
  help    Print this message or the help of the given subcommand(s)

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

## list

> List milestones for a project

```
List milestones for a project

Usage: linear milestone list [OPTIONS] --project <project>

Options:
      --project <project>  Project (UUID, slug ID, or name)
      --workspace <slug>   Target workspace (uses credentials)
  -j, --json               Output as JSON
  -h, --help               Print help
```

## view

> View milestone details. By default lists the first 10 attached issues from the first page of 50; use --all to paginate the full set.

```
View milestone details. By default lists the first 10 attached issues from the first page of 50; use --all to paginate the full set.

Usage: linear milestone view [OPTIONS] <milestone>

Arguments:
  <milestone>  

Options:
      --all                Fetch and list every issue attached to the milestone (paginates the Linear API).
      --workspace <slug>   Target workspace (uses credentials)
      --project <project>  Project for resolving a milestone name (UUID, slug ID, or name)
  -j, --json               Output as JSON
  -h, --help               Print help
```

## create

> Create a new project milestone

```
Create a new project milestone

Usage: linear milestone create [OPTIONS] --project <project> --name <name>

Options:
      --project <project>          Project (UUID, slug ID, or name)
      --workspace <slug>           Target workspace (uses credentials)
      --name <name>                Milestone name
      --description <description>  Milestone description
      --target-date <date>         Target date (YYYY-MM-DD)
  -h, --help                       Print help
```

## update

> Update an existing project milestone

```
Update an existing project milestone

Usage: linear milestone update [OPTIONS] <id>

Arguments:
  <id>  

Options:
      --name <name>                Milestone name
      --workspace <slug>           Target workspace (uses credentials)
      --description <description>  Milestone description
      --target-date <date>         Target date (YYYY-MM-DD)
      --sort-order <value>         Sort order relative to other milestones
      --project <project>          Move to a different project (UUID, slug ID, or name)
  -h, --help                       Print help
```

## delete

> Delete a project milestone

```
Delete a project milestone

Usage: linear milestone delete [OPTIONS] <id>

Arguments:
  <id>  

Options:
  -f, --force             Skip confirmation prompt
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```
