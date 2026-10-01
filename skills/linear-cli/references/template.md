# template

> Browse Linear issue, project, and document templates. Apply one with `issue create --template` or `project create --template`.

```
Browse Linear issue, project, and document templates. Apply one with `issue create --template` or `project create --template`.

Usage: linear template [OPTIONS] [COMMAND]

Commands:
  list  List templates. Without --team, every template in the workspace is shown.
  view  Show a template and what it pre-fills. Pass its name or ID. [alias: v]
  help  Print this message or the help of the given subcommand(s)

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

## list

> List templates. Without --team, every template in the workspace is shown.

```
List templates. Without --team, every template in the workspace is shown.

Usage: linear template list [OPTIONS]

Options:
      --type <type>       Only templates of this type (issue, project, or document) [possible values: issue, project, document]
      --workspace <slug>  Target workspace (uses credentials)
      --team <team>       Team key, name, or ID. Shows that team's templates plus workspace templates.
  -j, --json              Output as JSON
  -h, --help              Print help
```

## view

> Show a template and what it pre-fills. Pass its name or ID.

```
Show a template and what it pre-fills. Pass its name or ID.

Usage: linear template view [OPTIONS] <template>

Arguments:
  <template>  

Options:
  -j, --json              Output the template as JSON (templateData stays a JSON-encoded string; use `jq '.templateData | fromjson'`)
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```
