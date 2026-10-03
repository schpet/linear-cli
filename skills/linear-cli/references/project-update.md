# project-update

> Post and list project status updates

## Usage

```
Post and list project status updates

Usage: linear project-update [OPTIONS] <COMMAND>

Commands:
  create  Post a status update on a project [alias: c]
  list    List a project's status updates [alias: l]
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

## Subcommands

### create

> Post a status update on a project

```
Post a status update on a project

Usage: linear project-update create [OPTIONS] <PROJECT>

Arguments:
  <PROJECT>
          Project ID, slug, or name

Options:
      --body <TEXT>
          Update text, in Markdown

      --body-file <FILE>
          Read the update from a Markdown file

      --health <HEALTH>
          How the work is going
          
          [possible values: on-track, at-risk, off-track]

  -i, --interactive
          Also prompt for the optional fields

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

### list

> List a project's status updates

```
List a project's status updates

Usage: linear project-update list [OPTIONS] <PROJECT>

Arguments:
  <PROJECT>  Project ID, slug, or name

Options:
  -j, --json           Print JSON
      --limit <LIMIT>  Maximum number of updates to show, newest first (a number or `all`) [default:
                       10]
  -h, --help           Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```
