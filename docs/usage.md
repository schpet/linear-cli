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

# A state name or ID works too, and mixes with types
linear issue list --state "In Review"
linear issue list --state started --state "In Review"
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
# List issues for specific team (key, name, or ID)
linear issue list --team TEAM
linear issue list --team "Team Name"

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

# Create for specific team (key, name, or ID)
linear issue create --team TEAM

# Create and start working on it
linear issue create --start
```

create from a template (name or ID, applied by Linear server-side):

```bash
# The template supplies the title, body, and fields; --title is optional
linear issue create --team ENG --template "Bug report"

# Flags you pass override the template's values; labels merge with the template's
linear issue create --team ENG --template "Bug report" --title "Login fails" --label security

# --description replaces the template body; leave it out to keep the body
linear issue create --team ENG --template "Bug report" --description "Just this text"
```

`--template` takes the place of the team's default template (`--no-use-default-template` is implied; passing it too is fine). See [templates](#templates) for listing and inspecting them.

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

clear optional fields (each `--clear-*` flag conflicts with its set flag):

```bash
# Remove the due date, estimate, parent, project, or milestone
linear issue update TEAM-123 --clear-due-date
linear issue update TEAM-123 --clear-estimate --clear-parent
linear issue update TEAM-123 --clear-project --clear-milestone

# Move to another project and detach the milestone in one update
linear issue update TEAM-123 --project "Mobile App" --clear-milestone

# Assignee and cycle have their own clearing flags
linear issue update TEAM-123 --unassign --clear-cycle
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

#### issue comments

```bash
# List comments (threads, newest first); --json keeps the GraphQL connection
linear issue comment list TEAM-123
linear issue comment list TEAM-123 --json

# Add a comment; --body-file is preferred for markdown
linear issue comment add TEAM-123 --body "Reproduced on staging"
linear issue comment add TEAM-123 --body-file notes.md

# Reply to a top-level comment (-p / --parent are aliases of --reply-to)
linear issue comment add TEAM-123 --body "Fixed in #42" --reply-to COMMENT-ID
```

### teams

wherever a command takes a team, pass its key, its name, or its UUID. keys are canonical; an unknown team errors and lists the valid keys.

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
linear team members "Team Name"
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

# Create from a project template (name or ID); explicit flags override the template's values
linear project create --name "Q3 launch" --team APP --template "Kickoff"
```

#### update a project

```bash
# --description is the short summary; --content is the long-form overview body
linear project update PROJECT-ID --description "Short summary" --content "## Overview"

# Replace the overview body from a markdown file
linear project update PROJECT-ID --content-file overview.md

# Remove the lead, start date, or target date (each conflicts with its set flag)
linear project update PROJECT-ID --clear-lead --clear-start-date --clear-target-date

# Teams, labels, and initiatives are sets. --team, --label, and --initiative
# replace the whole set; --add-*/--remove-* change it incrementally. All are
# repeatable, and a replace flag cannot be combined with its add/remove flags.
linear project update PROJECT-ID --add-team OPS --remove-team APP
linear project update PROJECT-ID --add-label Launch --remove-label Beta
linear project update PROJECT-ID --add-initiative "Q4 Bets"     # ID, slug, or name
linear project update PROJECT-ID --initiative "Q4 Bets" --initiative "Platform"  # exactly these
# Removing a team, label, or initiative the project does not have is an error.
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

#### project comments

```bash
# A project is a UUID, slug ID, or exact name
linear project comment list "Mobile launch"
linear project comment list PROJECT-ID --json

linear project comment add PROJECT-ID --body "Kickoff is Monday"
linear project comment add PROJECT-ID --body-file update.md --reply-to COMMENT-ID
```

### documents and initiatives

Documents and initiatives take the same `comment list` and `comment add` subcommands as issues and projects. A document is a UUID or slug; an initiative is a UUID, slug, or name.

```bash
linear document comment list DOC-SLUG          # inline comments show the text they quote
linear document comment list DOC-SLUG --json   # quotedText and parent are in the JSON
linear document comment add DOC-SLUG --body-file review.md
linear document comment add DOC-SLUG --body "Agreed" --reply-to COMMENT-ID

linear initiative comment list "Platform"
linear initiative comment add "Platform" --body "Scope locked for Q3"
```

### templates

Linear templates pre-fill an issue, project, or document. A template belongs to a team or to the whole workspace; team templates are only available in that team, workspace templates everywhere.

#### list templates

```bash
linear template list                          # every template in the workspace
linear template list --type issue             # issue, project, or document
linear template list --team ENG               # ENG's templates plus workspace-level ones
linear template list --type project --json    # raw template objects, after the same filtering and ordering as the table
```

#### view a template

```bash
linear template view "Bug report"             # by name (exact, case-insensitive)
linear template view <template-id>            # by ID; needed when the same name exists in several teams
linear template view "Bug report" --json      # raw GraphQL object
linear template view "Bug report" --json | jq '.templateData | fromjson'   # decode the pre-filled data
```

The human view prints the template's metadata and then every key of its pre-filled data: title, priority, estimate, labels, state, sub-issues, and the body (Linear stores the body as rich text; it is shown as markdown). References come back as IDs, which `linear team states`, `linear label list`, and `linear user list` can map to names.

#### apply a template

Pass `--template <name|id>` to `issue create` or `project create`. Linear applies the template server-side on create; the CLI only sends the template's ID.

- Anything you pass explicitly (title, priority, state, description, and so on) overrides the template's value.
- `--label` merges with the template's labels rather than replacing them.
- `--description` replaces the template body. Leave it out to keep the body.
- `issue create --template` makes `--title` optional and takes the place of the team's default template (`--no-use-default-template` is implied and may be passed as well).
- Document templates can be listed and viewed, but Linear's API offers no way to apply one when creating a document, so `document create` has no `--template` flag.

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
