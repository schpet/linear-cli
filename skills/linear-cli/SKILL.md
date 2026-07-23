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

If not installed globally, you can run it without installing via npx:

```bash
npx @schpet/linear-cli --version
```

All subsequent commands can be prefixed with `npx @schpet/linear-cli` in place of `linear`. Otherwise, follow the install instructions at:\
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

## Available Commands

Compact command list, generated from `linear --help`:

```bash
linear api

linear auth
linear auth default
linear auth list
linear auth login
linear auth logout
linear auth migrate
linear auth token
linear auth whoami

linear config

linear cycle
linear cycle list
linear cycle view

linear document
linear document create
linear document delete
linear document list
linear document update
linear document view

linear initiative
linear initiative add-project
linear initiative archive
linear initiative create
linear initiative delete
linear initiative list
linear initiative remove-project
linear initiative unarchive
linear initiative update
linear initiative view

linear initiative-update
linear initiative-update create
linear initiative-update list

linear issue
linear issue agent-session
linear issue agent-session list
linear issue agent-session view
linear issue attach
linear issue comment
linear issue comment add
linear issue comment delete
linear issue comment list
linear issue comment update
linear issue commits
linear issue create
linear issue delete
linear issue describe
linear issue id
linear issue link
linear issue mine
linear issue pull-request
linear issue query
linear issue relation
linear issue relation add
linear issue relation delete
linear issue relation list
linear issue start
linear issue title
linear issue update
linear issue url
linear issue view

linear label
linear label create
linear label delete
linear label list

linear milestone
linear milestone create
linear milestone delete
linear milestone list
linear milestone update
linear milestone view

linear project
linear project create
linear project delete
linear project list
linear project update
linear project view

linear project-update
linear project-update create
linear project-update list

linear schema

linear team
linear team autolinks
linear team create
linear team delete
linear team id
linear team list
linear team members
linear team states

linear user
linear user list
```

## Reference Documentation

- [api](references/api.md) - Make a raw GraphQL API request
- [auth](references/auth.md) - Manage Linear authentication
- [config](references/config.md) - Interactively generate .linear.toml configuration
- [cycle](references/cycle.md) - Manage Linear team cycles
- [document](references/document.md) - Manage Linear documents
- [initiative](references/initiative.md) - Manage Linear initiatives
- [initiative-update](references/initiative-update.md) - Manage initiative status updates (timeline posts)
- [issue](references/issue.md) - Manage Linear issues
- [label](references/label.md) - Manage Linear issue labels
- [milestone](references/milestone.md) - Manage Linear project milestones
- [project](references/project.md) - Manage Linear projects
- [project-update](references/project-update.md) - Manage project status updates
- [schema](references/schema.md) - Print the GraphQL schema to stdout
- [team](references/team.md) - Manage Linear teams
- [user](references/user.md) - Manage Linear users

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

Some commands have required flags that aren't obvious. Notable examples:

- `issue list` sorts by priority by default — override via `--sort` (valid values: `manual`, `priority`), the `issue_sort` config option, or the `LINEAR_ISSUE_SORT` env var. Requires `--team <key>` unless the team can be inferred from the directory — if unknown, run `linear team list` first.
- `--no-pager` is only supported on `issue list` — passing it to other commands like `project list` will error.

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
