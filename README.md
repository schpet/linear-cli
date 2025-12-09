# linear cli

a cli to list, start and create issues in the [linear](https://linear.app/) issue tracker. git and [jj](https://www.jj-vcs.dev/) aware to keep you in the right views in linear. allows jumping to the web or the linear desktop app similar to `gh`.

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

2. add the API key to your shell environment:

   ```sh
   # in ~/.bashrc or ~/.zshrc:
   export LINEAR_API_KEY="lin_api_..."

   # or in fish:
   set -Ux LINEAR_API_KEY "lin_api_..."
   ```

3. run the configuration wizard:

   ```sh
   cd my-project-repo
   linear config
   ```

   _this will create a `.linear.toml` config file in your repository with your workspace and team settings._

the CLI works with both git and jj version control systems:

- **git**: works best when your branches include Linear issue IDs (e.g. `eng-123-my-feature`). use `linear issue start` or linear UI's 'copy git branch name' button and [related automations](https://linear.app/docs/account-preferences#git-related-automations).
- **jj**: detects issues from `Linear-issue` trailers in your commit descriptions. use `linear issue start` to automatically add the trailer, or add it manually with `jj describe`.

## commands

### issue commands

the current issue is determined by:

- **git**: the issue id in the current branch name (e.g. `eng-123-my-feature`)
- **jj**: the `Linear-issue` trailer in the current or ancestor commits

note that [Linear's GitHub integration](https://linear.app/docs/github#branch-format) will suggest git branch names.

```bash
linear issue view      # view issue details in terminal
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
linear issue update    # update an issue (interactive prompts)
linear issue delete    # delete an issue
linear issue comment list          # list comments on current issue
linear issue comment add           # add a comment to current issue
linear issue comment add -p <id>   # reply to a specific comment
linear issue comment update <id>   # update a comment
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
| API key         | `LINEAR_API_KEY`         | `api_key`         | `"lin_api_..."`            | your Linear API key (required)        |
| Team ID         | `LINEAR_TEAM_ID`         | `team_id`         | `"TEAM_abc123"`            | default team for operations           |
| Workspace       | `LINEAR_WORKSPACE`       | `workspace`       | `"mycompany"`              | workspace slug for web/app URLs       |
| Issue sort      | `LINEAR_ISSUE_SORT`      | `issue_sort`      | `"priority"` or `"manual"` | how to sort issue lists               |
| VCS             | `LINEAR_VCS`             | `vcs`             | `"git"` or `"jj"`          | version control system (default: git) |
| Download images | `LINEAR_DOWNLOAD_IMAGES` | `download_images` | `true` or `false`          | download images when viewing issues   |

the config file can be placed at:

- `./linear.toml` or `./.linear.toml` (current directory)
- `<repo-root>/linear.toml` or `<repo-root>/.linear.toml` (repository root)
- `<repo-root>/.config/linear.toml`

## why

linear's UI is incredibly good but it slows me down. i find the following pretty grating to experience frequently:

- switching context from my repo to linear
- not being on the right view when i open linear
- linear suggests a git branch, but i have to do the work of creating or switching to that branch
- linear's suggested git branch doesn't account for it already existing or having a merged pull request

this cli solves this. it knows what you're working on (via git branches or jj commit trailers), does the work of managing your version control state, and will write your pull request details for you.

## claude code skill

linear-cli includes [a skill for claude code](https://code.claude.com/docs/en/skills) that helps claude use the CLI effectively. for use cases outside the CLI, it includes instructions to interact directly with the graphql api, including authentication.

```bash
# from claude code
/plugin marketplace add schpet/linear-cli
/plugin install linear-cli@linear-cli

# from bash
claude plugin marketplace add schpet/linear-cli
claude plugin install linear-cli@linear-cli
```

[^1]: creating an API key requires member access, it is not available for guest accounts.
