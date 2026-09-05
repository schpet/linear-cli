# template

> Browse Linear issue, project, and document templates. Apply one with `issue create --template` or `project create --template`.

## Usage

```
Usage:   linear template

Description:

  Browse Linear issue, project, and document templates. Apply one with `issue create --template` or `project create --template`.

Options:

  -h, --help           - Show this help.                      
  --workspace  <slug>  - Target workspace (uses credentials)  

Commands:

  list                 - List templates. Without --team, every template in the workspace is shown.
  view, v  <template>  - Show a template and what it pre-fills. Pass its name or ID.
```

## Subcommands

### list

> List templates. Without --team, every template in the workspace is shown.

```
Usage:   linear template list

Description:

  List templates. Without --team, every template in the workspace is shown.

Options:

  -h, --help           - Show this help.                                                                                                       
  --workspace  <slug>  - Target workspace (uses credentials)                                                                                   
  --type       <type>  - Only templates of this type (issue, project, or document)                     (Values: "issue", "project", "document")
  --team       <team>  - Team key, name, or ID. Shows that team's templates plus workspace templates.                                          
  -j, --json           - Output as JSON
```

### view

> Show a template and what it pre-fills. Pass its name or ID.

```
Usage:   linear template view <template>

Description:

  Show a template and what it pre-fills. Pass its name or ID.

Options:

  -h, --help           - Show this help.                                                                 
  --workspace  <slug>  - Target workspace (uses credentials)                                             
  -j, --json           - Output the template as JSON (templateData stays a JSON-encoded string; use `jq  
                         '.templateData | fromjson'`)
```
