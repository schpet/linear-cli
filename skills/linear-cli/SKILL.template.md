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

## Common Tasks

Copy-pasteable recipes for the most frequent flows. Prefer these dedicated commands over `linear api` — reach for the GraphQL fallback only when no dedicated command or flag covers the operation.

### Query issues with filters

`issue query` searches across all assignees and supports structured filters that can be combined:

```bash
linear issue query --team ENG --state started --json
linear issue query --project "Mobile App" --state backlog --state triage --unassigned
linear issue query --assignee sam --label bug --updated-after 2026-01-01
```

Note: `linear issue list` is an alias of `issue mine` and only shows _your_ issues — use `issue query` for anything scoped to other people or a whole team/project.

### List my issues

```bash
linear issue mine --state started --sort priority
```

### Create an issue

```bash
linear issue create --team ENG --title "Fix login redirect" \
  --description-file ./description.md --no-interactive
```

Write multi-line markdown to a file and pass `--description-file` (see the markdown section below); `--no-interactive` avoids prompts in scripted use.

### Update an issue's state, assignee, or labels

```bash
linear issue update ENG-123 --state "In Review" --assignee sam
linear issue update ENG-123 --unassign
linear issue update ENG-123 --label infra --label security  # replaces the label set
```

### Add a comment

```bash
linear issue comment add ENG-123 --body-file ./comment.md
```

### Attach an image or screenshot so it is visible inline

```bash
linear issue comment add ENG-123 --attach ./screenshot.png
```

This uploads the image and embeds it in a comment, where Linear renders it inline. Do not use `linear issue attach` when the image must be visible: that command creates a sidebar link attachment and does not render images inline.

### View an issue / get its URL

```bash
linear issue view ENG-123          # details incl. comments
linear issue view ENG-123 --json   # structured output
linear issue url ENG-123           # print just the URL
```

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
{{COMMANDS}}
```

## Reference Documentation

{{REFERENCE_TOC}}

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
