# linear cli

a cli to list, start and create issues in the [linear](https://linear.app/) issue tracker. git and [jj](https://www.jj-vcs.dev/) aware to keep you in the right views in linear. allows jumping to the web or the linear desktop app similar to `gh`.

**works great with AI agents** — the CLI includes a [skill](#skills) that lets agents create issues, update status, and manage your Linear workflow alongside your code.

here's how it works:

```bash
linear config               # setup your repo, it writes a config file

linear issue list           # list unstarted issues assigned to you
linear issue list -A        # list unstarted issues assigned to anyone
linear issue start          # choose an issue to start, creates a branch
linear issue start ABC-123  # start a specific issue
linear issue view           # see current branch's issue as markdown
linear issue pr             # makes a PR with title/body preset, using gh cli
linear issue create         # create a new issue
```

it aims to be a complement to the web and desktop apps that lets you stay on the command line in an interactive or scripted way.

## screencast demos

<details>
<summary><code>linear issue create</code></summary>

<img width="600" src="docs/cast-issue-create.svg?1" alt="screencast showing the linear issue create command, interactively adding issue details">

</details>

<details>
<summary><code>linear issue start</code></summary>

<img width="600" src="docs/cast-issue-start.svg?1" alt="screencast showing the linear issue start command, interactively choosing an issue to start">

</details>

## install

### homebrew

```
brew install schpet/tap/linear
```

### deno via jsr

```bash
deno install -A --reload -f -g -n linear jsr:@schpet/linear-cli
```

### binaries

https://github.com/schpet/linear-cli/releases/latest

### local dev

```bash
git clone https://github.com/schpet/linear-cli
cd linear-cli
deno task install
```

## setup

1. create an API key at [linear.app/settings/account/security](https://linear.app/settings/account/security)[^1]

2. authenticate with the CLI:

   ```sh
   linear auth login
   ```

3. configure your project:

   ```sh
   cd my-project-repo
   linear config
   ```

see [docs/authentication.md](docs/authentication.md) for multi-workspace support and other authentication options.

the CLI works with both git and jj version control systems:

- **git**: works best when your branches include Linear issue IDs (e.g. `eng-123-my-feature`). use `linear issue start` or linear UI's 'copy git branch name' button and [related automations](https://linear.app/docs/account-preferences#git-related-automations).
- **jj**: detects issues from `Linear-issue` trailers in your commit descriptions. use `linear issue start` to automatically add the trailer, or add it manually with `jj describe`, e.g. `jj describe "$(linear issue describe ABC-123)"`

## commands

### issue commands

the current issue is determined by:

- **git**: the issue id in the current branch name (e.g. `eng-123-my-feature`)
- **jj**: the `Linear-issue` trailer in the current or ancestor commits

note that [Linear's GitHub integration](https://linear.app/docs/github#branch-format) will suggest git branch names.

```bash
linear issue view      # view current issue details in terminal
linear issue view ABC-123
linear issue view 123
linear issue view -w   # open issue in web browser
linear issue view -a   # open issue in Linear.app
linear issue id        # prints the issue id from current branch (e.g., "ENG-123")
linear issue title     # prints just the issue title
linear issue url       # prints the Linear.app URL for the issue
linear issue pr        # creates a GitHub PR with issue details via `gh pr create`
linear issue list      # list your issues in a table view (supports -s/--state and --sort)
linear issue list -w   # open issue list in web browser
linear issue list -a   # open issue list in Linear.app
linear issue start     # create/switch to issue branch and mark as started
linear issue create    # create a new issue (interactive prompts)
linear issue create -t "title" -d "description"  # create with flags
linear issue create --project "My Project" --milestone "Phase 1"  # create with milestone
linear issue update    # update an issue (interactive prompts)
linear issue update ENG-123 --milestone "Phase 2"  # set milestone on existing issue
linear issue delete    # delete an issue
linear issue comment list          # list comments on current issue
linear issue comment add           # add a comment to current issue
linear issue comment add -p <id>   # reply to a specific comment
linear issue comment update <id>   # update a comment
linear issue commits               # show all commits for an issue (jj only)
```

### team commands

```bash
linear team list       # list teams
linear team id         # print out the team id (e.g. for scripts)
linear team members    # list team members
linear team create     # create a new team
linear team autolinks  # configure GitHub repository autolinks for Linear issues
```

### project commands

```bash
linear project list    # list projects
linear project view    # view project details
```

### milestone commands

```bash
linear milestone list --project <projectId>     # list milestones for a project
linear m list --project <projectId>             # list milestones (alias)
linear milestone view <milestoneId>             # view milestone details
linear m view <milestoneId>                     # view milestone (alias)
linear milestone create --project <projectId> --name "Q1 Goals" --target-date "2026-03-31"  # create a milestone
linear m create --project <projectId>           # create a milestone (interactive)
linear milestone update <milestoneId> --name "New Name"  # update milestone name
linear m update <milestoneId> --target-date "2026-04-15"  # update target date
linear milestone delete <milestoneId>           # delete a milestone
linear m delete <milestoneId> --force           # delete without confirmation
```

### document commands

manage Linear documents from the command line. documents can be attached to projects or issues, or exist at the workspace level.

```bash
# list documents
linear document list                            # list all accessible documents
linear docs list                                # alias for document
linear document list --project <projectId>      # filter by project
linear document list --issue TC-123             # filter by issue
linear document list --json                     # output as JSON

# view a document
linear document view <slug>                     # view document rendered in terminal
linear document view <slug> --raw               # output raw markdown (for piping)
linear document view <slug> --web               # open in browser
linear document view <slug> --json              # output as JSON

# create a document
linear document create --title "My Doc" --content "# Hello"           # inline content
linear document create --title "Spec" --content-file ./spec.md        # from file
linear document create --title "Doc" --project <projectId>            # attach to project
linear document create --title "Notes" --issue TC-123                 # attach to issue
cat spec.md | linear document create --title "Spec"                   # from stdin

# update a document
linear document update <slug> --title "New Title"                     # update title
linear document update <slug> --content-file ./updated.md             # update content
linear document update <slug> --edit                                  # open in $EDITOR

# delete a document
linear document delete <slug>                   # soft delete (move to trash)
linear document delete <slug> --permanent       # permanent delete
linear document delete --bulk <slug1> <slug2>   # bulk delete
```

### other commands

```bash
linear --help          # show all commands
linear --version       # show version
linear config          # setup the project
linear completions     # generate shell completions
```

## configuration options

the CLI supports configuration via environment variables or a `.linear.toml` config file. environment variables take precedence over config file values.

| option          | env var                  | toml key          | example                    | description                           |
| --------------- | ------------------------ | ----------------- | -------------------------- | ------------------------------------- |
| Team ID         | `LINEAR_TEAM_ID`         | `team_id`         | `"ENG"`                    | default team for operations           |
| Workspace       | `LINEAR_WORKSPACE`       | `workspace`       | `"mycompany"`              | workspace slug for web/app URLs       |
| Issue sort      | `LINEAR_ISSUE_SORT`      | `issue_sort`      | `"priority"` or `"manual"` | how to sort issue lists               |
| VCS             | `LINEAR_VCS`             | `vcs`             | `"git"` or `"jj"`          | version control system (default: git) |
| Download images | `LINEAR_DOWNLOAD_IMAGES` | `download_images` | `true` or `false`          | download images when viewing issues   |

the config file can be placed at (checked in order, first found is used):

- `./linear.toml` or `./.linear.toml` (current directory)
- `<repo-root>/linear.toml` or `<repo-root>/.linear.toml` (repository root)
- `<repo-root>/.config/linear.toml`
- `$XDG_CONFIG_HOME/linear/linear.toml` or `~/.config/linear/linear.toml` (Unix)
- `%APPDATA%\linear\linear.toml` (Windows)

## skills

linear-cli includes a skill that helps AI agents use the CLI effectively. for use cases outside the CLI, it includes instructions to interact directly with the graphql api, including authentication.

### claude code

install the skill using [claude code's plugin system](https://code.claude.com/docs/en/skills):

```bash
# from claude code
/plugin marketplace add schpet/linear-cli
/plugin install linear-cli@linear-cli

# from bash
claude plugin marketplace add schpet/linear-cli
claude plugin install linear-cli@linear-cli

# to update
claude plugin marketplace update linear-cli
claude plugin update linear-cli@linear-cli
```

### skills.sh for other agents

install the skill using [skills.sh](https://skills.sh):

```bash
npx skills add schpet/linear-cli
```

view the skill at [skills.sh/schpet/linear-cli/linear-cli](https://skills.sh/schpet/linear-cli/linear-cli)

## development

### updating skill documentation

the skill documentation in `skills/linear-cli/` is automatically generated from the CLI help text. after making changes to commands or help text, regenerate the docs:

```bash
deno task generate-skill-docs
```

this will:

- discover all commands and subcommands from `linear --help`
- generate reference documentation for each command
- update the `SKILL.md` file from `SKILL.template.md`

**important:** the CI checks will fail if the generated docs are out of date, so make sure to run this before committing changes that affect command structure or help text.

### code formatting

ensure code is formatted consistently:

```bash
deno fmt
```

the project uses deno's built-in formatter with configuration in `deno.json`. formatting is checked in CI.

## why

linear's UI is incredibly good but it slows me down. i find the following pretty grating to experience frequently:

- switching context from my repo to linear
- not being on the right view when i open linear
- linear suggests a git branch, but i have to do the work of creating or switching to that branch
- linear's suggested git branch doesn't account for it already existing or having a merged pull request

this cli solves this. it knows what you're working on (via git branches or jj commit trailers), does the work of managing your version control state, and will write your pull request details for you.

[^1]: creating an API key requires member access, it is not available for guest accounts.
