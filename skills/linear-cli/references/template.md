# template

> Browse issue, project and document templates

## Usage

```
Browse issue, project and document templates

Apply one with `linear issue create --template` or `linear project create --template`.

Usage: linear template [OPTIONS] <COMMAND>

Commands:
  list  List templates
  view  Show a template and the fields it fills in
  help  Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help (see a summary with '-h')

Global options:
      --workspace <SLUG>
          Workspace to use, by the name its credential is stored under

      --no-input
          Never prompt; fail instead when a required value is missing
```

## Subcommands

### list

> List templates

```
List templates

Without --team, every template in the workspace is listed.

Usage: linear template list [OPTIONS]

Options:
      --type <TYPE>
          Show only templates of this type
          
          [possible values: issue, project, document]

      --team <TEAM>
          Show this team's templates (key, name, or ID) plus workspace templates

      --limit <LIMIT>
          Maximum number of templates to show (a number or `all`)
          
          [default: all]

  -j, --json
          Print JSON

  -h, --help
          Print help (see a summary with '-h')

Global options:
      --workspace <SLUG>
          Workspace to use, by the name its credential is stored under

      --no-input
          Never prompt; fail instead when a required value is missing
```

### view

> Show a template and the fields it fills in

```
Show a template and the fields it fills in

Usage: linear template view [OPTIONS] <TEMPLATE>

Arguments:
  <TEMPLATE>
          Template name or ID

Options:
  -j, --json
          Print JSON
          
          `templateData` stays a JSON-encoded string; decode it with `jq '.templateData |
          fromjson'`.

  -h, --help
          Print help (see a summary with '-h')

Global options:
      --workspace <SLUG>
          Workspace to use, by the name its credential is stored under

      --no-input
          Never prompt; fail instead when a required value is missing
```
