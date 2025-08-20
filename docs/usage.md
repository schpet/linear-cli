## Usage

Linear CLI provides commands to manage Linear issues, teams, and projects from
the command line.

## Repo Configuration

First, configure the CLI with your Linear API token:

```bash
linear config
```

This will interactively generate a `.linear.toml` configuration file in the
repo.

## Issues

### List Issues

List your issues (shows unstarted issues by default):

```bash
linear issue list
```

List issues with different states:

```bash
# List started issues
linear issue list --state started

# List all issues regardless of state  
linear issue list --all-states

# List multiple states
linear issue list --state unstarted --state started
```

Filter by assignee:

```bash
# List issues assigned to you
linear issue list --assignee self

# List issues assigned to specific user
linear issue list --assignee username

# List all unassigned issues
linear issue list --unassigned

# List issues for all assignees
linear issue list --all-assignees
```

Other options:

```bash
# List issues for specific team
linear issue list --team TEAM

# Sort by priority instead of manual order
linear issue list --sort priority

# Open in web browser
linear issue list --web

# Open in Linear app
linear issue list --app
```

### View Issue Details

View the current issue (based on git branch):

```bash
linear issue view
```

View a specific issue:

```bash
linear issue view TEAM-123
```

View options:

```bash
# Open in web browser
linear issue view TEAM-123 --web

# Open in Linear app  
linear issue view TEAM-123 --app

# Exclude comments from output
linear issue view TEAM-123 --no-comments
```

### Start Working on an Issue

Start the next available issue:

```bash
linear issue start
```

Start a specific issue:

```bash
linear issue start TEAM-123
```

This will move the issue to "In Progress" and create a git branch.

### Create an Issue

Create an issue interactively:

```bash
linear issue create
```

Create with specific options:

```bash
# Create with title and description
linear issue create --title "Fix bug" --description "Description here"

# Create and assign to yourself
linear issue create --assignee self

# Create with priority (1-4, where 1 is highest)
linear issue create --priority 1

# Create with estimate points
linear issue create --estimate 3

# Create with labels
linear issue create --label bug --label frontend

# Create for specific team
linear issue create --team TEAM

# Create and start working on it
linear issue create --start
```

### Update an Issue

Update the current issue:

```bash
linear issue update
```

Update a specific issue:

```bash
linear issue update TEAM-123
```

### Other Issue Commands

Get issue ID from current git branch:

```bash
linear issue id
```

Get issue title:

```bash
linear issue title TEAM-123
```

Get issue URL:

```bash
linear issue url TEAM-123
```

Create a GitHub pull request:

```bash
linear issue pull-request
linear issue pr  # Short alias
```

Delete an issue:

```bash
linear issue delete TEAM-123
```

## Teams

### List Teams

```bash
linear team list
```

### Get Team ID

Get team ID derived from repository name:

```bash
linear team id
```

### Team Members

List members of your default team:

```bash
linear team members
```

List members of a specific team:

```bash
linear team members TEAM
```

### Create a Team

```bash
linear team create
```

### Configure GitHub Autolinks

Set up GitHub repository autolinks for Linear issues:

```bash
linear team autolinks
```

## Projects

### List Projects

```bash
linear project list
```

### View Project Details

```bash
linear project view PROJECT-ID
```

## Shell Completions

Generate shell completions for better command-line experience:

```bash
# For bash
source <(linear completions bash)

# For zsh  
source <(linear completions zsh)

# For fish
linear completions fish | source
```

Add the appropriate line to your shell's configuration file (e.g., `~/.bashrc`,
`~/.zshrc`, or `~/.config/fish/config.fish`).

## Global Options

Most commands support these options:

- `--no-pager` - Disable automatic paging for long output
- `--no-color` - Disable colored output
- `--help` - Show help for the command

## Examples

Common workflows:

```bash
# Start working on the next issue
linear issue start

# View current issue details
linear issue view

# Create and start a new bug fix
linear issue create --title "Fix login error" --label bug --start

# List high priority issues
linear issue list --sort priority

# Create a pull request for current issue
linear issue pr
```
