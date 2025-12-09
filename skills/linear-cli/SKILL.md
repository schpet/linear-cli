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
linear issue      # Manage issues (list, view, create, start, update, delete, comment)
linear team       # Manage teams (list, members, create, autolinks)
linear project    # Manage projects (list, view)
linear config     # Configure the CLI for the current repo
linear auth       # Manage authentication (token, whoami)
linear schema     # Print the GraphQL schema (SDL or JSON)
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
