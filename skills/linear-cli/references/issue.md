# issue

> Manage Linear issues

```
Manage Linear issues

Usage: linear issue [OPTIONS] [COMMAND]

Commands:
  id             Print the issue based on the current git branch
  mine           List your issues [aliases: list, l]
  query          Query issues with structured filters [alias: q]
  title          Print the issue title
  start          Start working on an issue
  view           View issue details (default) or open in browser/app [alias: v]
  url            Print the issue URL
  describe       Print the issue title and Linear-issue trailer
  commits        Show all commits for a Linear issue (jj only)
  pull-request   Create a GitHub pull request with issue details [alias: pr]
  archive        Archive an issue
  delete         Delete an issue [alias: d]
  create         Create a linear issue
  update         Update a linear issue
  comment        Manage issue comments
  attach         Create a sidebar link attachment on an issue (images do not render inline)
  link           Link a URL to an issue
  relation       Manage issue relations (dependencies)
  agent-session  Manage agent sessions for an issue
  help           Print this message or the help of the given subcommand(s)

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

## id

> Print the issue based on the current git branch

```
Print the issue based on the current git branch

Usage: linear issue id [OPTIONS]

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

## mine

> List your issues

```
List your issues

Usage: linear issue mine [OPTIONS]

Options:
  -s, --state <state>                 Filter by workflow state type (triage, backlog, unstarted, started, completed, canceled), name, or ID (can be repeated for multiple states) [default: unstarted]
      --workspace <slug>              Target workspace (uses credentials)
      --all-states                    Show issues from all states
      --sort <sort>                   Sort order (default: priority, can also be set via LINEAR_ISSUE_SORT) [possible values: manual, priority]
      --team <team>                   Team key, name, or ID to list issues for (if not your default team)
      --project <project>             Filter by project (UUID, slug ID, or name)
      --project-label <projectLabel>  Filter by project label name (shows issues from all projects with this label)
      --cycle <cycle>                 Filter by cycle name, number, 'active'/'now', 'next', 'previous', or a relative offset like +1
      --milestone <milestone>         Filter by project milestone (UUID, or name when --project is set)
  -l, --label <label>                 Filter by label name (can be repeated for multiple labels)
      --limit <limit>                 Maximum number of issues to fetch (default: 50, use 0 for unlimited) [default: 50]
      --created-after <date>          Filter issues created after this date (ISO 8601 or YYYY-MM-DD)
      --updated-after <date>          Filter issues updated after this date (ISO 8601 or YYYY-MM-DD)
  -w, --web                           Open in web browser
  -a, --app                           Open in Linear.app
      --no-pager                      Disable automatic paging for long output
  -h, --help                          Print help
```

## query

> Query issues with structured filters

```
Query issues with structured filters

Usage: linear issue query [OPTIONS]

Options:
      --search <term>                 Full-text search term
      --workspace <slug>              Target workspace (uses credentials)
      --search-comments               Also search inside issue comments (requires --search)
      --team <team>                   Filter by team key, name, or ID (can be repeated for multiple teams)
      --all-teams                     Query across all teams
  -s, --state <state>                 Filter by workflow state type (triage, backlog, unstarted, started, completed, canceled), name, or ID (can be repeated for multiple states)
      --all-states                    Show issues from all states (this is the default)
      --assignee <assignee>           Filter by assignee (username)
  -A, --all-assignees                 Show issues for all assignees (this is the default)
  -U, --unassigned                    Show only unassigned issues
      --sort <sort>                   Sort order: manual or priority (default: priority, not available with --search) [possible values: manual, priority]
      --project <project>             Filter by project (UUID, slug ID, or name)
      --project-label <projectLabel>  Filter by project label name (shows issues from all projects with this label)
      --cycle <cycle>                 Filter by cycle name, number, 'active'/'now', 'next', 'previous', or a relative offset like +1
      --milestone <milestone>         Filter by project milestone (UUID, or name when --project is set)
  -l, --label <label>                 Filter by label name (can be repeated for multiple labels)
      --limit <limit>                 Maximum number of issues to fetch (default: 50, use 0 for unlimited) [default: 50]
      --created-after <date>          Filter issues created after this date (ISO 8601 or YYYY-MM-DD)
      --updated-after <date>          Filter issues updated after this date (ISO 8601 or YYYY-MM-DD)
      --include-archived              Include archived issues
  -j, --json                          Output results as JSON
      --no-pager                      Disable automatic paging for long output
  -h, --help                          Print help
```

## title

> Print the issue title

```
Print the issue title

Usage: linear issue title [OPTIONS] [issueId]

Arguments:
  [issueId]  

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

## start

> Start working on an issue

```
Start working on an issue

Usage: linear issue start [OPTIONS] [issueId]

Arguments:
  [issueId]  

Options:
  -A, --all-assignees       Show issues for all assignees
      --workspace <slug>    Target workspace (uses credentials)
  -U, --unassigned          Show only unassigned issues
  -f, --from-ref <fromRef>  Git ref to create new branch from
  -b, --branch <branch>     Custom branch name to use instead of the issue identifier
  -h, --help                Print help
```

## view

> View issue details (default) or open in browser/app

```
View issue details (default) or open in browser/app

Usage: linear issue view [OPTIONS] [issueId]

Arguments:
  [issueId]  

Options:
  -w, --web                    Open in web browser
      --workspace <slug>       Target workspace (uses credentials)
  -a, --app                    Open in Linear.app
      --no-comments            Exclude comments from the output
      --show-resolved-threads  Include resolved comment threads in the output
      --no-pager               Disable automatic paging for long output
  -j, --json                   Output issue data as JSON
      --no-download            Keep remote URLs instead of downloading files
  -h, --help                   Print help
```

## url

> Print the issue URL

```
Print the issue URL

Usage: linear issue url [OPTIONS] [issueId]

Arguments:
  [issueId]  

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

## describe

> Print the issue title and Linear-issue trailer

```
Print the issue title and Linear-issue trailer

Usage: linear issue describe [OPTIONS] [issueId]

Arguments:
  [issueId]  

Options:
  -r, --references        Use 'References' instead of 'Fixes' for the Linear issue link [alias: --ref]
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

## commits

> Show all commits for a Linear issue (jj only)

```
Show all commits for a Linear issue (jj only)

Usage: linear issue commits [OPTIONS] [issueId]

Arguments:
  [issueId]  

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

## pull-request

> Create a GitHub pull request with issue details

```
Create a GitHub pull request with issue details

Usage: linear issue pull-request [OPTIONS] [issueId]

Arguments:
  [issueId]  

Options:
      --base <branch>     The branch into which you want your code merged
      --workspace <slug>  Target workspace (uses credentials)
      --draft             Create the pull request as a draft
  -t, --title <title>     Optional title for the pull request (Linear issue ID will be prefixed)
      --web               Open the pull request in the browser after creating it
      --head <branch>     The branch that contains commits for your pull request
  -T, --template <file>   Start the pull request body from this template file (the Linear issue URL is appended)
      --no-template       Ignore the pr_template config option for this pull request
  -h, --help              Print help
```

## archive

> Archive an issue

```
Archive an issue

Linear archives closed issues on its own, and its docs say "archiving happens automatically with no option to manually archive items". Prefer closing (issue update --state) and letting auto-archive run, or issue delete to trash. This command calls the issueArchive mutation, which the Linear app and its official MCP server do not expose; archived issues drop out of list, query, and search results unless --include-archived is passed. See https://linear.app/docs/delete-archive-issues

Usage: linear issue archive [OPTIONS] [issueId]

Arguments:
  [issueId]
          

Options:
      --workspace <slug>
          Target workspace (uses credentials)

  -y, --confirm
          Skip confirmation prompt

      --bulk [<ids>...]
          Archive multiple issues by identifier (e.g., TC-123 TC-124)

      --bulk-file <file>
          Read issue identifiers from a file (one per line)

      --bulk-stdin
          Read issue identifiers from stdin

  -h, --help
          Print help (see a summary with '-h')
```

## delete

> Delete an issue

```
Delete an issue

Usage: linear issue delete [OPTIONS] [issueId]

Arguments:
  [issueId]  

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -y, --confirm           Skip confirmation prompt
      --bulk [<ids>...]   Delete multiple issues by identifier (e.g., TC-123 TC-124)
      --bulk-file <file>  Read issue identifiers from a file (one per line)
      --bulk-stdin        Read issue identifiers from stdin
  -h, --help              Print help
```

## create

> Create a linear issue

```
Create a linear issue

Linear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,
and `[Name](url)` do not. Get a person's URL from the `url` field of
`linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.
Run `linear markdown` for collapsible sections and the full reference.

Usage: linear issue create [OPTIONS]

Options:
      --start
          Start the issue after creation

      --workspace <slug>
          Target workspace (uses credentials)

  -a, --assignee <assignee>
          Assign the issue to 'self' or someone (by username or name)

      --due-date <dueDate>
          Due date of the issue

      --parent <parent>
          Parent issue (if any) as a team_number code

  -p, --priority <priority>
          Priority of the issue (1-4, descending priority)

      --estimate <estimate>
          Points estimate of the issue

  -d, --description <description>
          Description of the issue

      --description-file <path>
          Read description from a file (preferred for markdown content)

  -l, --label <label>
          Issue label associated with the issue. May be repeated.

      --team <team>
          Team (key, name, or ID) for the issue, if not your default team

      --project <project>
          Project for the issue (UUID, slug ID, or name)

  -s, --state <state>
          Workflow state for the issue (by name or type)

      --milestone <milestone>
          Project milestone (UUID, or name when --project is set)

      --cycle <cycle>
          Cycle name, number, 'active'/'now', 'next', 'previous', or a relative offset like +1 (use --cycle=-1 for negatives)

      --no-use-default-template
          Do not use default template for the issue

      --template <template>
          Issue template to apply, by name or ID (the team's templates plus workspace ones). Takes the place of the team's default template. The template fills in anything you do not pass: explicit flags override it, --label merges with the template's labels, and --description replaces the template body (omit it to keep the body). Makes --title optional.

      --no-interactive
          Disable interactive prompts

  -t, --title <title>
          Title of the issue

  -h, --help
          Print help (see a summary with '-h')
```

## update

> Update a linear issue

```
Update a linear issue

Linear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,
and `[Name](url)` do not. Get a person's URL from the `url` field of
`linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.
Run `linear markdown` for collapsible sections and the full reference.

Usage: linear issue update [OPTIONS] [issueId]

Arguments:
  [issueId]
          

Options:
  -a, --assignee <assignee>
          Assign the issue to 'self' or someone (by username or name)

      --workspace <slug>
          Target workspace (uses credentials)

      --unassign
          Clear the issue's assignee (cannot be combined with --assignee)

      --due-date <dueDate>
          Due date of the issue. Use --clear-due-date to remove it

      --clear-due-date
          Remove the issue's due date (cannot be combined with --due-date)

      --parent <parent>
          Parent issue (if any) as a team_number code. Use --clear-parent to remove it

      --clear-parent
          Remove the issue's parent (cannot be combined with --parent)

  -p, --priority <priority>
          Priority of the issue (1-4, descending priority)

      --estimate <estimate>
          Points estimate of the issue. Use --clear-estimate to remove it

      --clear-estimate
          Remove the issue's estimate (cannot be combined with --estimate)

  -d, --description <description>
          Description of the issue

      --description-file <path>
          Read description from a file (preferred for markdown content)

  -l, --label <label>
          Issue label associated with the issue; replaces the issue's entire label set. May be repeated. Use --add-label/--remove-label to change labels incrementally.

      --add-label <label>
          Add a label to the issue, keeping its existing labels. May be repeated.

      --remove-label <label>
          Remove a label from the issue, keeping its other labels (does not delete the label from the team). May be repeated.

      --team <team>
          Team (key, name, or ID) to move the issue to

      --project <project>
          Project to assign the issue to (UUID, slug ID, or name). Use --clear-project to remove it

      --clear-project
          Remove the issue from its project (cannot be combined with --project or --milestone)

  -s, --state <state>
          Workflow state for the issue (by name or type)

      --milestone <milestone>
          Project milestone (UUID, or name when --project is set or the issue already has a project). Use --clear-milestone to remove it

      --clear-milestone
          Remove the issue from its project milestone (cannot be combined with --milestone)

      --cycle <cycle>
          Cycle name, number, 'active'/'now', 'next', 'previous', or a relative offset like +1 (use --cycle=-1 for negatives). Use --clear-cycle to remove the issue from its cycle

      --clear-cycle
          Remove the issue from its cycle

  -t, --title <title>
          Title of the issue

  -h, --help
          Print help (see a summary with '-h')
```

## comment

> Manage issue comments

```
Manage issue comments

Usage: linear issue comment [OPTIONS] [COMMAND]

Commands:
  add     Add a comment or reply; images uploaded with --attach render inline
  delete  Delete a comment
  update  Update an existing comment
  list    List comments for an issue
  help    Print this message or the help of the given subcommand(s)

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

### add

> Add a comment or reply; images uploaded with --attach render inline

```
Add a comment or reply; images uploaded with --attach render inline

Linear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,
and `[Name](url)` do not. Get a person's URL from the `url` field of
`linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.
Run `linear markdown` for collapsible sections and the full reference.

Usage: linear issue comment add [OPTIONS] [issueId]

Arguments:
  [issueId]
          

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

  -a, --attach <filepath>
          Upload a file and add its Markdown link to the comment (images render inline; repeatable)

      --public
          Upload attached images to a public, unauthenticated URL (default: private, workspace-members only)

  -h, --help
          Print help (see a summary with '-h')
```

### delete

> Delete a comment

```
Delete a comment

Usage: linear issue comment delete [OPTIONS] <commentId>

Arguments:
  <commentId>  

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

### update

> Update an existing comment

```
Update an existing comment

Linear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,
and `[Name](url)` do not. Get a person's URL from the `url` field of
`linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.
Run `linear markdown` for collapsible sections and the full reference.

Usage: linear issue comment update [OPTIONS] <commentId>

Arguments:
  <commentId>
          

Options:
  -b, --body <text>
          New comment body text

      --workspace <slug>
          Target workspace (uses credentials)

      --body-file <path>
          Read comment body from a file (preferred for markdown content)

  -h, --help
          Print help (see a summary with '-h')
```

### list

> List comments for an issue

```
List comments for an issue

Usage: linear issue comment list [OPTIONS] [issueId]

Arguments:
  [issueId]  

Options:
  -j, --json              Output as JSON
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

## attach

> Create a sidebar link attachment on an issue (images do not render inline)

```
Create a sidebar link attachment on an issue (images do not render inline)

Usage: linear issue attach [OPTIONS] <issueId> <filepath>

Arguments:
  <issueId>   
  <filepath>  

Options:
  -t, --title <title>     Custom title for the attachment
      --workspace <slug>  Target workspace (uses credentials)
  -c, --comment <body>    Create a linked comment with this body; the file remains a sidebar attachment
      --public            Upload images to a public, unauthenticated URL (default: private, workspace-members only)
  -h, --help              Print help
```

## link

> Link a URL to an issue

```
Link a URL to an issue

Usage: linear issue link [OPTIONS] <urlOrIssueId> [url]

Arguments:
  <urlOrIssueId>  
  [url]           

Options:
  -t, --title <title>     Custom title for the link
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

## relation

> Manage issue relations (dependencies)

```
Manage issue relations (dependencies)

Usage: linear issue relation [OPTIONS] [COMMAND]

Commands:
  add     Add a relation between two issues
  delete  Delete a relation between two issues
  list    List relations for an issue
  help    Print this message or the help of the given subcommand(s)

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

### add

> Add a relation between two issues

```
Add a relation between two issues

Usage: linear issue relation add [OPTIONS] <issueId> <relationType> <relatedIssueId>

Arguments:
  <issueId>         
  <relationType>    [possible values: blocks, blocked-by, related, duplicate]
  <relatedIssueId>  

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

### delete

> Delete a relation between two issues

```
Delete a relation between two issues

Usage: linear issue relation delete [OPTIONS] <issueId> <relationType> <relatedIssueId>

Arguments:
  <issueId>         
  <relationType>    [possible values: blocks, blocked-by, related, duplicate]
  <relatedIssueId>  

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

### list

> List relations for an issue

```
List relations for an issue

Usage: linear issue relation list [OPTIONS] [issueId]

Arguments:
  [issueId]  

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

## agent-session

> Manage agent sessions for an issue

```
Manage agent sessions for an issue

Usage: linear issue agent-session [OPTIONS] [COMMAND]

Commands:
  list  List agent sessions for an issue
  view  View agent session details [alias: v]
  help  Print this message or the help of the given subcommand(s)

Options:
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```

### list

> List agent sessions for an issue

```
List agent sessions for an issue

Usage: linear issue agent-session list [OPTIONS] [issueId]

Arguments:
  [issueId]  

Options:
  -j, --json              Output as JSON
      --workspace <slug>  Target workspace (uses credentials)
      --status <status>   Filter by session status [possible values: pending, active, complete, awaitingInput, error, stale]
  -h, --help              Print help
```

### view

> View agent session details

```
View agent session details

Usage: linear issue agent-session view [OPTIONS] <sessionId>

Arguments:
  <sessionId>  

Options:
  -j, --json              Output as JSON
      --workspace <slug>  Target workspace (uses credentials)
  -h, --help              Print help
```
