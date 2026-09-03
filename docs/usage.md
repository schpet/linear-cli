## linear cli

### usage

linear cli provides commands to manage linear issues, teams, and projects from the command line.

### repo configuration

first, configure the cli with your linear api token:

```bash
linear config
```

this will interactively generate a `.linear.toml` configuration file in the repo.

### issues

#### list issues

list your issues (shows unstarted issues by default):

```bash
linear issue list
```

list issues with different states:

```bash
# List started issues
linear issue list --state started

# List all issues regardless of state  
linear issue list --all-states

# List multiple states
linear issue list --state unstarted --state started
```

filter by assignee:

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

other options:

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

#### view issue details

view the current issue (based on git branch):

```bash
linear issue view
```

view a specific issue:

```bash
linear issue view TEAM-123
```

view options:

```bash
# Open in web browser
linear issue view TEAM-123 --web

# Open in Linear app  
linear issue view TEAM-123 --app

# Exclude comments from output
linear issue view TEAM-123 --no-comments
```

#### start working on an issue

start the next available issue:

```bash
linear issue start
```

start a specific issue:

```bash
linear issue start TEAM-123
```

this will move the issue to "in progress" and create a git branch.

#### create an issue

create an issue interactively:

```bash
linear issue create
```

create with specific options:

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

#### update an issue

update the current issue:

```bash
linear issue update
```

update a specific issue:

```bash
linear issue update TEAM-123
```

change labels:

```bash
# Add a label, keeping existing labels
linear issue update TEAM-123 --add-label bug

# Remove a label from this issue (does not delete it from the team)
linear issue update TEAM-123 --remove-label sprint-42

# Swap labels atomically in one update
linear issue update TEAM-123 --remove-label sprint-42 --add-label sprint-43

# Replace the entire label set
linear issue update TEAM-123 --label bug --label frontend
```

#### other issue commands

get issue id from current git branch:

```bash
linear issue id
```

get issue title:

```bash
linear issue title TEAM-123
```

get issue url:

```bash
linear issue url TEAM-123
```

create a github pull request:

```bash
linear issue pull-request
linear issue pr  # Short alias
```

delete an issue:

```bash
linear issue delete TEAM-123
```

### teams

#### list teams

```bash
linear team list
linear team list --json   # machine-readable, e.g. to map a team name to its key
```

#### get team id

get team id derived from repository name:

```bash
linear team id
```

#### team members

list members of your default team:

```bash
linear team members
```

list members of a specific team:

```bash
linear team members TEAM
```

#### create a team

```bash
linear team create
```

#### configure github autolinks

set up github repository autolinks for linear issues:

```bash
linear team autolinks
```

### projects

#### create a project

```bash
# Create with a short description and long-form overview markdown
linear project create --name "API v2" --team ENG --description "Short summary" --content "## Overview"

# Read the project overview body from a markdown file
linear project create --name "API v2" --team ENG --content-file overview.md

# Create with priority, labels, members, icon, and color
linear project create --name "Mobile launch" --team APP --priority high --label Launch --member jane@example.com --icon rocket --color "#5E6AD2"
```

#### update a project

```bash
# --description is the short summary; --content is the long-form overview body
linear project update PROJECT-ID --description "Short summary" --content "## Overview"

# Replace the overview body from a markdown file
linear project update PROJECT-ID --content-file overview.md
```

#### list projects

```bash
linear project list
```

#### view project details

```bash
linear project view PROJECT-ID
linear project view PROJECT-ID --json
```

### shell completions

generate shell completions for better command-line experience:

```bash
# For bash
source <(linear completions bash)

# For zsh  
source <(linear completions zsh)

# For fish
linear completions fish | source
```

add the appropriate line to your shell's configuration file (e.g., `~/.bashrc`, `~/.zshrc`, or `~/.config/fish/config.fish`).

### global options

most commands support these options:

- `--no-pager` - disable automatic paging for long output
- `--no-color` - disable colored output
- `--help` - show help for the command

### examples

common workflows:

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
