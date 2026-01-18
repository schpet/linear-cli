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

## Available Commands

```
linear issue        # Manage issues (list, view, create, start, update, delete, comment)
linear team         # Manage teams (list, members, create, delete, autolinks)
linear project      # Manage projects (list, view, create)
linear initiative   # Manage initiatives (list, view, create, archive, unarchive, update, delete, add-project, remove-project)
linear label        # Manage labels (list, create, delete)
linear milestone    # Manage project milestones
linear config       # Configure the CLI for the current repo
linear auth         # Manage authentication (token, whoami)
linear schema       # Print the GraphQL schema (SDL or JSON)
```

## Initiative Management

```bash
# List initiatives (default: active only)
linear initiative list
linear initiative list --all-statuses
linear initiative list --status planned

# View initiative details
linear initiative view <id-or-slug>

# Create initiative
linear initiative create --name "Q1 Goals" --status active
linear initiative create -i  # Interactive mode

# Archive/unarchive
linear initiative archive <id>
linear initiative unarchive <id>

# Link projects to initiatives
linear initiative add-project <initiative> <project>
linear initiative remove-project <initiative> <project>
```

## Label Management

```bash
# List labels (shows ID, name, color, team)
linear label list
linear label list --team DEV
linear label list --workspace  # Workspace-level only

# Create label
linear label create --name "Bug" --color "#EB5757"
linear label create --name "Feature" --team DEV

# Delete label (by ID or name)
linear label delete <id>
linear label delete "Bug" --team DEV
```

## Project Management

```bash
# List projects
linear project list

# View project
linear project view <id>

# Create project
linear project create --name "New Feature" --team DEV
linear project create --name "Q1 Work" --team DEV --initiative "Q1 Goals"
linear project create -i  # Interactive mode
```

## Bulk Operations

```bash
# Delete multiple issues
linear issue delete --bulk DEV-123 DEV-124 DEV-125

# Delete from file (one ID per line)
linear issue delete --bulk-file issues.txt

# Delete from stdin
echo -e "DEV-123\nDEV-124" | linear issue delete --bulk-stdin

# Archive multiple initiatives
linear initiative archive --bulk <id1> <id2>
```

## Adding Labels to Issues

```bash
# Add label to issue
linear issue update DEV-123 --label "Bug"

# Add multiple labels
linear issue update DEV-123 --label "Bug" --label "High Priority"
```

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

**Prefer the CLI for all supported operations.** Direct API calls via curl are slower and should only be used as a fallback for advanced queries not covered by the CLI. For complex queries involving multiple calls, write and execute a script.

To make direct API calls, use `linear schema` and `linear auth token`:

### 1. Check the schema for available types and fields

Write the schema to a tempfile, then search it:

```bash
# Write schema to a tempfile (cross-platform)
linear schema -o "${TMPDIR:-/tmp}/linear-schema.graphql"

# Search for specific types or fields
grep -i "cycle" "${TMPDIR:-/tmp}/linear-schema.graphql"
grep -A 30 "^type Issue " "${TMPDIR:-/tmp}/linear-schema.graphql"

# View filter options
grep -A 50 "^input IssueFilter" "${TMPDIR:-/tmp}/linear-schema.graphql"
```

### 2. Get the auth token

```bash
linear auth token
```

### 3. Make a curl request

```bash
curl -s -X POST https://api.linear.app/graphql \
  -H "Content-Type: application/json" \
  -H "Authorization: $(linear auth token)" \
  -d '{"query": "{ issues(filter: { team: { key: { eq: \"CLI\" } } }, first: 5) { nodes { identifier title state { name } } } }"}'
```

### Example queries

```bash
# Get issues assigned to current user
curl -s -X POST https://api.linear.app/graphql \
  -H "Content-Type: application/json" \
  -H "Authorization: $(linear auth token)" \
  -d '{"query": "{ viewer { assignedIssues(first: 10) { nodes { identifier title state { name } } } } }"}'
```
