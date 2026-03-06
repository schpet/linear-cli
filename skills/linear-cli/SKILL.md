---
name: linear-cli
description: Manage Linear issues from the command line using the linear cli. This skill allows automating linear management.
allowed-tools: Bash(linear:*), Bash(curl:*)
---

# Linear CLI

A CLI to manage Linear issues from the command line, with git and jj integration.

## Prerequisites

The `linear` command must be available on PATH. To check:

```bash
linear --version
```

If not installed, follow the instructions at:\
https://github.com/schpet/linear-cli?tab=readme-ov-file#install

## Best Practices for Markdown Content

When working with issue descriptions or comment bodies that contain markdown, **always prefer using file-based flags** instead of passing content as command-line arguments:

- Use `--description-file` for `issue create` and `issue update` commands
- Use `--body-file` for `comment add` and `comment update` commands

**Why use file-based flags:**

- Ensures proper formatting in the Linear web UI
- Avoids shell escaping issues with newlines and special characters
- Prevents literal `\n` sequences from appearing in markdown
- Makes it easier to work with multi-line content

**Example workflow:**

```bash
# Write markdown to a temporary file
cat > /tmp/description.md <<'EOF'
## Summary

- First item
- Second item

## Details

This is a detailed description with proper formatting.
EOF

# Create issue using the file
linear issue create --title "My Issue" --description-file /tmp/description.md

# Or for comments
linear issue comment add ENG-123 --body-file /tmp/comment.md
```

**Only use inline flags** (`--description`, `--body`) for simple, single-line content.

## Command Reference

All subcommands with flags (omitting universal `-h/--help` and `-w/--workspace`):

```
linear auth login [--key <key>]  # Add a workspace credential
linear auth logout [--force]  # Remove a workspace credential
linear auth list  # List configured workspaces
linear auth default  # Set the default workspace
linear auth token  # Print the configured API token
linear auth whoami  # Print information about the authenticated user
linear issue id  # Print the issue based on the current git branch
linear issue list [--state <state>] [--all-states] [--assignee <assignee>] [--all-assignees] [--unassigned] [--sort <sort>] [--team <team>] [--project <project>] [--cycle <cycle>] [--limit <limit>] [--web] [--app] [--no-pager]  # List your issues
linear issue title  # Print the issue title
linear issue start [--all-assignees] [--unassigned] [--from-ref <fromRef>] [--branch <branch>]  # Start working on an issue
linear issue view [--web] [--app] [--no-comments] [--no-pager] [--json] [--no-download]  # View issue details (default) or open in browser/app
linear issue url  # Print the issue URL
linear issue describe [--references]  # Print the issue title and Linear-issue trailer
linear issue commits  # Show all commits for a Linear issue (jj only)
linear issue pull-request [--base <branch>] [--draft] [--title <title>] [--web] [--head <branch>]  # Create a GitHub pull request with issue details
linear issue delete [--confirm] [--bulk <ids...>] [--bulk-file <file>] [--bulk-stdin]  # Delete an issue
linear issue create [--start] [--assignee <assignee>] [--due-date <dueDate>] [--parent <parent>] [--priority <priority>] [--estimate <estimate>] [--description <description>] [--description-file <path>] [--label <label>] [--team <team>] [--project <project>] [--state <state>] [--milestone <milestone>] [--cycle <cycle>] [--no-use-default-template] [--no-interactive] [--title <title>]  # Create a linear issue
linear issue update [--assignee <assignee>] [--due-date <dueDate>] [--parent <parent>] [--priority <priority>] [--estimate <estimate>] [--description <description>] [--description-file <path>] [--label <label>] [--team <team>] [--project <project>] [--state <state>] [--milestone <milestone>] [--cycle <cycle>] [--title <title>]  # Update a linear issue
linear issue comment add [--body <text>] [--body-file <path>] [--parent <id>] [--attach <filepath>]  # Add a comment to an issue or reply to a comment
linear issue comment delete  # Delete a comment
linear issue comment update [--body <text>] [--body-file <path>]  # Update an existing comment
linear issue comment list [--json]  # List comments for an issue
linear issue attach [--title <title>] [--comment <body>]  # Attach a file to an issue
linear issue relation add  # Add a relation between two issues
linear issue relation delete  # Delete a relation between two issues
linear issue relation list  # List relations for an issue
linear team create [--name <name>] [--description <description>] [--key <key>] [--private] [--no-interactive]  # Create a linear team
linear team delete [--move-issues <targetTeam>] [--force]  # Delete a Linear team
linear team list [--web] [--app]  # List teams
linear team id  # Print the configured team id
linear team autolinks  # Configure GitHub repository autolinks for Linear issues with this team prefix
linear team members [--all]  # List team members
linear project list [--team <team>] [--all-teams] [--status <status>] [--web] [--app] [--json]  # List projects
linear project view [--web] [--app]  # View project details
linear project create [--name <name>] [--description <description>] [--team <team>] [--lead <lead>] [--status <status>] [--start-date <startDate>] [--target-date <targetDate>] [--initiative <initiative>] [--interactive] [--json]  # Create a new Linear project
linear project update [--name <name>] [--description <description>] [--status <status>] [--lead <lead>] [--start-date <startDate>] [--target-date <targetDate>] [--team <team>]  # Update a Linear project
linear project delete [--force]  # Delete (trash) a Linear project
linear project-update create [--body <body>] [--body-file <path>] [--health <health>] [--interactive]  # Create a new status update for a project
linear project-update list [--json] [--limit <limit>]  # List status updates for a project
linear cycle list [--team <team>]  # List cycles for a team
linear cycle view [--team <team>]  # View cycle details
linear milestone list [--project <projectId>]  # List milestones for a project
linear milestone view  # View milestone details
linear milestone create [--project <projectId>] [--name <name>] [--description <description>] [--target-date <date>]  # Create a new project milestone
linear milestone update [--name <name>] [--description <description>] [--target-date <date>] [--sort-order <value>] [--project <projectId>]  # Update an existing project milestone
linear milestone delete [--force]  # Delete a project milestone
linear initiative list [--status <status>] [--all-statuses] [--owner <owner>] [--web] [--app] [--json] [--archived]  # List initiatives
linear initiative view [--web] [--app] [--json]  # View initiative details
linear initiative create [--name <name>] [--description <description>] [--status <status>] [--owner <owner>] [--target-date <targetDate>] [--color <color>] [--icon <icon>] [--interactive]  # Create a new Linear initiative
linear initiative archive [--force] [--bulk <ids...>] [--bulk-file <file>] [--bulk-stdin]  # Archive a Linear initiative
linear initiative update [--name <name>] [--description <description>] [--status <status>] [--owner <owner>] [--target-date <targetDate>] [--color <color>] [--icon <icon>] [--interactive]  # Update a Linear initiative
linear initiative unarchive [--force]  # Unarchive a Linear initiative
linear initiative delete [--force] [--bulk <ids...>] [--bulk-file <file>] [--bulk-stdin]  # Permanently delete a Linear initiative
linear initiative add-project [--sort-order <sortOrder>]  # Link a project to an initiative
linear initiative remove-project [--force]  # Unlink a project from an initiative
linear initiative-update create [--body <body>] [--body-file <path>] [--health <health>] [--interactive]  # Create a new status update for an initiative
linear initiative-update list [--json] [--limit <limit>]  # List status updates for an initiative
linear label list [--team <teamKey>] [--all] [--json]  # List issue labels
linear label create [--name <name>] [--color <color>] [--description <description>] [--team <teamKey>] [--interactive]  # Create a new issue label
linear label delete [--team <teamKey>] [--force]  # Delete an issue label
linear document list [--project <project>] [--issue <issue>] [--json] [--limit <limit>]  # List documents
linear document view [--raw] [--web] [--json]  # View a document's content
linear document create [--title <title>] [--content <content>] [--content-file <path>] [--project <project>] [--issue <issue>] [--icon <icon>] [--interactive]  # Create a new document
linear document update [--title <title>] [--content <content>] [--content-file <path>] [--icon <icon>] [--edit]  # Update an existing document
linear document delete [--yes] [--bulk <ids...>] [--bulk-file <file>] [--bulk-stdin]  # Delete a document (moves to trash)
linear config  # Interactively generate .linear.toml configuration
linear schema [--json] [--output <file>]  # Print the GraphQL schema to stdout
linear api [--variable <variable>] [--variables-json <json>] [--paginate] [--silent]  # Make a raw GraphQL API request
```

## Reference Documentation

- [auth](references/auth.md) - Manage Linear authentication
- [issue](references/issue.md) - Manage Linear issues
- [team](references/team.md) - Manage Linear teams
- [project](references/project.md) - Manage Linear projects
- [project-update](references/project-update.md) - Manage project status updates
- [cycle](references/cycle.md) - Manage Linear team cycles
- [milestone](references/milestone.md) - Manage Linear project milestones
- [initiative](references/initiative.md) - Manage Linear initiatives
- [initiative-update](references/initiative-update.md) - Manage initiative status updates (timeline posts)
- [label](references/label.md) - Manage Linear issue labels
- [document](references/document.md) - Manage Linear documents
- [config](references/config.md) - Interactively generate .linear.toml configuration
- [schema](references/schema.md) - Print the GraphQL schema to stdout
- [api](references/api.md) - Make a raw GraphQL API request

For curated examples of organization features (initiatives, labels, projects, bulk operations), see [organization-features](references/organization-features.md).

## Discovering Options

To see available subcommands and flags, run `--help` on any command:

```bash
linear --help
linear issue --help
linear issue list --help
linear issue create --help
```

Each command has detailed help output describing all available flags and options.

## Using the Linear GraphQL API Directly

**Prefer the CLI for all supported operations.** The `api` command should only be used as a fallback for queries not covered by the CLI.

### Check the schema for available types and fields

Write the schema to a tempfile, then search it:

```bash
linear schema -o "${TMPDIR:-/tmp}/linear-schema.graphql"
grep -i "cycle" "${TMPDIR:-/tmp}/linear-schema.graphql"
grep -A 30 "^type Issue " "${TMPDIR:-/tmp}/linear-schema.graphql"
```

### Make a GraphQL request

**Important:** GraphQL queries containing non-null type markers (e.g. `String` followed by an exclamation mark) must be passed via heredoc stdin to avoid escaping issues. Simple queries without those markers can be passed inline.

```bash
# Simple query (no type markers, so inline is fine)
linear api '{ viewer { id name email } }'

# Query with variables — use heredoc to avoid escaping issues
linear api --variable teamId=abc123 <<'GRAPHQL'
query($teamId: String!) { team(id: $teamId) { name } }
GRAPHQL

# Search issues by text
linear api --variable term=onboarding <<'GRAPHQL'
query($term: String!) { searchIssues(term: $term, first: 20) { nodes { identifier title state { name } } } }
GRAPHQL

# Numeric and boolean variables
linear api --variable first=5 <<'GRAPHQL'
query($first: Int!) { issues(first: $first) { nodes { title } } }
GRAPHQL

# Complex variables via JSON
linear api --variables-json '{"filter": {"state": {"name": {"eq": "In Progress"}}}}' <<'GRAPHQL'
query($filter: IssueFilter!) { issues(filter: $filter) { nodes { title } } }
GRAPHQL

# Pipe to jq for filtering
linear api '{ issues(first: 5) { nodes { identifier title } } }' | jq '.data.issues.nodes[].title'
```

### Advanced: Using curl directly

For cases where you need full HTTP control, use `linear auth token`:

```bash
curl -s -X POST https://api.linear.app/graphql \
  -H "Content-Type: application/json" \
  -H "Authorization: $(linear auth token)" \
  -d '{"query": "{ viewer { id } }"}'
```
