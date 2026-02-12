---
name: linear-cli
description: Manage Linear issues from the command line using the linear cli. This skill allows automating linear management.
allowed-tools: Bash(linear:*), Bash(curl:*)
---

# Linear CLI

A CLI to manage Linear issues from the command line, with git and jj integration.

Generated from linear CLI v1.9.1

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
linear api                # Make a raw GraphQL API request
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

```bash
# Simple query
linear api '{ viewer { id name email } }'

# Query with variables (coerces types: booleans, numbers, null)
linear api 'query($teamId: String!) { team(id: $teamId) { name } }' --variable teamId=abc123

# Numeric and boolean variables
linear api 'query($first: Int!) { issues(first: $first) { nodes { title } } }' --variable first=5

# Complex variables via JSON
linear api 'query($filter: IssueFilter!) { issues(filter: $filter) { nodes { title } } }' \
  --variables-json '{"filter": {"state": {"name": {"eq": "In Progress"}}}}'

# Read query from stdin
echo '{ viewer { id } }' | linear api

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
