---
name: linear-cli
description: Manage Linear issues from the command line using the linear cli. This skill allows automating linear management.
allowed-tools: Bash(linear:*), Bash(curl:*)
---

# Linear CLI

A CLI to manage Linear issues from the command line, with git and jj integration.

Generated from linear CLI v1.8.1

## Prerequisites

The `linear` command must be available on PATH. To check:

```bash
linear --version
```

If not installed, follow the instructions at:\
https://github.com/schpet/linear-cli?tab=readme-ov-file#install

## Available Commands

```
linear auth               # Manage Linear authentication
linear issue              # Manage Linear issues
linear team               # Manage Linear teams
linear project            # Manage Linear projects
linear project-update     # Manage project status updates
linear milestone          # Manage Linear project milestones
linear initiative         # Manage Linear initiatives
linear initiative-update  # Manage initiative status updates (timeline posts)
linear label              # Manage Linear issue labels
linear document           # Manage Linear documents
linear config             # Interactively generate .linear.toml configuration
linear schema             # Print the GraphQL schema to stdout
```

## Reference Documentation

- [auth](references/auth.md) - Manage Linear authentication
- [issue](references/issue.md) - Manage Linear issues
- [team](references/team.md) - Manage Linear teams
- [project](references/project.md) - Manage Linear projects
- [project-update](references/project-update.md) - Manage project status updates
- [milestone](references/milestone.md) - Manage Linear project milestones
- [initiative](references/initiative.md) - Manage Linear initiatives
- [initiative-update](references/initiative-update.md) - Manage initiative status updates (timeline posts)
- [label](references/label.md) - Manage Linear issue labels
- [document](references/document.md) - Manage Linear documents
- [config](references/config.md) - Interactively generate .linear.toml configuration
- [schema](references/schema.md) - Print the GraphQL schema to stdout

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
