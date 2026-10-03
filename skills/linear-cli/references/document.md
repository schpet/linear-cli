# document

> Manage documents

## Usage

```
Manage documents

Usage: linear document [OPTIONS] <COMMAND>

Commands:
  list     List documents [alias: l]
  view     Show a document [alias: v]
  create   Create a document [alias: c]
  update   Update a document [alias: u]
  delete   Delete a document (moves it to the trash) [alias: d]
  comment  Add and list comments on a document
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

## Subcommands

### list

> List documents

```
List documents

Usage: linear document list [OPTIONS]

Options:
      --project <PROJECT>        Show this project's documents (ID, slug, or name)
      --issue <ISSUE>            Show this issue's documents (like ENG-123)
      --initiative <INITIATIVE>  Show this initiative's documents (ID, slug, or name)
      --team <TEAM>              Show this team's documents (key, name, or ID); with --cycle, the
                                 cycle's team
      --cycle <CYCLE>            Show this cycle's documents: a name, number, `active`, `next`,
                                 `previous`, or an offset like +1
      --release <RELEASE>        Show this release's documents (ID, name, or version)
  -j, --json                     Print JSON
      --limit <LIMIT>            Maximum number of documents to show (a number or `all`) [default:
                                 50]
  -h, --help                     Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### view

> Show a document

```
Show a document

Usage: linear document view [OPTIONS] <DOCUMENT>

Arguments:
  <DOCUMENT>  Document ID or slug

Options:
      --raw          Print the Markdown source instead of rendering it
  -w, --web          Open the document in the browser
  -j, --json         Print JSON
      --no-download  Keep remote image and file URLs instead of downloading them
  -h, --help         Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### create

> Create a document

```
Create a document

Usage: linear document create [OPTIONS]

Options:
  -t, --title <TITLE>
          Document title

  -c, --content <MARKDOWN>
          Document text, in Markdown

  -f, --content-file <FILE>
          Read the document from a Markdown file

      --project <PROJECT>
          Attach the document to a project (ID, slug, or name)

      --issue <ISSUE>
          Attach the document to an issue (like ENG-123)

      --initiative <INITIATIVE>
          Attach the document to an initiative (ID, slug, or name)

      --team <TEAM>
          Attach the document to a team (key, name, or ID); with --cycle, the cycle's team

      --cycle <CYCLE>
          Attach the document to a cycle: a name, number, `active`, `next`, `previous`, or an offset
          like +1

      --release <RELEASE>
          Attach the document to a release (ID, name, or version)

      --icon <ICON>
          Document icon (an emoji)

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

### update

> Update a document

```
Update a document

Usage: linear document update [OPTIONS] <DOCUMENT>

Arguments:
  <DOCUMENT>
          Document ID or slug

Options:
  -t, --title <TITLE>
          New title

  -c, --content <MARKDOWN>
          New text, in Markdown

  -f, --content-file <FILE>
          Read the new text from a Markdown file

      --icon <ICON>
          New icon (an emoji)

      --project <PROJECT>
          Move the document to a project (ID, slug, or name)

      --issue <ISSUE>
          Move the document to an issue (like ENG-123)

      --initiative <INITIATIVE>
          Move the document to an initiative (ID, slug, or name)

      --team <TEAM>
          Move the document to a team (key, name, or ID); with --cycle, the cycle's team

      --cycle <CYCLE>
          Move the document to a cycle: a name, number, `active`, `next`, `previous`, or an offset
          like +1

      --release <RELEASE>
          Move the document to a release (ID, name, or version)

  -e, --edit
          Edit the current text in $EDITOR

      --force
          Replace the text even if inline comments may lose their anchors

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

> Delete a document (moves it to the trash)

```
Delete a document (moves it to the trash)

Usage: linear document delete [OPTIONS] [DOCUMENT]

Arguments:
  [DOCUMENT]  Document ID or slug

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

### comment

> Add and list comments on a document

```
Add and list comments on a document

Usage: linear document comment [OPTIONS] <COMMAND>

Commands:
  add   Comment on a document, or reply to a comment
  list  List a document's comments
  help  Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

#### comment subcommands

##### add

> Comment on a document, or reply to a comment

```
Comment on a document, or reply to a comment

Usage: linear document comment add [OPTIONS] <DOCUMENT>

Arguments:
  <DOCUMENT>
          Document ID or slug

Options:
  -b, --body <TEXT>
          Comment text, in Markdown

      --body-file <FILE>
          Read the comment from a Markdown file

  -p, --parent <COMMENT>
          Reply to this top-level comment (by ID)
          
          [alias: --reply-to]

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

> List a document's comments

```
List a document's comments

Usage: linear document comment list [OPTIONS] <DOCUMENT>

Arguments:
  <DOCUMENT>  Document ID or slug

Options:
      --limit <LIMIT>  Maximum number of comments to show (a number or `all`) [default: all]
  -j, --json           Print JSON
  -h, --help           Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```
