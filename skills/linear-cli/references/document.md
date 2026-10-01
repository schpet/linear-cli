# document

> Manage Linear documents

```
Manage Linear documents

Usage: linear document [OPTIONS] [COMMAND]

Commands:
  list     List documents [alias: l]
  view     View a document's content [alias: v]
  create   Create a new document [alias: c]
  update   Update an existing document [alias: u]
  delete   Delete a document (moves to trash) [alias: d]
  comment  Manage document comments
  help     Print this message or the help of the given subcommand(s)

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

## list

> List documents

```
List documents

Usage: linear document list [OPTIONS]

Options:
      --project <project>        Filter by project (UUID, slug ID, or name)
      --workspace <slug>         Target workspace (uses credentials)
      --issue <issue>            Filter by issue (identifier like TC-123)
      --initiative <initiative>  Filter by initiative (UUID, slug ID, or name)
      --team <team>              Filter by team (key, name, or ID); with --cycle, scopes the cycle lookup instead
      --cycle <cycle>            Filter by cycle: name, number, 'active'/'now', 'next', 'previous', or a relative offset like +1 (team from --team or config)
      --release <release>        Filter by release (UUID, name, or version)
      --json                     Output as JSON
      --limit <limit>            Limit results [default: 50]
  -h, --help                     Print help
```

## view

> View a document's content

```
View a document's content

Usage: linear document view [OPTIONS] <id>

Arguments:
  <id>  

Options:
      --raw               Output raw markdown without rendering
      --workspace <slug>  Target workspace (uses credentials)
  -w, --web               Open document in browser
      --json              Output full document as JSON
      --no-download       Keep remote URLs instead of downloading files
  -h, --help              Print help
```

## create

> Create a new document

```
Create a new document

Linear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,
and `[Name](url)` do not. Get a person's URL from the `url` field of
`linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.
Run `linear markdown` for collapsible sections and the full reference.

Usage: linear document create [OPTIONS]

Options:
  -t, --title <title>
          Document title (required)

      --workspace <slug>
          Target workspace (uses credentials)

  -c, --content <content>
          Markdown content (inline)

  -f, --content-file <path>
          Read content from file

      --project <project>
          Attach to project (UUID, slug ID, or name)

      --issue <issue>
          Attach to issue (identifier like TC-123)

      --initiative <initiative>
          Attach to initiative (UUID, slug ID, or name)

      --team <team>
          Attach to team (key, name, or ID); with --cycle, scopes the cycle lookup instead

      --cycle <cycle>
          Attach to cycle: name, number, 'active'/'now', 'next', 'previous', or a relative offset like +1 (team from --team or config)

      --release <release>
          Attach to release (UUID, name, or version)

      --icon <icon>
          Document icon (emoji)

  -i, --interactive
          Interactive mode with prompts

  -h, --help
          Print help (see a summary with '-h')
```

## update

> Update an existing document

```
Update an existing document

Linear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,
and `[Name](url)` do not. Get a person's URL from the `url` field of
`linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.
Run `linear markdown` for collapsible sections and the full reference.

Usage: linear document update [OPTIONS] <documentId>

Arguments:
  <documentId>
          

Options:
  -t, --title <title>
          New title for the document

      --workspace <slug>
          Target workspace (uses credentials)

  -c, --content <content>
          New markdown content (inline)

  -f, --content-file <path>
          Read new content from file

      --icon <icon>
          New icon (emoji)

      --project <project>
          Re-point to project (UUID, slug ID, or name); replaces the current attachment

      --issue <issue>
          Re-point to issue (identifier like TC-123); replaces the current attachment

      --initiative <initiative>
          Re-point to initiative (UUID, slug ID, or name); replaces the current attachment

      --team <team>
          Re-point to team (key, name, or ID); with --cycle, scopes the cycle lookup instead

      --cycle <cycle>
          Re-point to cycle: name, number, 'active'/'now', 'next', 'previous', or a relative offset like +1 (team from --team or config)

      --release <release>
          Re-point to release (UUID, name, or version); replaces the current attachment

  -e, --edit
          Open current content in $EDITOR for editing

      --force
          Update content even when document comments may lose inline anchors

  -h, --help
          Print help (see a summary with '-h')
```

## delete

> Delete a document (moves to trash)

```
Delete a document (moves to trash)

Usage: linear document delete [OPTIONS] [documentId]

Arguments:
  [documentId]  

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -y, --yes               Skip confirmation prompt
      --bulk [<ids>...]   Delete multiple documents by slug or ID
      --bulk-file <file>  Read document slugs/IDs from a file (one per line)
      --bulk-stdin        Read document slugs/IDs from stdin
  -h, --help              Print help
```

## comment

> Manage document comments

```
Manage document comments

Usage: linear document comment [OPTIONS] [COMMAND]

Commands:
  add   Add a comment or reply to a document (by ID or slug)
  list  List comments on a document (by ID or slug)
  help  Print this message or the help of the given subcommand(s)

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

### add

> Add a comment or reply to a document (by ID or slug)

```
Add a comment or reply to a document (by ID or slug)

Linear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,
and `[Name](url)` do not. Get a person's URL from the `url` field of
`linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.
Run `linear markdown` for collapsible sections and the full reference.

Usage: linear document comment add [OPTIONS] <document>

Arguments:
  <document>
          

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

> List comments on a document (by ID or slug)

```
List comments on a document (by ID or slug)

Usage: linear document comment list [OPTIONS] <document>

Arguments:
  <document>  

Options:
  -j, --json              Output as JSON
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```
