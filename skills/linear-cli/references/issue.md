# issue

> Manage issues

## Usage

```
Manage issues

Usage: linear issue [OPTIONS] <COMMAND>

Commands:
  list           List issues, assigned to you by default [aliases: mine, l]
  query          Find issues by filters or full-text search [alias: q]
  view           Show an issue [alias: v]
  create         Create an issue
  update         Update an issue
  delete         Delete an issue (moves it to the trash) [alias: d]
  archive        Archive an issue
  start          Start an issue: switch to its branch and mark it started
  id             Print the issue ID of the current branch or jj change
  title          Print an issue's title
  url            Print an issue's URL
  describe       Print an issue's title and a Linear-issue trailer, for commit messages
  commits        List the commits that reference an issue (jj only)
  pull-request   Open a GitHub pull request for an issue [alias: pr]
  comment        Add, list, edit, and delete comments on an issue
  attach         Upload a file and attach it to an issue
  link           Link a URL to an issue
  relation       Manage relations between issues, like blocks and duplicates
  agent-session  Inspect agent sessions on an issue
  help           Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

## Subcommands

### list

> List issues, assigned to you by default

```
List issues, assigned to you by default

Usage: linear issue list [OPTIONS]

Options:
  -s, --state <STATE>          Show issues in this state: a type (triage, backlog, unstarted,
                               started, completed, canceled), name, or ID; repeatable [default:
                               unstarted]
      --all-states             Show issues in every state
      --sort <SORT>            Sort order [default: the issue_sort setting, or priority] [possible
                               values: manual, priority]
      --team <TEAM>            Show this team's issues (key, name, or ID); defaults to the
                               configured team
      --assignee <USER>        Show only issues assigned to this user: a username, email, name, or
                               @me
  -A, --all-assignees          Show issues of every assignee
  -U, --unassigned             Show only unassigned issues
      --project <PROJECT>      Show only this project's issues (ID, slug, or name)
      --project-label <LABEL>  Show only issues in projects with this project label
      --cycle <CYCLE>          Show only this cycle's issues: a name, number, `active`, `next`,
                               `previous`, or an offset like +1 or -1
      --milestone <MILESTONE>  Show only this milestone's issues (ID, or name with --project)
  -l, --label <LABEL>          Show only issues with this label; repeat to require several
      --created-after <DATE>   Show only issues created after this date (YYYY-MM-DD or RFC 3339)
      --updated-after <DATE>   Show only issues updated after this date (YYYY-MM-DD or RFC 3339)
      --limit <LIMIT>          Maximum number of issues to show (a number or `all`) [default: 50]
  -w, --web                    Open the list in the browser
  -a, --app                    Open the list in the Linear app
      --no-pager               Do not page long output
  -j, --json                   Print JSON
  -h, --help                   Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### query

> Find issues by filters or full-text search

```
Find issues by filters or full-text search

Usage: linear issue query [OPTIONS]

Options:
      --search <TEXT>          Search issue titles and descriptions for this text
      --search-comments        Also search comments (with --search)
      --team <TEAM>            Show this team's issues (key, name, or ID); repeatable [default: the
                               configured team]
      --all-teams              Show every team's issues
  -s, --state <STATE>          Show issues in this state: a type (triage, backlog, unstarted,
                               started, completed, canceled), name, or ID; repeatable
      --sort <SORT>            Sort order, except with --search [default: the issue_sort setting, or
                               priority] [possible values: manual, priority]
      --assignee <USER>        Show only issues assigned to this user: a username, email, name, or
                               @me
  -U, --unassigned             Show only unassigned issues
      --project <PROJECT>      Show only this project's issues (ID, slug, or name)
      --project-label <LABEL>  Show only issues in projects with this project label
      --cycle <CYCLE>          Show only this cycle's issues: a name, number, `active`, `next`,
                               `previous`, or an offset like +1 or -1
      --milestone <MILESTONE>  Show only this milestone's issues (ID, or name with --project)
  -l, --label <LABEL>          Show only issues with this label; repeat to require several
      --created-after <DATE>   Show only issues created after this date (YYYY-MM-DD or RFC 3339)
      --updated-after <DATE>   Show only issues updated after this date (YYYY-MM-DD or RFC 3339)
      --limit <LIMIT>          Maximum number of issues to show (a number or `all`) [default: 50]
      --include-archived       Include archived issues
  -j, --json                   Print JSON
      --no-pager               Do not page long output
  -h, --help                   Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### view

> Show an issue

```
Show an issue

Usage: linear issue view [OPTIONS] [ISSUE]

Arguments:
  [ISSUE]  Issue ID like ENG-123, or a URL; defaults to the current branch's issue

Options:
  -w, --web                    Open the issue in the browser
  -a, --app                    Open the issue in the Linear app
      --no-comments            Leave out comments
      --show-resolved-threads  Include resolved comment threads
      --no-pager               Do not page long output
  -j, --json                   Print JSON
      --no-download            Keep remote image and file URLs instead of downloading them
  -h, --help                   Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### create

> Create an issue

```
Create an issue

Usage: linear issue create [OPTIONS]

Options:
  -t, --title <TITLE>
          Issue title

  -d, --description <DESCRIPTION>
          Issue description, in Markdown

      --description-file <FILE>
          Read the description from a Markdown file

      --team <TEAM>
          Team (key, name, or ID); defaults to the configured team

  -a, --assignee <USER>
          Assignee: a username, email, name, or @me

  -s, --state <STATE>
          Workflow state, by name or type

  -p, --priority <PRIORITY>
          Priority, by name or number (0 none, 1 urgent to 4 low)
          
          [possible values: none, urgent, high, medium, low]

      --estimate <POINTS>
          Estimate, in points

  -l, --label <LABEL>
          Label; repeat for several labels

      --due-date <DATE>
          Due date (YYYY-MM-DD)

      --parent <ISSUE>
          Parent issue, like ENG-123

      --project <PROJECT>
          Project (ID, slug, or name)

      --milestone <MILESTONE>
          Project milestone (ID, or name with --project)

      --cycle <CYCLE>
          Cycle: a name, number, `active`, `next`, `previous`, or an offset like +1 or -1

      --template <TEMPLATE>
          Start from this issue template (name or ID) instead of the team's default
          
          The team's templates and workspace templates are searched. The template fills in anything
          you do not pass: flags override it, --label adds to its labels, and --description replaces
          its body. With a template, --title is optional.

      --no-use-default-template
          Do not apply the team's default template

      --start
          Start the issue after creating it

  -i, --interactive
          Ask for every field instead of taking them as flags
          
          Only --parent and --project can be combined with it.

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

> Update an issue

```
Update an issue

Usage: linear issue update [OPTIONS] [ISSUE]

Arguments:
  [ISSUE]
          Issue ID like ENG-123, or a URL; defaults to the current branch's issue

Options:
  -t, --title <TITLE>
          New title

  -d, --description <DESCRIPTION>
          New description, in Markdown

      --description-file <FILE>
          Read the new description from a Markdown file

      --team <TEAM>
          Move the issue to this team (key, name, or ID)

  -a, --assignee <USER>
          Assignee: a username, email, name, or @me

      --unassign
          Remove the assignee

  -s, --state <STATE>
          Workflow state, by name or type

  -p, --priority <PRIORITY>
          Priority, by name or number (0 none, 1 urgent to 4 low)
          
          [possible values: none, urgent, high, medium, low]

      --estimate <POINTS>
          Estimate, in points

      --clear-estimate
          Remove the estimate

  -l, --label <LABEL>
          Set the labels, replacing the current ones; repeatable

      --add-label <LABEL>
          Add a label, keeping the others; repeatable

      --remove-label <LABEL>
          Remove a label, keeping the others; repeatable

      --due-date <DATE>
          Due date (YYYY-MM-DD)

      --clear-due-date
          Remove the due date

      --parent <ISSUE>
          Parent issue, like ENG-123

      --clear-parent
          Remove the parent

      --project <PROJECT>
          Project (ID, slug, or name)

      --clear-project
          Remove the issue from its project

      --milestone <MILESTONE>
          Project milestone (ID, or name within --project or the issue's project)

      --clear-milestone
          Remove the issue from its milestone

      --cycle <CYCLE>
          Cycle: a name, number, `active`, `next`, `previous`, or an offset like +1 or -1

      --clear-cycle
          Remove the issue from its cycle

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

> Delete an issue (moves it to the trash)

```
Delete an issue (moves it to the trash)

Usage: linear issue delete [OPTIONS] [ISSUE]

Arguments:
  [ISSUE]  Issue ID like ENG-123, or a URL; defaults to the current branch's issue

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

### archive

> Archive an issue

```
Archive an issue

Linear archives closed issues on its own, so prefer closing an issue (`issue update --state`) and
letting auto-archive run, or `issue delete` to trash it. Archived issues drop out of list, query,
and search results unless --include-archived is passed. See
https://linear.app/docs/delete-archive-issues

Usage: linear issue archive [OPTIONS] [ISSUE]

Arguments:
  [ISSUE]
          Issue ID like ENG-123, or a URL; defaults to the current branch's issue

Options:
  -y, --yes
          Do not ask for confirmation

      --bulk [<IDS>...]
          Act on several at once instead of one

      --bulk-file <FILE>
          Read the IDs from a file, one per line

      --bulk-stdin
          Read the IDs from stdin, one per line

  -h, --help
          Print help (see a summary with '-h')

Global options:
      --workspace <SLUG>
          Workspace to use, by the name its credential is stored under

      --no-input
          Never prompt; fail instead when a required value is missing
```

### start

> Start an issue: switch to its branch and mark it started

```
Start an issue: switch to its branch and mark it started

Usage: linear issue start [OPTIONS] [ISSUE]

Arguments:
  [ISSUE]  Issue ID like ENG-123, or a URL; asked for when omitted

Options:
      --team <TEAM>      Team to pick from, and the team of a bare issue number (key, name, or ID);
                         defaults to the configured team
  -A, --all-assignees    Offer issues of every assignee in the picker
  -U, --unassigned       Offer only unassigned issues in the picker
  -f, --from-ref <REF>   Git ref to create the branch from
  -b, --branch <BRANCH>  Branch name to use instead of the issue's
  -h, --help             Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### id

> Print the issue ID of the current branch or jj change

```
Print the issue ID of the current branch or jj change

Usage: linear issue id [OPTIONS]

Options:
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### title

> Print an issue's title

```
Print an issue's title

Usage: linear issue title [OPTIONS] [ISSUE]

Arguments:
  [ISSUE]  Issue ID like ENG-123, or a URL; defaults to the current branch's issue

Options:
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### url

> Print an issue's URL

```
Print an issue's URL

Usage: linear issue url [OPTIONS] [ISSUE]

Arguments:
  [ISSUE]  Issue ID like ENG-123, or a URL; defaults to the current branch's issue

Options:
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### describe

> Print an issue's title and a Linear-issue trailer, for commit messages

```
Print an issue's title and a Linear-issue trailer, for commit messages

Usage: linear issue describe [OPTIONS] [ISSUE]

Arguments:
  [ISSUE]  Issue ID like ENG-123, or a URL; defaults to the current branch's issue

Options:
  -r, --references  Write "References" instead of "Fixes" in the trailer [alias: --ref]
  -h, --help        Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### commits

> List the commits that reference an issue (jj only)

```
List the commits that reference an issue (jj only)

Usage: linear issue commits [OPTIONS] [ISSUE]

Arguments:
  [ISSUE]  Issue ID like ENG-123, or a URL; defaults to the current change's issue

Options:
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### pull-request

> Open a GitHub pull request for an issue

```
Open a GitHub pull request for an issue

Usage: linear issue pull-request [OPTIONS] [ISSUE]

Arguments:
  [ISSUE]  Issue ID like ENG-123, or a URL; defaults to the current branch's issue

Options:
      --base <BRANCH>    Branch to merge into
      --draft            Open the pull request as a draft
  -t, --title <TITLE>    Pull request title, after the issue ID [default: the issue title]
      --web              Open the pull request in the browser
      --head <BRANCH>    Branch that holds the commits
  -T, --template <FILE>  Start the body from this template file; the issue URL is appended
      --no-template      Ignore the pr_template setting
  -h, --help             Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### comment

> Add, list, edit, and delete comments on an issue

```
Add, list, edit, and delete comments on an issue

Usage: linear issue comment [OPTIONS] <COMMAND>

Commands:
  add     Comment on an issue, or reply to a comment
  list    List an issue's comments
  update  Edit a comment
  delete  Delete a comment
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

#### comment subcommands

##### add

> Comment on an issue, or reply to a comment

```
Comment on an issue, or reply to a comment

Images uploaded with --attach render inline.

Usage: linear issue comment add [OPTIONS] [ISSUE]

Arguments:
  [ISSUE]
          Issue ID like ENG-123, or a URL; defaults to the current branch's issue

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

  -a, --attach <FILE>
          Upload a file and link it in the comment (images render inline); repeatable

      --public
          Make uploaded files public instead of visible to workspace members only

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

> List an issue's comments

```
List an issue's comments

Usage: linear issue comment list [OPTIONS] [ISSUE]

Arguments:
  [ISSUE]  Issue ID like ENG-123, or a URL; defaults to the current branch's issue

Options:
      --limit <LIMIT>  Maximum number of comments to show (a number or `all`) [default: all]
  -j, --json           Print JSON
  -h, --help           Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

##### update

> Edit a comment

```
Edit a comment

Usage: linear issue comment update [OPTIONS] <COMMENT>

Arguments:
  <COMMENT>
          Comment ID

Options:
  -b, --body <TEXT>
          New text, in Markdown

      --body-file <FILE>
          Read the new text from a Markdown file

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

##### delete

> Delete a comment

```
Delete a comment

Usage: linear issue comment delete [OPTIONS] <COMMENT>

Arguments:
  <COMMENT>  Comment ID

Options:
  -y, --yes   Do not ask for confirmation
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### attach

> Upload a file and attach it to an issue

```
Upload a file and attach it to an issue

The file is listed in the issue's sidebar; images do not render inline. To show an image in the
conversation, use `issue comment add --attach`.

Usage: linear issue attach [OPTIONS] <ISSUE> <FILE>

Arguments:
  <ISSUE>
          Issue ID like ENG-123, or a URL

  <FILE>
          File to upload

Options:
  -t, --title <TITLE>
          Attachment title [default: the file name]

  -c, --comment <TEXT>
          Also add a comment with this text, linked to the attachment

      --public
          Make the upload public instead of visible to workspace members only

  -h, --help
          Print help (see a summary with '-h')

Global options:
      --workspace <SLUG>
          Workspace to use, by the name its credential is stored under

      --no-input
          Never prompt; fail instead when a required value is missing
```

### link

> Link a URL to an issue

```
Link a URL to an issue

Usage: linear issue link [OPTIONS] <ISSUE|URL> [URL]

Arguments:
  <ISSUE|URL>  Issue ID like ENG-123; or, alone, the URL to link to the current branch's issue
  [URL]        URL to link, when the issue is given first

Options:
  -t, --title <TITLE>  Link title
  -h, --help           Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### relation

> Manage relations between issues, like blocks and duplicates

```
Manage relations between issues, like blocks and duplicates

Usage: linear issue relation [OPTIONS] <COMMAND>

Commands:
  add     Relate two issues
  delete  Remove a relation between two issues
  list    List an issue's relations
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

#### relation subcommands

##### add

> Relate two issues

```
Relate two issues

Usage: linear issue relation add [OPTIONS] <ISSUE> <RELATION> <RELATED>

Arguments:
  <ISSUE>     Issue ID like ENG-123, or a URL
  <RELATION>  How ISSUE relates to RELATED [possible values: blocks, blocked-by, related, duplicate]
  <RELATED>   The other issue

Options:
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

##### delete

> Remove a relation between two issues

```
Remove a relation between two issues

Usage: linear issue relation delete [OPTIONS] <ISSUE> <RELATION> <RELATED>

Arguments:
  <ISSUE>     Issue ID like ENG-123, or a URL
  <RELATION>  How ISSUE relates to RELATED [possible values: blocks, blocked-by, related, duplicate]
  <RELATED>   The other issue

Options:
  -y, --yes   Do not ask for confirmation
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

##### list

> List an issue's relations

```
List an issue's relations

Usage: linear issue relation list [OPTIONS] [ISSUE]

Arguments:
  [ISSUE]  Issue ID like ENG-123, or a URL; defaults to the current branch's issue

Options:
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

### agent-session

> Inspect agent sessions on an issue

```
Inspect agent sessions on an issue

Usage: linear issue agent-session [OPTIONS] <COMMAND>

Commands:
  list  List an issue's agent sessions
  view  Show an agent session and its activity [alias: v]
  help  Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

#### agent-session subcommands

##### list

> List an issue's agent sessions

```
List an issue's agent sessions

Usage: linear issue agent-session list [OPTIONS] [ISSUE]

Arguments:
  [ISSUE]  Issue ID like ENG-123, or a URL; defaults to the current branch's issue

Options:
      --limit <LIMIT>    Maximum number of sessions to show (a number or `all`) [default: all]
  -j, --json             Print JSON
      --status <STATUS>  Show only sessions with this status [possible values: pending, active,
                         complete, awaiting-input, error, stale]
  -h, --help             Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```

##### view

> Show an agent session and its activity

```
Show an agent session and its activity

Usage: linear issue agent-session view [OPTIONS] <SESSION>

Arguments:
  <SESSION>  Agent session ID

Options:
  -j, --json  Print JSON
  -h, --help  Print help

Global options:
      --workspace <SLUG>  Workspace to use, by the name its credential is stored under
      --no-input          Never prompt; fail instead when a required value is missing
```
